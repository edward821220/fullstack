use std::time::Duration;

use config::DatabaseConfig;

use crate::user_repo::AnyUserRepo;
use crate::user_repo::mssql::MssqlUserRepo;
use crate::user_repo::postgres::PostgresUserRepo;
use crate::{Error, Result};

pub async fn connect(config: &DatabaseConfig) -> Result<AnyUserRepo> {
    use config::DatabaseDriver;

    match config.driver() {
        DatabaseDriver::Postgres => {
            let url = config.to_postgres_url();
            let pool = sqlx::postgres::PgPoolOptions::new()
                .max_connections(config.max_connections)
                .acquire_timeout(Duration::from_secs(10))
                .connect(&url)
                .await
                .map_err(|e| Error::Database {
                    message: e.to_string(),
                })?;

            Ok(AnyUserRepo::Postgres(PostgresUserRepo::new(pool)))
        }
        DatabaseDriver::Mssql => {
            let tiberius_config = config
                .to_tiberius_config()
                .map_err(|e| Error::Database { message: e })?;
            let mgr = bb8_tiberius::ConnectionManager::new(tiberius_config.clone());
            let pool = bb8::Pool::builder()
                .max_size(config.max_connections)
                .build(mgr)
                .await
                .map_err(|e| Error::Database {
                    message: e.to_string(),
                })?;

            Ok(AnyUserRepo::Mssql(MssqlUserRepo::new(
                pool,
                tiberius_config,
            )))
        }
    }
}
