use config::AppConfig;
use server::{grpc, health_checker::DbHealthChecker, telemetry::init_tracing};
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = AppConfig::load()?;
    config.validate()?;

    let telemetry = init_tracing(&config)?;

    migration::run(&config.database)
        .await
        .map_err(|error| std::io::Error::other(error.to_string()))?;

    let (repo, health) = connect_to_database(&config).await?;
    let addr = config.grpc_addr()?;

    tracing::info!("Standalone gRPC server listening on {}", addr);

    grpc::serve(config, repo, health, addr).await?;

    telemetry.shutdown();

    Ok(())
}

async fn connect_to_database(
    config: &AppConfig,
) -> Result<
    (repo::AnyUserRepo, Arc<dyn svc::HealthChecker>),
    Box<dyn std::error::Error + Send + Sync>,
> {
    let mut attempt = 0;

    loop {
        attempt += 1;

        match repo::connect(&config.database).await {
            Ok((repo, probe)) => {
                let health = Arc::new(DbHealthChecker::new(probe));
                return Ok((repo, health));
            }
            Err(error) => {
                if attempt >= config.database.connect_retry_attempts {
                    return Err(Box::new(std::io::Error::other(error.to_string())));
                }

                tracing::warn!(
                    "Database connection attempt {}/{} failed: {}. Retrying...",
                    attempt,
                    config.database.connect_retry_attempts,
                    error
                );

                tokio::time::sleep(Duration::from_millis(
                    config.database.connect_retry_delay_ms,
                ))
                .await;
            }
        }
    }
}
