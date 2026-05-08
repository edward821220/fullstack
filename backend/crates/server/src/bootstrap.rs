use config::AppConfig;
use grpc::serve as grpc_serve;
use infra::startup::{
    StartupError, connect_to_database, shutdown_with_budget, wait_for_shutdown_signal,
};
use infra::telemetry::init_tracing;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[derive(Debug)]
pub enum BootstrapError {
    Config(String),
    Database(String),
    Migration(String),
    Telemetry(String),
    RestServer(String),
    GrpcServer(String),
    Startup(String),
}

impl std::fmt::Display for BootstrapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BootstrapError::Config(msg) => write!(f, "Config error: {msg}"),
            BootstrapError::Database(msg) => write!(f, "Database error: {msg}"),
            BootstrapError::Migration(msg) => write!(f, "Migration error: {msg}"),
            BootstrapError::Telemetry(msg) => write!(f, "Telemetry error: {msg}"),
            BootstrapError::RestServer(msg) => write!(f, "REST server error: {msg}"),
            BootstrapError::GrpcServer(msg) => write!(f, "gRPC server error: {msg}"),
            BootstrapError::Startup(msg) => write!(f, "Startup error: {msg}"),
        }
    }
}

impl std::error::Error for BootstrapError {}

impl From<StartupError> for BootstrapError {
    fn from(e: StartupError) -> Self {
        BootstrapError::Startup(e.to_string())
    }
}

pub fn load_and_validate_config() -> Result<AppConfig, BootstrapError> {
    let config = AppConfig::load()
        .map_err(|e| BootstrapError::Config(format!("Failed to load configuration: {e}")))?;
    config
        .validate()
        .map_err(|e| BootstrapError::Config(format!("Config validation failed: {e}")))?;
    Ok(config)
}

pub async fn run_migrations(config: &AppConfig) -> Result<(), BootstrapError> {
    tracing::info!("Running migrations...");
    migration::run(&config.database)
        .await
        .map_err(|e| BootstrapError::Migration(e.to_string()))?;
    tracing::info!("Migrations completed successfully.");
    Ok(())
}

/// Orchestrate the full server lifecycle: tracing, migrations, DB connection,
/// REST + gRPC server startup, graceful shutdown.
pub async fn run(config: AppConfig) -> Result<(), BootstrapError> {
    infra::ensure_jwt_crypto_provider();

    let telemetry = init_tracing(&config).map_err(|e| BootstrapError::Telemetry(e.to_string()))?;

    if config.database.run_migrations_on_startup {
        run_migrations(&config).await?;
    } else {
        tracing::info!("Skipping migrations on startup (run_migrations_on_startup=false)");
    }

    let (repo, health, metrics_handle, metrics_cancel) = connect_to_database(&config).await?;

    let rest_addr = config
        .rest_addr()
        .map_err(|e| BootstrapError::Config(format!("Invalid REST address: {e}")))?;

    let grpc_addr = if config.grpc.enabled {
        Some(
            config
                .grpc_addr()
                .map_err(|e| BootstrapError::Config(format!("Invalid gRPC address: {e}")))?,
        )
    } else {
        None
    };

    let cancel = CancellationToken::new();
    let rest_cancel = cancel.clone();
    let grpc_cancel = cancel.clone();

    let rest_config = config.clone();
    let rest_repo = repo.clone();
    let rest_health = Arc::clone(&health);
    let mut rest_handle: tokio::task::JoinHandle<
        Result<(), Box<dyn std::error::Error + Send + Sync>>,
    > = tokio::spawn(async move {
        tracing::info!("REST server listening on {}", rest_addr);
        crate::rest_server::serve_rest(rest_config, rest_repo, rest_health, rest_addr, rest_cancel)
            .await
    });

    let mut grpc_handle: Option<
        tokio::task::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>>,
    > = if let Some(grpc_addr) = grpc_addr {
        let grpc_config = config.clone();
        let grpc_repo = repo.clone();
        let grpc_health = Arc::clone(&health);
        Some(tokio::spawn(async move {
            tracing::info!("gRPC server listening on {}", grpc_addr);
            grpc_serve(grpc_config, grpc_repo, grpc_health, grpc_addr, grpc_cancel).await
        }))
    } else {
        tracing::info!("gRPC server is disabled");
        None
    };

    let signal_fut = wait_for_shutdown_signal();
    tokio::pin!(signal_fut);

    let grpc_fut = async {
        if let Some(ref mut h) = grpc_handle {
            h.await
        } else {
            std::future::pending().await
        }
    };

    let mut early_failure: Option<BootstrapError> = None;

    tokio::select! {
        rest = &mut rest_handle => {
            early_failure = Some(match rest {
                Ok(Ok(())) => BootstrapError::RestServer("REST server exited early (before shutdown signal)".to_owned()),
                Ok(Err(e)) => BootstrapError::RestServer(format!("REST server error: {e}")),
                Err(e) => BootstrapError::RestServer(format!("REST task panicked: {e}")),
            });
        }
        grpc = grpc_fut => {
            early_failure = Some(match grpc {
                Ok(Ok(())) => BootstrapError::GrpcServer("gRPC server exited early (before shutdown signal)".to_owned()),
                Ok(Err(e)) => BootstrapError::GrpcServer(format!("gRPC server error: {e}")),
                Err(e) => BootstrapError::GrpcServer(format!("gRPC task panicked: {e}")),
            });
        }
        result = &mut signal_fut => {
            match result {
                Ok(_) => tracing::info!("Shutdown signal received"),
                Err(e) => {
                    tracing::warn!("Shutdown signal handler error: {e}");
                    early_failure = Some(BootstrapError::Startup(format!("Shutdown signal handler error: {e}")));
                }
            }
        }
    }

    cancel.cancel();

    let timeout = Duration::from_secs(config.server.shutdown_timeout_seconds);
    tracing::info!("Draining for {}s...", timeout.as_secs());

    let mut handles: Vec<
        tokio::task::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>>,
    > = Vec::new();
    handles.push(rest_handle);
    if let Some(h) = grpc_handle {
        handles.push(h);
    }

    if let Err(e) = shutdown_with_budget(cancel.clone(), timeout, handles).await {
        tracing::warn!("Shutdown incomplete: {e}");
    } else {
        tracing::info!("All services shut down gracefully");
    }

    metrics_cancel.cancel();
    let _ = metrics_handle.await;

    telemetry.shutdown();
    tracing::info!("Goodbye.");

    if let Some(e) = early_failure {
        Err(e)
    } else {
        Ok(())
    }
}
