use config::AppConfig;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let config = AppConfig::load().expect("Failed to load configuration");

    migration::run(&config.database)
        .await
        .expect("Migration failed");

    tracing::info!("All migrations applied successfully");
}
