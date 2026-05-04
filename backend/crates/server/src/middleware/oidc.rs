use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::{
    extract::{Request, State},
    middleware::Next,
    response::{IntoResponse, Response},
};
use jsonwebtoken::{DecodingKey, Validation, decode, decode_header};
use openidconnect::{IssuerUrl, core::CoreProviderMetadata};
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::audit::{AuditEvent, log_audit_event};
use crate::problem::ProblemResponse;
use config::{AuthConfig, DiscoveryMode, RoleClaimSource};
use svc::{OidcUserInfo, ProvisioningPolicy, UserService, UserServiceTrait};

#[derive(Debug, Clone)]
pub enum AuthFailure {
    Unauthorized(String),
    Forbidden(String),
    Internal(String),
}

impl std::fmt::Display for AuthFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthFailure::Unauthorized(detail) => write!(f, "Unauthorized: {detail}"),
            AuthFailure::Forbidden(detail) => write!(f, "Forbidden: {detail}"),
            AuthFailure::Internal(detail) => write!(f, "Internal: {detail}"),
        }
    }
}

impl IntoResponse for AuthFailure {
    fn into_response(self) -> Response {
        let response = match self {
            AuthFailure::Unauthorized(detail) => ProblemResponse::unauthorized(detail),
            AuthFailure::Forbidden(detail) => ProblemResponse::forbidden(detail),
            AuthFailure::Internal(detail) => ProblemResponse::internal_error(detail),
        };
        response.into_response()
    }
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: uuid::Uuid,
    pub email: String,
    pub display_name: String,
    pub role: String,
    pub sub: String,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct JwkKey {
    kid: String,
    kty: String,
    #[serde(default)]
    alg: Option<String>,
    n: String,
    e: String,
    #[serde(default)]
    r#use: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JwksResponse {
    keys: Vec<JwkKey>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Claims {
    pub sub: String,
    #[serde(default)]
    pub iss: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub preferred_username: Option<String>,
    #[serde(default)]
    pub email_verified: Option<bool>,
    #[serde(default)]
    pub roles: Option<Vec<String>>,
    #[serde(default)]
    pub groups: Option<Vec<String>>,
}

pub struct OidcValidator {
    config: AuthConfig,
    jwks_cache: Mutex<(Vec<JwkKey>, Instant)>,
    metadata_cache: Mutex<Option<(CoreProviderMetadata, Instant)>>,
    http_client: reqwest::Client,
}

impl OidcValidator {
    pub fn new(config: AuthConfig) -> Self {
        let mut http_builder = reqwest::Client::builder().timeout(Duration::from_secs(10));

        if config.danger_accept_invalid_certs {
            http_builder = http_builder.danger_accept_invalid_certs(true);
            tracing::warn!(
                "OIDC: TLS certificate verification disabled (danger_accept_invalid_certs=true)"
            );
        }

        let http_client = http_builder.build().unwrap_or_else(|e| {
            tracing::error!("Failed to build OIDC HTTP client: {e}");
            std::process::exit(1);
        });

        Self {
            config,
            jwks_cache: Mutex::new((vec![], Instant::now())),
            metadata_cache: Mutex::new(None),
            http_client,
        }
    }

    pub fn auth_enabled(&self) -> bool {
        self.config.enabled
    }

    pub async fn authenticate_token(
        &self,
        token: &str,
        svc: &UserService,
        provisioning: &ProvisioningPolicy,
    ) -> Result<AuthUser, AuthFailure> {
        let jwks = self.get_jwks().await.map_err(|e| {
            AuthFailure::Unauthorized(format!("Failed to retrieve JWKS for token validation: {e}"))
        })?;

        let claims = self
            .validate_token(token, &jwks)
            .map_err(|e| AuthFailure::Unauthorized(format!("Invalid or expired JWT token: {e}")))?;

        let user_info = self.extract_user_info(&claims).map_err(|e| {
            AuthFailure::Unauthorized(format!("Failed to extract required claims from token: {e}"))
        })?;

        let user = svc.provision_user(&user_info, provisioning).await.map_err(|e| {
            tracing::warn!("User lookup/creation failed: {e}");
            AuthFailure::Forbidden(
                "User provisioning failed — email may not be in allowed domains, or identity could not be resolved"
                    .to_owned(),
            )
        })?;

        Ok(AuthUser {
            user_id: user.id,
            email: user.email,
            display_name: user.display_name,
            role: user.role,
            sub: claims.sub,
        })
    }

    // --- internal helpers ---

    async fn get_jwks(&self) -> Result<Vec<JwkKey>, AuthFailure> {
        let cache_ttl = Duration::from_secs(self.config.jwks_cache_duration_secs);
        {
            let cache = self.jwks_cache.lock().await;
            if !cache.0.is_empty() && cache.1.elapsed() < cache_ttl {
                return Ok(cache.0.clone());
            }
        }

        let jwks_uri = self
            .resolve_jwks_uri()
            .await
            .map_err(|e| AuthFailure::Internal(format!("Failed to resolve JWKS URI: {e}")))?;
        let response = self
            .http_client
            .get(&jwks_uri)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| {
                tracing::warn!("Failed to fetch JWKS: {e}");
                AuthFailure::Unauthorized(format!("Failed to fetch JWKS: {e}"))
            })?
            .json::<JwksResponse>()
            .await
            .map_err(|e| {
                tracing::warn!("Failed to parse JWKS response: {e}");
                AuthFailure::Internal(format!("Failed to parse JWKS response: {e}"))
            })?;

        let mut cache = self.jwks_cache.lock().await;
        *cache = (response.keys.clone(), Instant::now());
        Ok(response.keys)
    }

    async fn discover_provider_metadata(&self) -> Result<CoreProviderMetadata, AuthFailure> {
        let cache_ttl = Duration::from_secs(self.config.jwks_cache_duration_secs);
        {
            let cache = self.metadata_cache.lock().await;
            if let Some((ref metadata, cached_at)) = *cache
                && cached_at.elapsed() < cache_ttl
            {
                return Ok(metadata.clone());
            }
        }

        let issuer_url = IssuerUrl::new(self.config.issuer_url.clone()).map_err(|e| {
            tracing::warn!("Invalid issuer URL: {e}");
            AuthFailure::Internal(format!("Invalid issuer URL: {e}"))
        })?;

        let discovery_url = format!(
            "{}/.well-known/openid-configuration",
            issuer_url.as_str().trim_end_matches('/')
        );

        let metadata = self
            .http_client
            .get(&discovery_url)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| {
                tracing::warn!("Failed to fetch OIDC discovery: {e}");
                AuthFailure::Unauthorized(format!("Failed to fetch OIDC discovery: {e}"))
            })?
            .json::<CoreProviderMetadata>()
            .await
            .map_err(|e| {
                tracing::warn!("Failed to parse OIDC discovery: {e}");
                AuthFailure::Internal(format!("Failed to parse OIDC discovery: {e}"))
            })?;

        let mut cache = self.metadata_cache.lock().await;
        *cache = Some((metadata.clone(), Instant::now()));

        Ok(metadata)
    }

    async fn resolve_jwks_uri(&self) -> Result<String, AuthFailure> {
        match self.config.discovery_mode {
            DiscoveryMode::Manual => self
                .config
                .manual_endpoints
                .as_ref()
                .map(|e| e.jwks_uri.clone())
                .ok_or_else(|| {
                    AuthFailure::Internal(
                        "JWKS URI not configured for manual discovery mode".to_owned(),
                    )
                }),
            DiscoveryMode::Discovery => {
                let metadata = self.discover_provider_metadata().await?;
                Ok(metadata.jwks_uri().to_string())
            }
        }
    }

    fn validate_token(&self, token: &str, jwks: &[JwkKey]) -> Result<Claims, AuthFailure> {
        let header = decode_header(token).map_err(|e| {
            tracing::warn!("Failed to decode JWT header: {e}");
            AuthFailure::Unauthorized(format!("Failed to decode JWT header: {e}"))
        })?;

        let kid = header.kid.ok_or_else(|| {
            tracing::warn!("JWT missing kid claim");
            AuthFailure::Unauthorized("JWT missing kid claim".to_owned())
        })?;

        let jwk = jwks.iter().find(|k| k.kid == kid).ok_or_else(|| {
            tracing::warn!("JWK with kid={kid} not found in JWKS");
            AuthFailure::Unauthorized(format!("JWK with kid={kid} not found in JWKS"))
        })?;

        let decoding_key = DecodingKey::from_rsa_components(&jwk.n, &jwk.e).map_err(|e| {
            tracing::warn!("Failed to construct decoding key: {e}");
            AuthFailure::Internal(format!("Failed to construct decoding key: {e}"))
        })?;

        let mut validation = Validation::new(header.alg);
        validation.set_audience(&self.config.audience);
        validation.set_issuer(&[self.resolve_issuer()]);
        validation.set_required_spec_claims(&["exp", "iss", "sub"]);

        let token_data = decode::<Claims>(token, &decoding_key, &validation).map_err(|e| {
            tracing::warn!("JWT validation failed: {e}");
            AuthFailure::Unauthorized(format!("JWT validation failed: {e}"))
        })?;

        Ok(token_data.claims)
    }

    fn resolve_issuer(&self) -> String {
        match self.config.discovery_mode {
            DiscoveryMode::Manual => self
                .config
                .manual_endpoints
                .as_ref()
                .map(|e| e.issuer.clone())
                .unwrap_or_else(|| self.config.issuer_url.clone()),
            DiscoveryMode::Discovery => self.config.issuer_url.clone(),
        }
    }

    fn extract_user_info(&self, claims: &Claims) -> Result<OidcUserInfo, AuthFailure> {
        let roles = self.extract_roles(claims);
        let issuer = claims.iss.clone().unwrap_or_else(|| self.resolve_issuer());
        let email = claims.email.clone().ok_or_else(|| {
            tracing::warn!("JWT missing required email claim");
            AuthFailure::Unauthorized("JWT missing required email claim".to_owned())
        })?;
        Ok(OidcUserInfo {
            sub: claims.sub.clone(),
            issuer,
            email,
            name: claims
                .name
                .clone()
                .or_else(|| claims.preferred_username.clone())
                .unwrap_or_else(|| claims.email.clone().unwrap_or_default()),
            email_verified: claims.email_verified.unwrap_or(false),
            roles,
        })
    }

    fn extract_roles(&self, claims: &Claims) -> Vec<String> {
        match self.config.role_claim_source {
            RoleClaimSource::Roles => claims.roles.clone().unwrap_or_default(),
            RoleClaimSource::Groups => claims.groups.clone().unwrap_or_default(),
        }
    }
}

pub struct AppState {
    pub svc: Arc<UserService>,
    pub oidc: Arc<OidcValidator>,
    pub provisioning: ProvisioningPolicy,
}

pub async fn oidc_middleware(
    State(state): State<Arc<AppState>>,
    mut req: Request,
    next: Next,
) -> Result<Response, AuthFailure> {
    if !state.oidc.auth_enabled() {
        return Ok(next.run(req).await);
    }

    let token = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| {
            log_audit_event(&AuditEvent::AuthFailure {
                reason: "Missing or invalid Bearer token".to_owned(),
            });
            AuthFailure::Unauthorized("Missing or invalid Bearer token".to_owned())
        })?;

    let auth_user = state
        .oidc
        .authenticate_token(token, state.svc.as_ref(), &state.provisioning)
        .await
        .map_err(|e| {
            log_audit_event(&AuditEvent::AuthFailure {
                reason: format!("{e:?}"),
            });
            e
        })?;

    log_audit_event(&AuditEvent::AuthSuccess {
        user_id: auth_user.user_id,
        email: auth_user.email.clone(),
        role: auth_user.role.clone(),
        sub: auth_user.sub.clone(),
    });

    req.extensions_mut().insert(auth_user);

    Ok(next.run(req).await)
}
