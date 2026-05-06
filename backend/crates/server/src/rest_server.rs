use crate::audit::{NoopExporter, OtelLogsExporter, SyslogExporter};
use crate::handlers::{health, users};
use crate::middleware::OidcValidator;
use crate::middleware::oidc::oidc_middleware;
use crate::openapi::ApiDoc;
use crate::state::AppState;
use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderName, HeaderValue, Method};
use axum::{Router, middleware as axum_middleware, response::IntoResponse, routing::get};
use axum_prometheus::PrometheusMetricLayer;
use config::AppConfig;
use repo::AnyUserRepo;
use std::sync::Arc;
use std::time::Duration;
use svc::UserService;
use svc::audit::PiiMode;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_governor::{GovernorLayer, governor::GovernorConfigBuilder};
use tower_http::{
    cors::CorsLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    timeout::TimeoutLayer,
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
};
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};
/// Middleware that adds security headers to all responses.
async fn security_headers_middleware(
    axum::extract::State(tls_enabled): axum::extract::State<bool>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let mut response = next.run(req).await;
    let headers = response.headers_mut();
    headers.insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        HeaderName::from_static("x-frame-options"),
        HeaderValue::from_static("DENY"),
    );
    headers.insert(
        HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    headers.insert(
        HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static(
            "accelerometer=(), camera=(), geolocation=(), gyroscope=(), magnetometer=(), microphone=(), payment=(), usb=()",
        ),
    );
    if tls_enabled {
        headers.insert(
            HeaderName::from_static("strict-transport-security"),
            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        );
    }
    response
}
fn create_audit_exporter(
    config: &config::AuditConfig,
    service_name: &str,
) -> Arc<dyn svc::AuditExporter> {
    match config.exporter.as_str() {
        "syslog" => {
            let cfg = config.syslog.as_ref().expect("syslog config validated");
            Arc::new(SyslogExporter::new(
                cfg.host.clone(),
                cfg.port,
                cfg.protocol.clone(),
                &cfg.facility,
                service_name.to_owned(),
                std::env::var("HOSTNAME").unwrap_or_else(|_| "localhost".to_owned()),
                cfg.tls_enabled,
            ))
        }
        "otel-logs" => {
            let cfg = config
                .otel_logs
                .as_ref()
                .expect("otel_logs config validated");
            Arc::new(OtelLogsExporter::new(
                cfg.endpoint.clone(),
                cfg.timeout_seconds,
                service_name.to_owned(),
            ))
        }
        _ => Arc::new(NoopExporter),
    }
}
pub async fn serve_rest(
    config: AppConfig,
    repo: AnyUserRepo,
    health_checker: Arc<dyn svc::HealthChecker>,
    addr: std::net::SocketAddr,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let svc = Arc::new(UserService::new(repo));
    let oidc_validator = Arc::new(OidcValidator::new(config.auth.clone()));
    let provisioning =
        svc::ProvisioningPolicy::new(config.auth.allowed_email_domains.clone(), "user".to_owned());
    let pii_mode = match config.audit.pii_mode {
        config::PiiMode::Full => PiiMode::Full,
        config::PiiMode::Redact => PiiMode::Redact,
    };
    let audit_exporter = create_audit_exporter(&config.audit, &config.observability.service_name);
    let audit_service = svc::AuditService::new(audit_exporter, pii_mode);
    let app_state = Arc::new(AppState {
        svc: svc.clone(),
        health: Arc::clone(&health_checker),
        oidc: oidc_validator,
        provisioning,
        audit: audit_service,
    });
    let cors = CorsLayer::new()
        .allow_origin(
            config
                .server
                .cors_origins
                .iter()
                .map(|o| {
                    o.parse::<axum::http::HeaderValue>().unwrap_or_else(|e| {
                        tracing::error!("Invalid CORS origin '{o}': {e}");
                        std::process::exit(1);
                    })
                })
                .collect::<Vec<_>>(),
        )
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::PATCH,
            Method::OPTIONS,
            Method::HEAD,
        ])
        .allow_headers([
            axum::http::header::AUTHORIZATION,
            axum::http::header::CONTENT_TYPE,
            HeaderName::from_static("x-request-id"),
        ]);
    let api_routes = Router::new()
        .merge(users::routes(app_state.clone()))
        .route_layer(axum_middleware::from_fn_with_state(
            app_state.clone(),
            oidc_middleware,
        ));
    let api_routes = if config.rate_limit.enabled {
        let governor_conf = Arc::new(
            GovernorConfigBuilder::default()
                .per_second(config.rate_limit.requests_per_second.into())
                .burst_size(config.rate_limit.burst_size)
                .finish()
                .expect("valid governor config"),
        );
        api_routes.layer(GovernorLayer {
            config: governor_conf,
        })
    } else {
        api_routes
    };
    let mut app = Router::new()
        .route("/health", get(health::health))
        .route(
            "/health/ready",
            get({
                let health = Arc::clone(&health_checker);
                move || {
                    let health = Arc::clone(&health);
                    async move { health::health_ready(health).await }
                }
            }),
        )
        .nest("/api/v1", api_routes);
    if config.server.docs_enabled {
        app = app.merge(Scalar::with_url("/docs", ApiDoc::openapi()));
    }
    let tls_enabled = config.server.tls.enabled;
    app = app.layer(
        ServiceBuilder::new()
            .layer(axum_middleware::from_fn_with_state(
                tls_enabled,
                security_headers_middleware,
            ))
            .layer(
                TraceLayer::new_for_http()
                    .make_span_with(DefaultMakeSpan::new().level(tracing::Level::INFO))
                    .on_response(DefaultOnResponse::new().level(tracing::Level::INFO)),
            )
            .layer(cors)
            .layer(DefaultBodyLimit::max(config.server.max_request_body_size))
            .layer(TimeoutLayer::with_status_code(
                axum::http::StatusCode::GATEWAY_TIMEOUT,
                Duration::from_secs(config.server.timeout_seconds),
            ))
            .layer(SetRequestIdLayer::new(
                axum::http::HeaderName::from_static("x-request-id"),
                MakeRequestUuid,
            ))
            .layer(PropagateRequestIdLayer::new(
                axum::http::HeaderName::from_static("x-request-id"),
            ))
            .into_inner(),
    );
    let app = if config.observability.metrics_enabled {
        let (metrics_layer, metrics_handle) = PrometheusMetricLayer::pair();
        let metrics_auth_token = config.observability.metrics_auth_token.clone();
        let metrics_route = get({
            let metrics_handle = metrics_handle.clone();
            move || {
                let metrics_handle = metrics_handle.clone();
                async move { metrics_handle.render() }
            }
        });
        let metrics_route = if let Some(token) = metrics_auth_token {
            axum::routing::get(move |req: axum::extract::Request| {
                let metrics_handle = metrics_handle.clone();
                let token = token.clone();
                async move {
                    let auth_header = req
                        .headers()
                        .get(axum::http::header::AUTHORIZATION)
                        .and_then(|v| v.to_str().ok());
                    match auth_header {
                        Some(header) if header == format!("Bearer {token}") => {
                            metrics_handle.render().into_response()
                        }
                        _ => (
                            axum::http::StatusCode::UNAUTHORIZED,
                            "Unauthorized".to_owned(),
                        )
                            .into_response(),
                    }
                }
            })
        } else {
            metrics_route
        };
        app.route("/metrics", metrics_route).layer(metrics_layer)
    } else {
        app
    };
    let listener = tokio::net::TcpListener::bind(addr).await?;
    if config.server.tls.enabled {
        let tls_config =
            load_tls_config(&config.server.tls.cert_path, &config.server.tls.key_path)?;
        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(tls_config));
        let tls_listener = TlsListener {
            inner: listener,
            acceptor,
        };
        axum::serve(tls_listener, app.into_make_service())
            .with_graceful_shutdown(async {
                tokio::signal::ctrl_c().await.ok();
            })
            .await?;
    } else {
        axum::serve(listener, app.into_make_service())
            .with_graceful_shutdown(async {
                tokio::signal::ctrl_c().await.ok();
            })
            .await?;
    }
    Ok(())
}
struct TlsListener {
    inner: TcpListener,
    acceptor: tokio_rustls::TlsAcceptor,
}
impl axum::serve::Listener for TlsListener {
    type Io = tokio_rustls::server::TlsStream<tokio::net::TcpStream>;
    type Addr = std::net::SocketAddr;
    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            let (stream, addr) = match self.inner.accept().await {
                Ok(tup) => tup,
                Err(e) => {
                    tracing::error!("TCP accept error: {e}");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
            };
            match self.acceptor.accept(stream).await {
                Ok(tls_stream) => return (tls_stream, addr),
                Err(e) => {
                    tracing::error!("TLS handshake error from {addr}: {e}");
                    continue;
                }
            }
        }
    }
    fn local_addr(&self) -> std::io::Result<Self::Addr> {
        self.inner.local_addr()
    }
}
fn load_tls_config(
    cert_path: &str,
    key_path: &str,
) -> Result<tokio_rustls::rustls::ServerConfig, Box<dyn std::error::Error + Send + Sync>> {
    use tokio_rustls::rustls::pki_types::CertificateDer;

    let cert_file = std::fs::File::open(cert_path)
        .map_err(|e| format!("Failed to open TLS cert file {cert_path}: {e}"))?;
    let mut cert_reader = std::io::BufReader::new(cert_file);
    let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut cert_reader)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to parse TLS certs: {e}"))?;

    if certs.is_empty() {
        return Err("TLS cert file contains no certificates".into());
    }

    let key_file = std::fs::File::open(key_path)
        .map_err(|e| format!("Failed to open TLS key file {key_path}: {e}"))?;
    let mut key_reader = std::io::BufReader::new(key_file);
    let key = rustls_pemfile::private_key(&mut key_reader)
        .map_err(|e| format!("Failed to parse TLS key: {e}"))?
        .ok_or_else(|| -> Box<dyn std::error::Error + Send + Sync> {
            "TLS key file contains no private key".into()
        })?;

    let config = tokio_rustls::rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| format!("Invalid TLS cert/key pair: {e}"))?;

    Ok(config)
}
