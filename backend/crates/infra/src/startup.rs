use config::AppConfig;
use repo::AnyUserRepo;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub enum ShutdownReason {
    CtrlC,
    SigTerm,
}

#[derive(Debug)]
pub enum StartupError {
    Config(String),
    Database(String),
}

impl std::fmt::Display for StartupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StartupError::Config(msg) => write!(f, "Config error: {msg}"),
            StartupError::Database(msg) => write!(f, "Database error: {msg}"),
        }
    }
}

impl std::error::Error for StartupError {}

/// Connect to the database with retry logic, returning a shared repo, health checker,
/// the background pool-metrics task handle, and the cancellation token used by the metrics task.
///
/// This helper is shared between the combined REST+gRPC binary and the standalone gRPC binary.
pub async fn connect_to_database(
    config: &AppConfig,
) -> Result<
    (
        AnyUserRepo,
        Arc<dyn svc::HealthChecker>,
        tokio::task::JoinHandle<()>,
        CancellationToken,
    ),
    StartupError,
> {
    let cancel = CancellationToken::new();
    let metrics_interval = Duration::from_secs(config.server.metrics_interval_seconds);
    let mut attempt = 0;
    loop {
        attempt += 1;
        match repo::connect(&config.database, cancel.clone(), metrics_interval).await {
            Ok((repo, probe, metrics_handle)) => {
                tracing::info!("Connected to database after {} attempt(s)", attempt);
                let health = Arc::new(crate::health_checker::DbHealthChecker::new(probe));
                return Ok((repo, health, metrics_handle, cancel));
            }
            Err(e) => {
                if attempt >= config.database.connect_retry_attempts {
                    return Err(StartupError::Database(format!(
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

/// Wait for an OS shutdown signal (SIGINT or SIGTERM).
///
/// Returns an error if the signal handler installation or await fails.
pub async fn wait_for_shutdown_signal() -> Result<ShutdownReason, std::io::Error> {
    let ctrl_c = async { tokio::signal::ctrl_c().await.map(|_| ShutdownReason::CtrlC) };

    #[cfg(unix)]
    let terminate = async {
        let mut stream = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .map_err(|e| {
                std::io::Error::other(format!("failed to install SIGTERM handler: {e}"))
            })?;
        stream.recv().await;
        Ok(ShutdownReason::SigTerm)
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<Result<ShutdownReason, std::io::Error>>();

    tokio::select! {
        result = ctrl_c => {
            result.inspect(|_| {
                tracing::info!("Received SIGINT");
            })
        }
        result = terminate => {
            result.inspect(|_| {
                tracing::info!("Received SIGTERM");
            })
        }
    }
}

/// Gracefully shut down one or more server task handles under a single timeout budget.
///
/// Cancels the provided token (to signal servers to start graceful shutdown), then waits
/// for all handles with a single `tokio::time::timeout`. Returns `Ok` if all tasks finish
/// within the budget, or `Err` if any task times out.
pub async fn shutdown_with_budget(
    cancel: CancellationToken,
    timeout: Duration,
    handles: Vec<tokio::task::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>>>,
) -> Result<(), String> {
    cancel.cancel();

    let joined = async {
        let mut results = Vec::with_capacity(handles.len());
        for handle in handles {
            results.push(handle.await);
        }
        results
    };

    match tokio::time::timeout(timeout, joined).await {
        Ok(results) => {
            let mut errors = Vec::new();
            for (idx, r) in results.into_iter().enumerate() {
                match r {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => errors.push(format!("task {idx} failed: {e}")),
                    Err(e) => errors.push(format!("task {idx} panicked: {e}")),
                }
            }
            if errors.is_empty() {
                Ok(())
            } else {
                Err(errors.join(", "))
            }
        }
        Err(_) => Err(format!("Shutdown timed out after {}s", timeout.as_secs())),
    }
}
