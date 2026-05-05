use std::sync::Arc;
use std::time::Duration;

use axum::{Router, middleware as axum_middleware, routing::get};
use axum_prometheus::PrometheusMetricLayer;
use config::AppConfig;
use repo::AnyUserRepo;
use svc::UserService;
use tower::ServiceBuilder;
use tower_http::{
    cors::{Any, CorsLayer},
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    timeout::TimeoutLayer,
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
};
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};

use crate::handlers::{health, users};
use crate::middleware::OidcValidator;
use crate::middleware::oidc::oidc_middleware;
use crate::openapi::ApiDoc;
use crate::state::AppState;

pub async fn serve_rest(
    config: AppConfig,
    repo: AnyUserRepo,
    addr: std::net::SocketAddr,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let svc = Arc::new(UserService::new(repo));
    let oidc_validator = Arc::new(OidcValidator::new(config.auth.clone()));

    let provisioning =
        svc::ProvisioningPolicy::new(config.auth.allowed_email_domains.clone(), "user".to_owned());

    let app_state = Arc::new(AppState {
        svc: svc.clone(),
        oidc: oidc_validator,
        provisioning,
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
        .allow_methods(Any)
        .allow_headers(Any);

    let api_routes = Router::new()
        .merge(users::routes(app_state.clone()))
        .route_layer(axum_middleware::from_fn_with_state(
            app_state.clone(),
            oidc_middleware,
        ));

    let app = Router::new()
        .route("/health", get(health::health))
        .route(
            "/health/ready",
            get({
                let svc = svc.clone();
                move || {
                    let svc = svc.clone();
                    async move { health::health_ready(svc).await }
                }
            }),
        )
        .nest("/api/v1", api_routes)
        .merge(Scalar::with_url("/docs", ApiDoc::openapi()))
        .layer(
            ServiceBuilder::new()
                .layer(
                    TraceLayer::new_for_http()
                        .make_span_with(DefaultMakeSpan::new().level(tracing::Level::INFO))
                        .on_response(DefaultOnResponse::new().level(tracing::Level::INFO)),
                )
                .layer(cors)
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

        app.route(
            "/metrics",
            get({
                let metrics_handle = metrics_handle.clone();
                move || {
                    let metrics_handle = metrics_handle.clone();
                    async move { metrics_handle.render() }
                }
            }),
        )
        .layer(metrics_layer)
    } else {
        app
    };

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service())
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.ok();
        })
        .await?;

    Ok(())
}
