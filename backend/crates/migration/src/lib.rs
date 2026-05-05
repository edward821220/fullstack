mod postgres_migrations {
    refinery::embed_migrations!("./migrations/postgres");
}

mod mssql_migrations {
    refinery::embed_migrations!("./migrations/mssql");
}

pub async fn run(config: &config::DatabaseConfig) -> Result<(), Box<dyn std::error::Error>> {
    use config::DatabaseDriver;

    match config.driver() {
        DatabaseDriver::Postgres => {
            tracing::info!("Running PostgreSQL migrations via refinery");

            let (mut client, connection) =
                tokio_postgres::connect(&config.to_postgres_url(), tokio_postgres::NoTls).await?;

            tokio::spawn(async move {
                if let Err(e) = connection.await {
                    tracing::error!("PostgreSQL migration connection error: {e}");
                }
            });

            let report = postgres_migrations::migrations::runner()
                .run_async(&mut client)
                .await?;

            for migration in report.applied_migrations() {
                tracing::info!(
                    "PostgreSQL migration applied: {} {}",
                    migration.version(),
                    migration.name()
                );
            }

            tracing::info!("PostgreSQL migrations complete");
        }
        DatabaseDriver::Mssql => {
            tracing::info!("Running MSSQL migrations via refinery");
            let db_name = config.extract_mssql_database_name()?;

            let tiberius_config = config.to_tiberius_config()?;

            // Connect to master first, create database if it doesn't exist
            {
                let mut master_config = tiberius_config.clone();
                master_config.database("master");
                let addr = master_config.get_addr().to_owned();
                let tcp = tokio::net::TcpStream::connect(addr).await?;
                tcp.set_nodelay(true)?;
                let mut master_client = tiberius::Client::connect(
                    master_config,
                    tokio_util::compat::TokioAsyncWriteCompatExt::compat_write(tcp),
                )
                .await?;

                let create_db = format!(
                    "IF NOT EXISTS (SELECT name FROM sys.databases WHERE name = '{db_name}') CREATE DATABASE [{db_name}]"
                );
                master_client.execute(&create_db, &[]).await?;
                tracing::info!("MSSQL database '{}' ensured", db_name);
            }

            let addr = tiberius_config.get_addr().to_owned();
            let tcp = tokio::net::TcpStream::connect(addr).await?;
            tcp.set_nodelay(true)?;

            let mut client = tiberius::Client::connect(
                tiberius_config,
                tokio_util::compat::TokioAsyncWriteCompatExt::compat_write(tcp),
            )
            .await?;

            let report = mssql_migrations::migrations::runner()
                .run_async(&mut client)
                .await?;

            for migration in report.applied_migrations() {
                tracing::info!(
                    "MSSQL migration applied: {} {}",
                    migration.version(),
                    migration.name()
                );
            }

            tracing::info!("MSSQL migrations complete");
        }
    }

    Ok(())
}
