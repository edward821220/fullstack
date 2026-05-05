use std::time::Duration;

use config::AppConfig;

pub async fn connect_to_database(config: &AppConfig) -> repo::AnyUserRepo {
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
