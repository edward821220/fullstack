use async_trait::async_trait;

/// Infrastructure health check seam.
///
/// Implementations probe external dependencies (database, message queues, etc.)
/// and report whether the service can accept traffic.
#[async_trait]
pub trait HealthChecker: Send + Sync {
    async fn check(&self) -> Result<(), HealthError>;
}

/// Error produced when a health probe fails.
#[derive(Debug, snafu::Snafu)]
pub enum HealthError {
    #[snafu(display("Health check failed: {message}"))]
    CheckFailed { message: String },
}
