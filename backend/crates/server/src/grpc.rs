use crate::audit::{NoopExporter, OtelLogsExporter, SyslogExporter};
use crate::middleware::OidcValidator;
use crate::middleware::oidc::{Claims, JwksResponse};
use crate::state::AppState;
use config::{AppConfig, DiscoveryMode};
use jsonwebtoken::{DecodingKey, Validation, decode, decode_header};
use repo::AnyUserRepo;
use std::{
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};
use svc::audit::PiiMode;
use svc::{ProvisioningPolicy, UserService};
use tonic::{Request, Response, Status};

pub mod proto {
    pub mod health {
        pub mod v1 {
            tonic::include_proto!("health.v1");
        }
    }
}

use proto::health::v1::{
    HealthCheckRequest, HealthCheckResponse,
    health_service_server::{HealthService, HealthServiceServer},
};

#[derive(Clone)]
pub struct HealthGrpcService {
    state: Arc<AppState>,
}

impl HealthGrpcService {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }
}

#[tonic::async_trait]
impl HealthService for HealthGrpcService {
    #[tracing::instrument(skip(self, _request))]
    async fn health_check(
        &self,
        _request: Request<HealthCheckRequest>,
    ) -> Result<Response<HealthCheckResponse>, Status> {
        self.state.health.check().await.map_err(|e| {
            tracing::error!("gRPC health check failed: {e}");
            Status::internal("Health check failed")
        })?;

        Ok(Response::new(HealthCheckResponse {
            status: "SERVING".to_owned(),
        }))
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// gRPC JWT Auth
// ═════════════════════════════════════════════════════════════════════════════

/// Holds the configuration and a synchronously-readable JWKS cache
/// for gRPC service-to-service JWT validation.
struct GrpcAuthState {
    config: config::AuthConfig,
    jwks: std::sync::RwLock<(Vec<serde_json::Value>, Instant)>,
    client: reqwest::Client,
}

impl GrpcAuthState {
    fn new(config: config::AuthConfig) -> Self {
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
            DiscoveryMode::Discovery => {
                // Synchronous fetch is not possible here; caller should have
                // pre-fetched JWKS. This path should not be hit if
                // `prime_jwks_cache` was called during startup.
                Err(Status::internal(
                    "JWKS cache miss in discovery mode; cache should be primed at startup",
                ))
            }
        }
    }

    /// Asynchronously fetch JWKS and update the cache.
    async fn refresh_jwks(&self) -> Result<(), Status> {
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
    async fn prime_cache(&self) -> Result<(), Status> {
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
    fn validate_token(&self, token: &str) -> Result<Claims, Status> {
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
    state: Option<Arc<GrpcAuthState>>,
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

pub async fn serve(
    config: AppConfig,
    repo: AnyUserRepo,
    health_checker: Arc<dyn svc::HealthChecker>,
    addr: SocketAddr,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let svc = Arc::new(UserService::new(repo));
    let oidc_validator = Arc::new(OidcValidator::new(config.auth.clone()));
    let provisioning =
        ProvisioningPolicy::new(config.auth.allowed_email_domains.clone(), "user".to_owned());

    let audit_exporter = match config.audit.exporter.as_str() {
        "syslog" => {
            let cfg = config
                .audit
                .syslog
                .as_ref()
                .expect("syslog config validated");
            Arc::new(SyslogExporter::new(
                cfg.host.clone(),
                cfg.port,
                cfg.protocol.clone(),
                &cfg.facility,
                config.observability.service_name.clone(),
                std::env::var("HOSTNAME").unwrap_or_else(|_| "localhost".to_owned()),
                cfg.tls_enabled,
            )) as Arc<dyn svc::AuditExporter>
        }
        "otel-logs" => {
            let cfg = config
                .audit
                .otel_logs
                .as_ref()
                .expect("otel_logs config validated");
            Arc::new(OtelLogsExporter::new(
                cfg.endpoint.clone(),
                cfg.timeout_seconds,
                config.observability.service_name.clone(),
            )) as Arc<dyn svc::AuditExporter>
        }
        _ => Arc::new(NoopExporter) as Arc<dyn svc::AuditExporter>,
    };
    let audit_service = svc::AuditService::new(audit_exporter, PiiMode::Redact);

    let app_state = Arc::new(AppState {
        svc,
        health: Arc::clone(&health_checker),
        oidc: oidc_validator,
        provisioning,
        audit: audit_service,
    });

    // Build gRPC auth interceptor with real JWT validation when enabled.
    let auth_state = if config.grpc.auth_enabled {
        let state = Arc::new(GrpcAuthState::new(config.auth.clone()));
        state
            .prime_cache()
            .await
            .map_err(|e| format!("Failed to prime gRPC JWKS cache: {e}"))?;

        // Spawn background JWKS refresh.
        let refresh_state = Arc::clone(&state);
        let cache_ttl = Duration::from_secs(config.auth.jwks_cache_duration_secs);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(cache_ttl).await;
                if let Err(e) = refresh_state.refresh_jwks().await {
                    tracing::warn!("gRPC JWKS background refresh failed: {e}");
                }
            }
        });

        Some(state)
    } else {
        None
    };

    let auth_interceptor = GrpcAuthInterceptor { state: auth_state };

    let listener = tokio::net::TcpListener::bind(addr).await?;

    let health_svc = HealthServiceServer::with_interceptor(
        HealthGrpcService::new(app_state),
        auth_interceptor.clone(),
    );

    let mut server_builder = tonic::transport::Server::builder();

    if config.grpc.tls.enabled {
        let cert = tokio::fs::read(&config.grpc.tls.cert_path)
            .await
            .map_err(|e| {
                format!(
                    "Failed to read gRPC TLS cert from '{}': {e}",
                    config.grpc.tls.cert_path
                )
            })?;
        let key = tokio::fs::read(&config.grpc.tls.key_path)
            .await
            .map_err(|e| {
                format!(
                    "Failed to read gRPC TLS key from '{}': {e}",
                    config.grpc.tls.key_path
                )
            })?;
        let identity = tonic::transport::Identity::from_pem(cert, key);

        let tls_config = if let Some(ref ca_path) = config.grpc.tls.ca_cert_path {
            let ca = tokio::fs::read(ca_path)
                .await
                .map_err(|e| format!("Failed to read gRPC TLS CA from '{ca_path}': {e}"))?;
            let ca_cert = tonic::transport::Certificate::from_pem(ca);
            tonic::transport::ServerTlsConfig::new()
                .identity(identity)
                .client_ca_root(ca_cert)
        } else {
            tonic::transport::ServerTlsConfig::new().identity(identity)
        };

        server_builder = server_builder.tls_config(tls_config)?;
    }

    server_builder
        .add_service(health_svc)
        .serve_with_incoming_shutdown(
            tokio_stream::wrappers::TcpListenerStream::new(listener),
            async {
                tokio::signal::ctrl_c().await.ok();
                tracing::info!("gRPC server graceful shutdown");
            },
        )
        .await?;

    Ok(())
}
