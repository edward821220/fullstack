use config::AppConfig;
use grpc::serve as grpc_serve;
use infra::startup::{
    StartupError, connect_to_database, shutdown_with_budget, wait_for_shutdown_signal,
};
use infra::telemetry::TelemetryGuard;
use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

type ServerTaskResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;
type ServerTaskHandle = tokio::task::JoinHandle<ServerTaskResult>;

#[derive(Debug)]
pub enum BootstrapError {
    Config(String),
    Database(String),
    Migration(String),
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
            BootstrapError::RestServer(msg) => write!(f, "REST server error: {msg}"),
            BootstrapError::GrpcServer(msg) => write!(f, "gRPC server error: {msg}"),
            BootstrapError::Startup(msg) => write!(f, "Startup error: {msg}"),
        }
    }
}

impl std::error::Error for BootstrapError {}

impl From<StartupError> for BootstrapError {
    fn from(e: StartupError) -> Self {
        match e {
            StartupError::Database { message } => BootstrapError::Database(message),
            StartupError::Signal { message } | StartupError::Shutdown { message } => {
                BootstrapError::Startup(message)
            }
        }
    }
}

fn rest_task_result_to_bootstrap_error(result: &ServerTaskResult) -> BootstrapError {
    match result {
        Ok(()) => BootstrapError::RestServer(
            "REST server exited early (before shutdown signal)".to_owned(),
        ),
        Err(e) => BootstrapError::RestServer(format!("REST server error: {e}")),
    }
}

fn rest_task_panic_to_bootstrap_error(message: String) -> BootstrapError {
    BootstrapError::RestServer(format!("REST task panicked: {message}"))
}

fn grpc_task_result_to_bootstrap_error(result: &ServerTaskResult) -> BootstrapError {
    match result {
        Ok(()) => BootstrapError::GrpcServer(
            "gRPC server exited early (before shutdown signal)".to_owned(),
        ),
        Err(e) => BootstrapError::GrpcServer(format!("gRPC server error: {e}")),
    }
}

fn grpc_task_panic_to_bootstrap_error(message: String) -> BootstrapError {
    BootstrapError::GrpcServer(format!("gRPC task panicked: {message}"))
}

fn spawn_monitored_server_task<F>(
    future: F,
    shutdown: CancellationToken,
    failure_tx: tokio::sync::mpsc::Sender<BootstrapError>,
    result_mapper: fn(&ServerTaskResult) -> BootstrapError,
    panic_mapper: fn(String) -> BootstrapError,
) -> ServerTaskHandle
where
    F: Future<Output = ServerTaskResult> + Send + 'static,
{
    let handle: ServerTaskHandle = tokio::spawn(future);

    tokio::spawn(async move {
        match handle.await {
            Ok(result) => {
                if !shutdown.is_cancelled() {
                    let _ = failure_tx.send(result_mapper(&result)).await;
                }
                result
            }
            Err(e) => {
                let message = e.to_string();
                if !shutdown.is_cancelled() {
                    let _ = failure_tx.send(panic_mapper(message.clone())).await;
                }
                Err(Box::new(std::io::Error::other(message))
                    as Box<dyn std::error::Error + Send + Sync>)
            }
        }
    })
}

pub fn load_and_validate_config(config_dir: Option<PathBuf>) -> Result<AppConfig, BootstrapError> {
    let config = if let Some(dir) = config_dir {
        AppConfig::load_with_config_dir(dir)
    } else {
        AppConfig::load()
    }
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

/// Orchestrate the full server lifecycle: migrations, DB connection,
/// REST + gRPC server startup, graceful shutdown.
/// Telemetry is initialized in main() and passed in so its lifetime
/// covers the entire process including shutdown.
pub async fn run_with_telemetry(
    config: AppConfig,
    telemetry: &mut TelemetryGuard,
) -> Result<(), BootstrapError> {
    infra::ensure_jwt_crypto_provider();

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
    let (failure_tx, mut failure_rx) = tokio::sync::mpsc::channel(2);

    let rest_config = config.clone();
    let rest_repo = repo.clone();
    let rest_health = Arc::clone(&health);
    let rest_task_cancel = cancel.clone();
    let rest_handle = spawn_monitored_server_task(
        async move {
            tracing::info!("REST server listening on {}", rest_addr);
            crate::rest_server::serve_rest(
                rest_config,
                rest_repo,
                rest_health,
                rest_addr,
                rest_task_cancel,
            )
            .await
        },
        cancel.clone(),
        failure_tx.clone(),
        rest_task_result_to_bootstrap_error,
        rest_task_panic_to_bootstrap_error,
    );

    let grpc_handle: Option<ServerTaskHandle> = if let Some(grpc_addr) = grpc_addr {
        let grpc_config = config.clone();
        let grpc_repo = repo.clone();
        let grpc_health = Arc::clone(&health);
        let grpc_task_cancel = cancel.clone();
        Some(spawn_monitored_server_task(
            async move {
                tracing::info!("gRPC server listening on {}", grpc_addr);
                grpc_serve(
                    grpc_config,
                    grpc_repo,
                    grpc_health,
                    grpc_addr,
                    grpc_task_cancel,
                )
                .await
            },
            cancel.clone(),
            failure_tx.clone(),
            grpc_task_result_to_bootstrap_error,
            grpc_task_panic_to_bootstrap_error,
        ))
    } else {
        tracing::info!("gRPC server is disabled");
        None
    };

    drop(failure_tx);

    let signal_fut = wait_for_shutdown_signal();
    tokio::pin!(signal_fut);

    let mut early_failure: Option<BootstrapError> = None;

    tokio::select! {
        failure = failure_rx.recv() => {
            early_failure = Some(match failure {
                Some(failure) => failure,
                None => BootstrapError::Startup("Server task monitors exited unexpectedly".to_owned()),
            });
        }
        result = &mut signal_fut => {
            match result {
                Ok(_) => tracing::info!("Shutdown signal received"),
                Err(e) => {
                    early_failure = Some(BootstrapError::from(e));
                }
            }
        }
    }

    cancel.cancel();

    let timeout = Duration::from_secs(config.server.shutdown_timeout_seconds);
    tracing::info!("Draining for {}s...", timeout.as_secs());

    let mut handles: Vec<ServerTaskHandle> = Vec::new();
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
