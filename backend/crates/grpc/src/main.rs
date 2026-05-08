use clap::Parser;
use config::AppConfig;
use grpc::serve;
use infra::startup::{connect_to_database, shutdown_with_budget, wait_for_shutdown_signal};
use infra::telemetry::init_tracing;
use std::future::Future;
use std::path::PathBuf;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

type ServeTaskResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;
type ServeTaskHandle = tokio::task::JoinHandle<ServeTaskResult>;

#[derive(Parser)]
#[command(name = "grpc-server")]
struct Cli {
    #[arg(long, env = "APP_CONFIG_DIR")]
    config_dir: Option<PathBuf>,
}

fn spawn_monitored_serve_task<F>(
    future: F,
    shutdown: CancellationToken,
    failure_tx: tokio::sync::mpsc::Sender<Box<dyn std::error::Error + Send + Sync>>,
) -> ServeTaskHandle
where
    F: Future<Output = ServeTaskResult> + Send + 'static,
{
    let handle: ServeTaskHandle = tokio::spawn(future);

    tokio::spawn(async move {
        match handle.await {
            Ok(result) => {
                if !shutdown.is_cancelled() {
                    let failure: Box<dyn std::error::Error + Send + Sync> = match &result {
                        Ok(()) => "gRPC server exited early (before shutdown signal)".into(),
                        Err(e) => {
                            Box::new(std::io::Error::other(format!("gRPC server error: {e}")))
                        }
                    };
                    let _ = failure_tx.send(failure).await;
                }
                result
            }
            Err(e) => {
                let message = e.to_string();
                if !shutdown.is_cancelled() {
                    let _ = failure_tx
                        .send(Box::new(std::io::Error::other(format!(
                            "gRPC task panicked: {message}"
                        )))
                            as Box<dyn std::error::Error + Send + Sync>)
                        .await;
                }
                Err(Box::new(std::io::Error::other(message))
                    as Box<dyn std::error::Error + Send + Sync>)
            }
        }
    })
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

    let mut telemetry = init_tracing(&config)?;

    migration::run(&config.database)
        .await
        .map_err(|error| std::io::Error::other(error.to_string()))?;

    let (repo, health, metrics_handle, metrics_cancel) = connect_to_database(&config).await?;

    let addr = config.grpc_addr()?;

    tracing::info!("Standalone gRPC server listening on {}", addr);

    let cancel = CancellationToken::new();
    let (failure_tx, mut failure_rx) = tokio::sync::mpsc::channel(1);
    let serve_cancel = cancel.clone();

    let config_for_serve = config.clone();
    let serve_handle = spawn_monitored_serve_task(
        async move { serve(config_for_serve, repo, health, addr, serve_cancel).await },
        cancel.clone(),
        failure_tx.clone(),
    );

    drop(failure_tx);

    let signal_fut = wait_for_shutdown_signal();
    tokio::pin!(signal_fut);

    let mut early_failure: Option<Box<dyn std::error::Error + Send + Sync>> = None;

    tokio::select! {
        failure = failure_rx.recv() => {
            early_failure = Some(match failure {
                Some(failure) => failure,
                None => Box::new(std::io::Error::other("gRPC server monitor exited unexpectedly")) as Box<dyn std::error::Error + Send + Sync>,
            });
        }
        result = &mut signal_fut => {
            match result {
                Ok(_) => tracing::info!("Shutdown signal received"),
                Err(e) => {
                    early_failure = Some(Box::new(e) as Box<dyn std::error::Error + Send + Sync>);
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
