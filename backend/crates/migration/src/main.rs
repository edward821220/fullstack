use clap::Parser;
use config::AppConfig;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "migration")]
struct Cli {
    #[arg(long, env = "APP_CONFIG_DIR")]
    config_dir: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    let config = if let Some(dir) = cli.config_dir {
        AppConfig::load_with_config_dir(dir).expect("Failed to load configuration")
    } else {
        AppConfig::load().expect("Failed to load configuration")
    };

    migration::run(&config.database)
        .await
        .expect("Migration failed");

    tracing::info!("All migrations applied successfully");
}
