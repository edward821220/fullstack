use clap::{Parser, Subcommand};
use config::AppConfig;
use server::openapi::ApiDoc;
use std::path::PathBuf;
use std::process;
use utoipa::OpenApi;

#[derive(Parser)]
#[command(name = "server")]
struct Cli {
    #[arg(long, env = "APP_CONFIG_DIR", global = true)]
    config_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Print the OpenAPI specification as JSON to stdout
    GenOpenapi,
    /// Run database migrations and exit
    Migrate,
    /// Start the server (default)
    Serve,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::GenOpenapi) => {
            let spec = ApiDoc::openapi();
            let json = serde_json::to_string_pretty(&spec).unwrap_or_else(|e| {
                eprintln!("Failed to serialize OpenAPI spec: {e}");
                process::exit(1);
            });
            println!("{json}");
        }
        Some(Command::Migrate) => {
            let config = load_config_or_exit(cli.config_dir);
            let telemetry = init_telemetry_or_exit(&config);
            if let Err(e) = server::bootstrap::run_migrations(&config).await {
                tracing::error!("{e}");
                telemetry.shutdown();
                process::exit(1);
            }
            telemetry.shutdown();
        }
        Some(Command::Serve) | None => {
            let config = load_config_or_exit(cli.config_dir);
            if let Err(e) = server::bootstrap::run(config).await {
                tracing::error!("{e}");
                process::exit(1);
            }
        }
    }
}

fn load_config_or_exit(config_dir: Option<PathBuf>) -> AppConfig {
    match server::bootstrap::load_and_validate_config(config_dir) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            process::exit(1);
        }
    }
}

fn init_telemetry_or_exit(config: &AppConfig) -> infra::telemetry::TelemetryGuard {
    match infra::telemetry::init_tracing(config) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to initialize observability: {e}");
            process::exit(1);
        }
    }
}
