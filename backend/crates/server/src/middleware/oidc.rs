use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::{IntoResponse, Response},
};
use jsonwebtoken::{DecodingKey, Validation, decode, decode_header};
use openidconnect::{IssuerUrl, core::CoreProviderMetadata};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::audit::{AuditEvent, log_audit_event};
use crate::problem::ProblemResponse;
use crate::state::AppState;
use config::{AuthConfig, DiscoveryMode, RoleClaimSource};
use repo::UserRepo;
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

#[derive(Debug, Clone, Deserialize, Serialize)]
struct JwkKey {
    kid: String,
    n: String,
    e: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JwksResponse {
    keys: Vec<JwkKey>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
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

/// Port for OIDC HTTP operations. Production uses reqwest; tests inject a fake.
#[async_trait]
pub trait OidcHttpClient: Send + Sync {
    async fn fetch_jwks(&self, uri: &str) -> Result<JwksResponse, String>;
    async fn fetch_metadata(&self, url: &str) -> Result<CoreProviderMetadata, String>;
}

pub struct ReqwestOidcClient {
    client: reqwest::Client,
}

#[async_trait]
impl OidcHttpClient for ReqwestOidcClient {
    async fn fetch_jwks(&self, uri: &str) -> Result<JwksResponse, String> {
        self.client
            .get(uri)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| format!("Failed to fetch JWKS: {e}"))?
            .json::<JwksResponse>()
            .await
            .map_err(|e| format!("Failed to parse JWKS: {e}"))
    }

    async fn fetch_metadata(&self, url: &str) -> Result<CoreProviderMetadata, String> {
        self.client
            .get(url)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| format!("Failed to fetch OIDC discovery: {e}"))?
            .json::<CoreProviderMetadata>()
            .await
            .map_err(|e| format!("Failed to parse OIDC discovery: {e}"))
    }
}

pub struct OidcValidator {
    config: AuthConfig,
    jwks_cache: Mutex<(Vec<JwkKey>, Instant)>,
    metadata_cache: Mutex<Option<(CoreProviderMetadata, Instant)>>,
    http_client: Arc<dyn OidcHttpClient>,
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

        let client = http_builder.build().unwrap_or_else(|e| {
            tracing::error!("Failed to build OIDC HTTP client: {e}");
            std::process::exit(1);
        });

        Self::with_client(config, Arc::new(ReqwestOidcClient { client }))
    }

    pub fn with_client(config: AuthConfig, http_client: Arc<dyn OidcHttpClient>) -> Self {
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

    pub async fn authenticate_token<R: UserRepo>(
        &self,
        token: &str,
        svc: &UserService<R>,
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

        let response = self.http_client.fetch_jwks(&jwks_uri).await.map_err(|e| {
            tracing::warn!("Failed to fetch JWKS: {e}");
            AuthFailure::Unauthorized(format!("Failed to fetch JWKS: {e}"))
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
            .fetch_metadata(&discovery_url)
            .await
            .map_err(|e| {
                tracing::warn!("Failed to fetch OIDC discovery: {e}");
                AuthFailure::Unauthorized(format!("Failed to fetch OIDC discovery: {e}"))
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

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{EncodingKey, Header};
    use repo::MockUserRepo;
    use svc::UserService;

    // Pre-generated RSA private key for test JWT signing.
    const TEST_PRIVATE_KEY_PEM: &str = r"-----BEGIN PRIVATE KEY-----
MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQDC/1W/9r+GPki7
JzRZk/vzyaljJ9AQW35Xl6HiZhrx+njrbFO69Yz/CzxRO/ZTAVetK/Hy/qqmpTeZ
faBVIT1T2xSuAGy4YwfePunfFyGe5Nne7/ZAtfqd9JgmaApNKEyefMCHwNTl/6rV
yXiBLmo9jpEBHOLeApgYgttMPPxbmpWDz7tZV5uHUzQ3v72dHqwedmk5ctef8p6S
uGlFfCNAuTRh3MFrK2wAAm7Egag2yDZHgw56WZTROKJGlS3hnOuaN/WuhHGAclkU
2wrXNzVNcm+lY6Gt4kGfqnlA/a5QcYfT+J4ys23NI/vKV8o02G+1LLN7gVIfptOV
lFaeB7PnAgMBAAECggEAFvL4A5Slq8Xux1/w0E4TS/jC37GS0ioCb4qf+cYsi6f/
zS09mXZGtsl6utDEx7YTvIS8f+2q5dvx9DWNxhcYYaMaBvRz1yuIhIaA/cl6Inoq
jgtNvwXrzeU4duStubDxe4GRIyj1pW/2ihyg+wscY8xAkpH0vU4u2kukRY+z34/X
6ZtaEitKWRTKkQevy0AK90175yz5RnX+B8YsKXiXDNfuiHbDRzjd3WcCHp0I61B+
McL0MwT5SNjJHJ4KgdnQg/KhI4zqMzEJlZRP1M8yYCRDhljE3ogrLeKj6TDxsnHZ
yZiutXGkW/G4uwGyQVg2KeIeuW1jbEREBP/9ukfvmQKBgQD8Ersz/XxZgno2fC+l
Gc0WIYJoqCN8kjuPzq8EOGLt8wY+68MOs51nCwbxjY/vzm/QMoijHofjJagJnR6o
ej6cpFyLChhN5BXpcul+9lc0OObHIOCVa1TG/vlJH7GbcG0TDk/Ys56Bfh6rGAXe
EoJKUvGCqjzJd5SRNk6lpsJzAwKBgQDGCPw+a+imV5ZjmdUr5gAHdKg4klfN6Yc/
KmPf714pRipVVg+/3GzCZpCyf491SMzqGy4Qv7epk91ErRNgVVXMqVkB4uszkm3e
XppfuxHiPB7vwgEqWp1akE1KbDai4G5CrCc3zRF6T/KKQh8IR0ukEcz6M1ewmK5g
/5VCKo+0TQKBgDtqzuW6Yo1WzCf7rd9k7YrB61NwDq1WauPC/E4qXGdqWZaLTDcy
729SkhhuHfeQ2ZphzwNkNvO79JgPiSJ9bjTOKWI8xu3VTbTxeUiMGJIet4dIoKeX
3Svq/ifWaK8TGSLcxMV30W0EpYX+66MlCcErO/Jo3ls+7K4t9uvlGwCxAoGAXlzR
tPy+MuTxaCxZAz3pLaRMtAgnhpqMM3EDfiUU/R3N9jO39XgW6trsf+GAfiZeXf7t
iFWSMl+ew6ak4PZIl+jp3Jg/8qcHazow3QTKlL6JDz2mSIQ9DnZMHfZKYnoGkAZv
+YrhlSKbM34mQ0+Vn2xL+7yTQDbAgL/IY2rcZtECgYEAybFs0EYFfzKDlLAbNmem
vUdbZDSAyeUKXD9ms8b/tRsEi9VQnn8EuYAwnJKlIvCgczwkroB2Qm6KEsSqSyoS
pJihLAeFBtyl9MCGyq8p9uhyAEDPDVGXe2gA/CyXTZfMb5glVdbuIvtBEJsxER+g
Ayt5d8YaALp3owVyEfJ3Uok=
-----END PRIVATE KEY-----";

    fn test_jwks() -> Vec<JwkKey> {
        vec![JwkKey {
            kid: "test-key-1".to_owned(),
            n: "wv9Vv_a_hj5Iuyc0WZP788mpYyfQEFt-V5eh4mYa8fp462xTuvWM_ws8UTv2UwFXrSvx8v6qpqU3mX2gVSE9U9sUrgBsuGMH3j7p3xchnuTZ3u_2QLX6nfSYJmgKTShMnnzAh8DU5f-q1cl4gS5qPY6RARzi3gKYGILbTDz8W5qVg8-7WVebh1M0N7-9nR6sHnZpOXLXn_KekrhpRXwjQLk0YdzBaytsAAJuxIGoNsg2R4MOelmU0TiiRpUt4Zzrmjf1roRxgHJZFNsK1zc1TXJvpWOhreJBn6p5QP2uUHGH0_ieMrNtzSP7ylfKNNhvtSyze4FSH6bTlZRWngez5w".to_owned(),
            e: "AQAB".to_owned(),
        }]
    }

    fn encode_token(claims: &Claims) -> String {
        let key = EncodingKey::from_rsa_pem(TEST_PRIVATE_KEY_PEM.as_bytes()).unwrap();
        let mut header = Header::new(jsonwebtoken::Algorithm::RS256);
        header.kid = Some("test-key-1".to_owned());
        jsonwebtoken::encode(&header, claims, &key).unwrap()
    }

    fn encode_token_with_exp(claims: &Claims, exp: i64) -> String {
        let key = EncodingKey::from_rsa_pem(TEST_PRIVATE_KEY_PEM.as_bytes()).unwrap();
        let mut header = Header::new(jsonwebtoken::Algorithm::RS256);
        header.kid = Some("test-key-1".to_owned());
        let mut map = serde_json::to_value(claims).unwrap();
        let obj = map.as_object_mut().unwrap();
        obj.insert("exp".to_owned(), serde_json::json!(exp));
        obj.insert("aud".to_owned(), serde_json::json!("test-audience"));
        jsonwebtoken::encode(&header, &map, &key).unwrap()
    }

    fn test_config() -> AuthConfig {
        AuthConfig {
            enabled: true,
            issuer_url: "https://test-issuer.example.com".to_owned(),
            audience: vec!["test-audience".to_owned()],
            allowed_email_domains: vec![],
            role_claim_source: RoleClaimSource::Roles,
            discovery_mode: DiscoveryMode::Manual,
            manual_endpoints: Some(config::ManualOidcEndpoints {
                jwks_uri: "https://test-issuer.example.com/jwks".to_owned(),
                issuer: "https://test-issuer.example.com".to_owned(),
                authorization_endpoint: None,
                token_endpoint: None,
            }),
            jwks_cache_duration_secs: 3600,
            danger_accept_invalid_certs: false,
        }
    }

    fn valid_claims() -> Claims {
        Claims {
            sub: "user-123".to_owned(),
            iss: Some("https://test-issuer.example.com".to_owned()),
            email: Some("test@example.com".to_owned()),
            name: Some("Test User".to_owned()),
            preferred_username: None,
            email_verified: Some(true),
            roles: Some(vec!["user".to_owned()]),
            groups: None,
        }
    }

    struct MockOidcClient {
        jwks: JwksResponse,
    }

    #[async_trait]
    impl OidcHttpClient for MockOidcClient {
        async fn fetch_jwks(&self, _uri: &str) -> Result<JwksResponse, String> {
            Ok(self.jwks.clone())
        }

        async fn fetch_metadata(&self, _url: &str) -> Result<CoreProviderMetadata, String> {
            Err("Discovery not configured in tests".to_owned())
        }
    }

    #[tokio::test]
    async fn validate_token_should_succeed_with_valid_jwt() {
        let config = test_config();
        let jwks = test_jwks();
        let client = Arc::new(MockOidcClient {
            jwks: JwksResponse { keys: jwks.clone() },
        });
        let validator = OidcValidator::with_client(config, client);

        let claims = valid_claims();
        let now = jsonwebtoken::get_current_timestamp();
        let token = encode_token_with_exp(&claims, (now + 3600) as i64);

        let result = validator.validate_token(&token, &jwks);
        assert!(
            result.is_ok(),
            "Expected success but got: {:?}",
            result.err()
        );
        let decoded = result.unwrap();
        assert_eq!(decoded.sub, "user-123");
        assert_eq!(decoded.email, Some("test@example.com".to_owned()));
    }

    #[tokio::test]
    async fn validate_token_should_fail_with_invalid_kid() {
        let config = test_config();
        let mut jwks = test_jwks();
        jwks[0].kid = "wrong-kid".to_owned();
        let client = Arc::new(MockOidcClient {
            jwks: JwksResponse { keys: jwks.clone() },
        });
        let validator = OidcValidator::with_client(config, client);

        let claims = valid_claims();
        let token = encode_token(&claims);

        let result = validator.validate_token(&token, &jwks);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("kid"));
    }

    #[tokio::test]
    async fn validate_token_should_fail_with_expired_jwt() {
        let config = test_config();
        let jwks = test_jwks();
        let client = Arc::new(MockOidcClient {
            jwks: JwksResponse { keys: jwks.clone() },
        });
        let validator = OidcValidator::with_client(config, client);

        let claims = valid_claims();
        // Encode a token with an exp in the past so validation fails.
        let token = encode_token_with_exp(&claims, 1);

        let result = validator.validate_token(&token, &jwks);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("ExpiredSignature"),
            "Expected error to mention 'ExpiredSignature', got: {}",
            err
        );
    }

    #[tokio::test]
    async fn extract_user_info_should_extract_email_and_roles() {
        let config = test_config();
        let client = Arc::new(MockOidcClient {
            jwks: JwksResponse { keys: vec![] },
        });
        let validator = OidcValidator::with_client(config, client);

        let claims = valid_claims();
        let info = validator.extract_user_info(&claims).unwrap();

        assert_eq!(info.sub, "user-123");
        assert_eq!(info.email, "test@example.com");
        assert_eq!(info.name, "Test User");
        assert_eq!(info.roles, vec!["user"]);
        assert!(info.email_verified);
    }

    #[tokio::test]
    async fn extract_user_info_should_fallback_to_preferred_username() {
        let config = test_config();
        let client = Arc::new(MockOidcClient {
            jwks: JwksResponse { keys: vec![] },
        });
        let validator = OidcValidator::with_client(config, client);

        let mut claims = valid_claims();
        claims.name = None;
        claims.preferred_username = Some("prefuser".to_owned());
        let info = validator.extract_user_info(&claims).unwrap();

        assert_eq!(info.name, "prefuser");
    }

    #[tokio::test]
    async fn extract_user_info_should_reject_missing_email() {
        let config = test_config();
        let client = Arc::new(MockOidcClient {
            jwks: JwksResponse { keys: vec![] },
        });
        let validator = OidcValidator::with_client(config, client);

        let mut claims = valid_claims();
        claims.email = None;
        let result = validator.extract_user_info(&claims);

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn extract_roles_should_use_groups_when_configured() {
        let mut config = test_config();
        config.role_claim_source = RoleClaimSource::Groups;
        let client = Arc::new(MockOidcClient {
            jwks: JwksResponse { keys: vec![] },
        });
        let validator = OidcValidator::with_client(config, client);

        let mut claims = valid_claims();
        claims.roles = Some(vec!["admin".to_owned()]);
        claims.groups = Some(vec!["manager".to_owned()]);

        let roles = validator.extract_roles(&claims);
        assert_eq!(roles, vec!["manager"]);
    }

    #[tokio::test]
    async fn authenticate_token_should_provision_user() {
        let config = test_config();
        let jwks = test_jwks();
        let client = Arc::new(MockOidcClient {
            jwks: JwksResponse { keys: jwks.clone() },
        });
        let validator = OidcValidator::with_client(config, client);

        let repo = MockUserRepo::new();
        let svc = UserService::new(repo);
        let policy = ProvisioningPolicy::new(vec![], "user".to_owned());

        // Build a token with exp so validation passes.
        let claims = valid_claims();
        let now = jsonwebtoken::get_current_timestamp();
        let token = encode_token_with_exp(&claims, (now + 3600) as i64);

        let result = validator.authenticate_token(&token, &svc, &policy).await;
        assert!(
            result.is_ok(),
            "Expected success but got: {:?}",
            result.err()
        );
        let auth_user = result.unwrap();
        assert_eq!(auth_user.email, "test@example.com");
        assert_eq!(auth_user.role, "user");
    }
}
