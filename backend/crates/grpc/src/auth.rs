use config::DiscoveryMode;
use jsonwebtoken::{DecodingKey, Validation, decode, decode_header};
use serde::Deserialize;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tonic::{Request, Status};

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

#[derive(Debug, Clone, Deserialize)]
pub struct JwksResponse {
    pub keys: Vec<serde_json::Value>,
}

/// Holds the configuration and a synchronously-readable JWKS cache
/// for gRPC service-to-service JWT validation.
pub struct GrpcAuthState {
    config: config::AuthConfig,
    jwks: std::sync::RwLock<(Vec<serde_json::Value>, Instant)>,
    client: reqwest::Client,
}

impl GrpcAuthState {
    pub fn new(config: config::AuthConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();
        Self {
            config,
            jwks: std::sync::RwLock::new((vec![], Instant::now())),
            client,
        }
    }

    /// Resolve the expected issuer string from config.
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

    /// Resolve the JWKS URI from config (manual or discovery).
    fn resolve_jwks_uri(&self) -> Result<String, Status> {
        match self.config.discovery_mode {
            DiscoveryMode::Manual => self
                .config
                .manual_endpoints
                .as_ref()
                .map(|e| e.jwks_uri.clone())
                .ok_or_else(|| {
                    Status::internal("JWKS URI not configured for manual discovery mode")
                }),
            DiscoveryMode::Discovery => Err(Status::internal(
                "JWKS cache miss in discovery mode; cache should be primed at startup",
            )),
        }
    }

    /// Asynchronously fetch JWKS and update the cache.
    pub async fn refresh_jwks(&self) -> Result<(), Status> {
        let uri = self.resolve_jwks_uri()?;
        let response = self
            .client
            .get(&uri)
            .send()
            .await
            .map_err(|e| Status::internal(format!("Failed to fetch JWKS: {e}")))?;
        let jwks: JwksResponse = response
            .json()
            .await
            .map_err(|e| Status::internal(format!("Failed to parse JWKS: {e}")))?;
        let mut cache = self.jwks.write().unwrap();
        *cache = (jwks.keys, Instant::now());
        Ok(())
    }

    /// Prime the cache during startup. For discovery mode, this fetches
    /// the OIDC discovery document first, then the JWKS.
    pub async fn prime_cache(&self) -> Result<(), Status> {
        let jwks_uri = match self.config.discovery_mode {
            DiscoveryMode::Manual => self
                .config
                .manual_endpoints
                .as_ref()
                .map(|e| e.jwks_uri.clone())
                .ok_or_else(|| {
                    Status::internal("JWKS URI not configured for manual discovery mode")
                })?,
            DiscoveryMode::Discovery => {
                let discovery_url = format!(
                    "{}/.well-known/openid-configuration",
                    self.config.issuer_url.trim_end_matches('/')
                );
                let meta: serde_json::Value = self
                    .client
                    .get(&discovery_url)
                    .send()
                    .await
                    .map_err(|e| Status::internal(format!("Failed to fetch OIDC discovery: {e}")))?
                    .json()
                    .await
                    .map_err(|e| {
                        Status::internal(format!("Failed to parse OIDC discovery: {e}"))
                    })?;
                meta.get("jwks_uri")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_owned())
                    .ok_or_else(|| Status::internal("OIDC discovery missing jwks_uri"))?
            }
        };

        let response = self
            .client
            .get(&jwks_uri)
            .send()
            .await
            .map_err(|e| Status::internal(format!("Failed to fetch JWKS: {e}")))?;
        let jwks: JwksResponse = response
            .json()
            .await
            .map_err(|e| Status::internal(format!("Failed to parse JWKS: {e}")))?;
        let mut cache = self.jwks.write().unwrap();
        *cache = (jwks.keys, Instant::now());
        Ok(())
    }

    /// Synchronously validate a JWT using the cached JWKS.
    pub fn validate_token(&self, token: &str) -> Result<Claims, Status> {
        let header = decode_header(token)
            .map_err(|e| Status::unauthenticated(format!("Failed to decode JWT header: {e}")))?;

        let kid = header
            .kid
            .ok_or_else(|| Status::unauthenticated("JWT missing kid claim".to_owned()))?;

        let cache = self.jwks.read().unwrap();
        let jwk_value = cache
            .0
            .iter()
            .find(|k| {
                k.get("kid")
                    .and_then(|v| v.as_str())
                    .is_some_and(|v| v == kid)
            })
            .ok_or_else(|| {
                Status::unauthenticated(format!("JWK with kid={kid} not found in JWKS"))
            })?;

        let jwk: jsonwebtoken::jwk::Jwk = serde_json::from_value(jwk_value.clone())
            .map_err(|e| Status::internal(format!("Failed to parse JWK: {e}")))?;

        let decoding_key = DecodingKey::from_jwk(&jwk)
            .map_err(|e| Status::internal(format!("Failed to construct decoding key: {e}")))?;

        let alg_str = format!("{:?}", header.alg);
        if !self
            .config
            .allowed_algorithms
            .iter()
            .any(|a| a.eq_ignore_ascii_case(&alg_str))
        {
            return Err(Status::unauthenticated(format!(
                "JWT algorithm {alg_str} is not allowed"
            )));
        }

        let mut validation = Validation::new(header.alg);
        validation.set_audience(&self.config.audience);
        validation.set_issuer(&[self.resolve_issuer()]);
        validation.set_required_spec_claims(&["exp", "iss", "sub"]);
        validation.leeway = self.config.clock_skew_seconds;

        let token_data = decode::<Claims>(token, &decoding_key, &validation)
            .map_err(|e| Status::unauthenticated(format!("JWT validation failed: {e}")))?;

        if self.config.require_email_verified {
            let verified = token_data.claims.email_verified.unwrap_or(false);
            if !verified {
                return Err(Status::unauthenticated(
                    "Email verification required".to_owned(),
                ));
            }
        }

        Ok(token_data.claims)
    }
}

/// gRPC auth interceptor. When enabled, validates the Bearer token
/// synchronously using a pre-loaded JWKS cache.
#[derive(Clone)]
pub struct GrpcAuthInterceptor {
    pub state: Option<Arc<GrpcAuthState>>,
}

impl tonic::service::Interceptor for GrpcAuthInterceptor {
    fn call(&mut self, request: Request<()>) -> Result<Request<()>, Status> {
        let Some(state) = self.state.as_ref() else {
            return Ok(request);
        };

        let auth_header = request
            .metadata()
            .get("authorization")
            .and_then(|v| v.to_str().ok());

        let token = match auth_header {
            Some(header) if header.starts_with("Bearer ") => &header[7..],
            _ => return Err(Status::unauthenticated("Missing or invalid Bearer token")),
        };

        let claims = state.validate_token(token)?;

        // Attach validated identity to the request metadata so handlers can use it.
        let mut req = request;
        req.metadata_mut().insert(
            "x-auth-sub",
            claims
                .sub
                .parse()
                .unwrap_or_else(|_| "unknown".parse().unwrap()),
        );
        Ok(req)
    }
}
