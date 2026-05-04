use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{DecodingKey, Validation, decode, decode_header};
use openidconnect::{IssuerUrl, core::CoreProviderMetadata};
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::problem::ProblemResponse;
use config::{AuthConfig, DiscoveryMode, RoleClaimSource};
use svc::{OidcUserInfo, ProvisioningPolicy, UserService, UserServiceTrait};

#[derive(Debug, Clone)]
pub enum AuthFailure {
    Unauthorized(String),
    Forbidden(String),
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
        let jwks = self.get_jwks().await.map_err(|_| {
            AuthFailure::Unauthorized("Failed to retrieve JWKS for token validation".to_owned())
        })?;

        let claims = self
            .validate_token(token, &jwks)
            .map_err(|_| AuthFailure::Unauthorized("Invalid or expired JWT token".to_owned()))?;

        let user_info = self.extract_user_info(&claims).map_err(|_| {
            AuthFailure::Unauthorized("Failed to extract required claims from token".to_owned())
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

    async fn get_jwks(&self) -> Result<Vec<JwkKey>, StatusCode> {
        let cache_ttl = Duration::from_secs(self.config.jwks_cache_duration_secs);
        {
            let cache = self.jwks_cache.lock().await;
            if !cache.0.is_empty() && cache.1.elapsed() < cache_ttl {
                return Ok(cache.0.clone());
            }
        }

        let jwks_uri = self.resolve_jwks_uri().await?;
        let response = self
            .http_client
            .get(&jwks_uri)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| {
                tracing::warn!("Failed to fetch JWKS: {e}");
                StatusCode::UNAUTHORIZED
            })?
            .json::<JwksResponse>()
            .await
            .map_err(|e| {
                tracing::warn!("Failed to parse JWKS response: {e}");
                StatusCode::UNAUTHORIZED
            })?;

        let mut cache = self.jwks_cache.lock().await;
        *cache = (response.keys.clone(), Instant::now());
        Ok(response.keys)
    }

    async fn discover_provider_metadata(&self) -> Result<CoreProviderMetadata, StatusCode> {
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
            StatusCode::INTERNAL_SERVER_ERROR
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
                StatusCode::UNAUTHORIZED
            })?
            .json::<CoreProviderMetadata>()
            .await
            .map_err(|e| {
                tracing::warn!("Failed to parse OIDC discovery: {e}");
                StatusCode::UNAUTHORIZED
            })?;

        let mut cache = self.metadata_cache.lock().await;
        *cache = Some((metadata.clone(), Instant::now()));

        Ok(metadata)
    }

    async fn resolve_jwks_uri(&self) -> Result<String, StatusCode> {
        match self.config.discovery_mode {
            DiscoveryMode::Manual => self
                .config
                .manual_endpoints
                .as_ref()
                .map(|e| e.jwks_uri.clone())
                .ok_or(StatusCode::INTERNAL_SERVER_ERROR),
            DiscoveryMode::Discovery => {
                let metadata = self.discover_provider_metadata().await?;
                Ok(metadata.jwks_uri().to_string())
            }
        }
    }

    fn validate_token(&self, token: &str, jwks: &[JwkKey]) -> Result<Claims, StatusCode> {
        let header = decode_header(token).map_err(|e| {
            tracing::warn!("Failed to decode JWT header: {e}");
            StatusCode::UNAUTHORIZED
        })?;

        let kid = header.kid.ok_or({
            tracing::warn!("JWT missing kid claim");
            StatusCode::UNAUTHORIZED
        })?;

        let jwk = jwks.iter().find(|k| k.kid == kid).ok_or({
            tracing::warn!("JWK with kid={kid} not found in JWKS");
            StatusCode::UNAUTHORIZED
        })?;

        let decoding_key = DecodingKey::from_rsa_components(&jwk.n, &jwk.e).map_err(|e| {
            tracing::warn!("Failed to construct decoding key: {e}");
            StatusCode::UNAUTHORIZED
        })?;

        let mut validation = Validation::new(header.alg);
        validation.set_audience(&self.config.audience);
        validation.set_issuer(&[self.resolve_issuer()]);
        validation.set_required_spec_claims(&["exp", "iss", "sub"]);

        let token_data = decode::<Claims>(token, &decoding_key, &validation).map_err(|e| {
            tracing::warn!("JWT validation failed: {e}");
            StatusCode::UNAUTHORIZED
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

    fn extract_user_info(&self, claims: &Claims) -> Result<OidcUserInfo, StatusCode> {
        let roles = self.extract_roles(claims);
        let issuer = claims.iss.clone().unwrap_or_else(|| self.resolve_issuer());
        let email = claims.email.clone().ok_or_else(|| {
            tracing::warn!("JWT missing required email claim");
            StatusCode::UNAUTHORIZED
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
) -> Result<Response, Response> {
    if !state.oidc.auth_enabled() {
        return Ok(next.run(req).await);
    }

    let token = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| {
            Response::from(ProblemResponse::unauthorized(
                "Missing or invalid Bearer token",
            ))
        })?;

    let auth_user = state
        .oidc
        .authenticate_token(token, state.svc.as_ref(), &state.provisioning)
        .await
        .map_err(auth_failure_to_response)?;

    req.extensions_mut().insert(auth_user);

    Ok(next.run(req).await)
}

fn auth_failure_to_response(error: AuthFailure) -> Response {
    match error {
        AuthFailure::Unauthorized(detail) => Response::from(ProblemResponse::unauthorized(detail)),
        AuthFailure::Forbidden(detail) => Response::from(ProblemResponse::forbidden(detail)),
    }
}
