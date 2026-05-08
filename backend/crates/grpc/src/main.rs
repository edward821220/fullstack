use clap::Parser;
use config::AppConfig;
use grpc::serve;
use infra::startup::{connect_to_database, shutdown_with_budget, wait_for_shutdown_signal};
use infra::telemetry::init_tracing;
use std::path::PathBuf;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[derive(Parser)]
#[command(name = "grpc-server")]
struct Cli {
    #[arg(long, env = "APP_CONFIG_DIR")]
    config_dir: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cli = Cli::parse();

    let config = if let Some(dir) = cli.config_dir {
        AppConfig::load_with_config_dir(dir)?
    } else {
        AppConfig::load()?
    };
    config.validate()?;

    let telemetry = init_tracing(&config)?;

    migration::run(&config.database)
        .await
        .map_err(|error| std::io::Error::other(error.to_string()))?;

    let (repo, health, metrics_handle, metrics_cancel) = connect_to_database(&config).await?;

    let addr = config.grpc_addr()?;

    tracing::info!("Standalone gRPC server listening on {}", addr);

    let cancel = CancellationToken::new();
    let serve_cancel = cancel.clone();

    let config_for_serve = config.clone();
    let mut serve_handle: tokio::task::JoinHandle<
        Result<(), Box<dyn std::error::Error + Send + Sync>>,
    > = tokio::spawn(
        async move { serve(config_for_serve, repo, health, addr, serve_cancel).await },
    );

    let signal_fut = wait_for_shutdown_signal();
    tokio::pin!(signal_fut);

    let mut early_failure: Option<Box<dyn std::error::Error + Send + Sync>> = None;

    tokio::select! {
        result = &mut serve_handle => {
            early_failure = Some(match result {
                Ok(Ok(())) => "gRPC server exited early (before shutdown signal)".into(),
                Ok(Err(e)) => Box::new(std::io::Error::other(format!("gRPC server error: {e}"))) as Box<dyn std::error::Error + Send + Sync>,
                Err(e) => Box::new(std::io::Error::other(format!("gRPC task panicked: {e}"))) as Box<dyn std::error::Error + Send + Sync>,
            });
        }
        result = &mut signal_fut => {
            match result {
                Ok(_) => tracing::info!("Shutdown signal received"),
                Err(e) => {
                    tracing::warn!("Shutdown signal handler error: {e}");
                    early_failure = Some(Box::new(std::io::Error::other(format!("Shutdown signal handler error: {e}"))) as Box<dyn std::error::Error + Send + Sync>);
                }
            }
        }
    }

    cancel.cancel();

    let shutdown_timeout = Duration::from_secs(config.server.shutdown_timeout_seconds);
    let result = shutdown_with_budget(cancel, shutdown_timeout, vec![serve_handle]).await;

    if let Err(e) = result {
        tracing::warn!("gRPC shutdown incomplete: {e}");
    } else {
        tracing::info!("gRPC server shut down gracefully");
    }

    metrics_cancel.cancel();
    let _ = metrics_handle.await;

    telemetry.shutdown();

    if let Some(e) = early_failure {
        Err(e)
    } else {
        Ok(())
    }
}
