use clap::{Parser, Subcommand};
use config::AppConfig;
use grpc::serve as grpc_serve;
use infra::telemetry::init_tracing;
use server::openapi::ApiDoc;
use std::time::Duration;
use utoipa::OpenApi;

#[derive(Parser)]
#[command(name = "server")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
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
                std::process::exit(1);
            });
            println!("{json}");
            return;
        }
        Some(Command::Migrate) => {
            let config = match AppConfig::load() {
                Ok(c) => {
                    if let Err(e) = c.validate() {
                        eprintln!("Config validation failed: {e}");
                        std::process::exit(1);
                    }
                    c
                }
                Err(e) => {
                    eprintln!("Failed to load configuration: {e}");
                    std::process::exit(1);
                }
            };
            let telemetry = init_tracing(&config).unwrap_or_else(|e| {
                eprintln!("Failed to initialize observability: {e}");
                std::process::exit(1);
            });
            tracing::info!("Running migrations...");
            if let Err(e) = migration::run(&config.database).await {
                tracing::error!("Migration failed: {e}");
                telemetry.shutdown();
                std::process::exit(1);
            }
            tracing::info!("Migrations completed successfully.");
            telemetry.shutdown();
            return;
        }
        Some(Command::Serve) | None => {}
    }

    let config = match AppConfig::load() {
        Ok(c) => {
            if let Err(e) = c.validate() {
                eprintln!("Config validation failed: {e}");
                std::process::exit(1);
            }
            c
        }
        Err(e) => {
            eprintln!("Failed to load configuration: {e}");
            std::process::exit(1);
        }
    };

    let telemetry = init_tracing(&config).unwrap_or_else(|e| {
        eprintln!("Failed to initialize observability: {e}");
        std::process::exit(1);
    });

    tracing::info!("Starting server...");

    if config.database.run_migrations_on_startup {
        if let Err(e) = migration::run(&config.database).await {
            tracing::error!("Migration failed: {e}");
            std::process::exit(1);
        }
    } else {
        tracing::info!("Skipping migrations on startup (run_migrations_on_startup=false)");
    }

    let (rest_repo, rest_health) = server::bootstrap::connect_to_database(&config).await;
    let (grpc_repo, grpc_health) = server::bootstrap::connect_to_database(&config).await;

    let rest_addr = config.rest_addr().unwrap_or_else(|e| {
        tracing::error!("Invalid REST address: {e}");
        std::process::exit(1);
    });
    if config.grpc.enabled {
        let _grpc_addr = config.grpc_addr().unwrap_or_else(|e| {
            tracing::error!("Invalid gRPC address: {e}");
            std::process::exit(1);
        });
    }

    let config_clone = config.clone();
    let rest_handle = tokio::spawn(async move {
        tracing::info!("REST server listening on {}", rest_addr);
        if let Err(e) =
            server::rest_server::serve_rest(config_clone, rest_repo, rest_health, rest_addr).await
        {
            tracing::error!("REST server error: {e}");
        }
    });

    let grpc_handle = if config.grpc.enabled {
        let grpc_addr = config.grpc_addr().unwrap_or_else(|e| {
            tracing::error!("Invalid gRPC address: {e}");
            std::process::exit(1);
        });
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

    tokio::signal::ctrl_c().await.unwrap_or_else(|e| {
        tracing::error!("Failed to listen for shutdown signal: {e}");
    });
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
}
