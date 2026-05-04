use axum::middleware as axum_middleware;
use axum::{Router, routing::get};
use axum_prometheus::PrometheusMetricLayer;
use clap::{Parser, Subcommand};
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceBuilder;
use tower_http::{
    cors::{Any, CorsLayer},
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    timeout::TimeoutLayer,
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
};
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};

use config::AppConfig;
use server::grpc;
use server::handlers::{health, users};
use server::middleware::oidc::oidc_middleware;
use server::middleware::{AppState, OidcValidator};
use server::openapi::ApiDoc;
use server::telemetry::init_tracing;

#[derive(Parser)]
#[command(name = "server")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Print the OpenAPI specification as JSON to stdout
    GenOpenapi,
    /// Start the server (default)
    Serve,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::GenOpenapi) => {
            let spec = ApiDoc::openapi();
            let json = serde_json::to_string_pretty(&spec).unwrap_or_else(|e| {
                eprintln!("Failed to serialize OpenAPI spec: {e}");
                std::process::exit(1);
            });
            println!("{json}");
            return;
        }
        Some(Command::Serve) | None => {}
    }

    let config = match AppConfig::load() {
        Ok(c) => {
            if let Err(e) = c.validate() {
                eprintln!("Config validation failed: {e}");
                std::process::exit(1);
            }
            c
        }
        Err(e) => {
            eprintln!("Failed to load configuration: {e}");
            std::process::exit(1);
        }
    };

    let telemetry = init_tracing(&config).unwrap_or_else(|e| {
        eprintln!("Failed to initialize observability: {e}");
        std::process::exit(1);
    });

    tracing::info!("Starting server...");

    if let Err(e) = migration::run(&config.database).await {
        tracing::error!("Migration failed: {e}");
        std::process::exit(1);
    }

    let rest_repo = connect_to_database(&config).await;
    let grpc_repo = connect_to_database(&config).await;

    let rest_addr = config.rest_addr().unwrap_or_else(|e| {
        tracing::error!("Invalid REST address: {e}");
        std::process::exit(1);
    });
    let grpc_addr = config.grpc_addr().unwrap_or_else(|e| {
        tracing::error!("Invalid gRPC address: {e}");
        std::process::exit(1);
    });

    let config_clone = config.clone();
    let rest_handle = tokio::spawn(async move {
        tracing::info!("REST server listening on {}", rest_addr);
        if let Err(e) = serve_rest(config_clone, rest_repo, rest_addr).await {
            tracing::error!("REST server error: {e}");
        }
    });

    let config_clone2 = config.clone();
    let grpc_handle = tokio::spawn(async move {
        tracing::info!("gRPC server listening on {}", grpc_addr);
        if let Err(e) = grpc::serve(config_clone2, grpc_repo, grpc_addr).await {
            tracing::error!("gRPC server error: {e}");
        }
    });

    tokio::signal::ctrl_c().await.unwrap_or_else(|e| {
        tracing::error!("Failed to listen for shutdown signal: {e}");
    });
    tracing::info!(
        "Shutdown signal received, draining for {}s...",
        config.server.shutdown_timeout_seconds
    );

    let timeout = Duration::from_secs(config.server.shutdown_timeout_seconds);

    let (rest_result, grpc_result) = tokio::join!(
        tokio::time::timeout(timeout, rest_handle),
        tokio::time::timeout(timeout, grpc_handle),
    );

    let rest_done = rest_result.is_ok();
    let grpc_done = grpc_result.is_ok();

    if rest_done && grpc_done {
        tracing::info!("All services shut down gracefully");
    } else {
        tracing::warn!("Shutdown timeout reached (rest_ok={rest_done}, grpc_ok={grpc_done})");
    }

    telemetry.shutdown();

    tracing::info!("Goodbye.");
}

async fn connect_to_database(config: &AppConfig) -> Box<dyn repo::UserRepo> {
    let mut attempt = 0;
    loop {
        attempt += 1;
        match repo::connect(&config.database).await {
            Ok(repo) => {
                tracing::info!("Connected to database after {} attempt(s)", attempt);
                return repo;
            }
            Err(e) => {
                if attempt >= config.database.connect_retry_attempts {
                    tracing::error!(
                        "Failed to connect to database after {} attempts: {}",
                        attempt,
                        e
                    );
                    std::process::exit(1);
                }
                tracing::warn!(
                    "Database connection attempt {}/{} failed: {}. Retrying...",
                    attempt,
                    config.database.connect_retry_attempts,
                    e
                );
                tokio::time::sleep(Duration::from_millis(
                    config.database.connect_retry_delay_ms,
                ))
                .await;
            }
        }
    }
}

async fn serve_rest(
    config: AppConfig,
    repo: Box<dyn repo::UserRepo>,
    addr: std::net::SocketAddr,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let svc = Arc::new(svc::UserService::new(repo));
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
