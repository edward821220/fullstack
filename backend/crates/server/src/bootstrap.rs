use config::AppConfig;
use grpc::serve as grpc_serve;
use infra::health_checker::DbHealthChecker;
use infra::telemetry::init_tracing;
use repo::AnyUserRepo;
use std::{sync::Arc, time::Duration};

#[derive(Debug)]
pub enum BootstrapError {
    Config(String),
    Database(String),
    Migration(String),
    Telemetry(String),
    RestServer(String),
    GrpcServer(String),
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
        }
    }
}

impl std::error::Error for BootstrapError {}

pub fn load_and_validate_config() -> Result<AppConfig, BootstrapError> {
    let config = AppConfig::load()
        .map_err(|e| BootstrapError::Config(format!("Failed to load configuration: {e}")))?;
    config
        .validate()
        .map_err(|e| BootstrapError::Config(format!("Config validation failed: {e}")))?;
    Ok(config)
}

pub async fn connect_to_database(
    config: &AppConfig,
) -> Result<(AnyUserRepo, Arc<dyn svc::HealthChecker>), BootstrapError> {
    let mut attempt = 0;
    loop {
        attempt += 1;
        match repo::connect(&config.database).await {
            Ok((repo, probe)) => {
                tracing::info!("Connected to database after {} attempt(s)", attempt);
                let health = Arc::new(DbHealthChecker::new(probe));
                return Ok((repo, health));
            }
            Err(e) => {
                if attempt >= config.database.connect_retry_attempts {
                    return Err(BootstrapError::Database(format!(
                        "Failed to connect after {} attempts: {e}",
                        attempt
                    )));
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
    let telemetry = init_tracing(&config).map_err(|e| BootstrapError::Telemetry(e.to_string()))?;

    if config.database.run_migrations_on_startup {
        run_migrations(&config).await?;
    } else {
        tracing::info!("Skipping migrations on startup (run_migrations_on_startup=false)");
    }

    let (rest_repo, rest_health) = connect_to_database(&config).await?;
    let (grpc_repo, grpc_health) = connect_to_database(&config).await?;

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

    let config_clone = config.clone();
    let rest_handle = tokio::spawn(async move {
        tracing::info!("REST server listening on {}", rest_addr);
        if let Err(e) =
            crate::rest_server::serve_rest(config_clone, rest_repo, rest_health, rest_addr).await
        {
            tracing::error!("REST server error: {e}");
        }
    });

    let grpc_handle = if let Some(grpc_addr) = grpc_addr {
        let config_clone2 = config.clone();
        Some(tokio::spawn(async move {
            tracing::info!("gRPC server listening on {}", grpc_addr);
            if let Err(e) = grpc_serve(config_clone2, grpc_repo, grpc_health, grpc_addr).await {
                tracing::error!("gRPC server error: {e}");
            }
        }))
    } else {
        tracing::info!("gRPC server is disabled");
        None
    };

    tokio::signal::ctrl_c().await.map_err(|e| {
        BootstrapError::Config(format!("Failed to listen for shutdown signal: {e}"))
    })?;
    tracing::info!(
        "Shutdown signal received, draining for {}s...",
        config.server.shutdown_timeout_seconds
    );

    let timeout = Duration::from_secs(config.server.shutdown_timeout_seconds);

    let rest_result = tokio::time::timeout(timeout, rest_handle).await;
    let rest_done = rest_result.is_ok();

    let grpc_done = if let Some(handle) = grpc_handle {
        let grpc_result = tokio::time::timeout(timeout, handle).await;
        grpc_result.is_ok()
    } else {
        true
    };

    if rest_done && grpc_done {
        tracing::info!("All services shut down gracefully");
    } else {
        tracing::warn!("Shutdown timeout reached (rest_ok={rest_done}, grpc_ok={grpc_done})");
    }

    telemetry.shutdown();

    tracing::info!("Goodbye.");
    Ok(())
}
