use std::time::Duration;

use config::DatabaseConfig;

use crate::user_repo::mssql::MssqlUserRepo;
use crate::user_repo::postgres::PostgresUserRepo;
use crate::{Error, Result, UserRepo};

pub async fn connect(config: &DatabaseConfig) -> Result<Box<dyn UserRepo>> {
    use config::DatabaseDriver;

    match config.driver() {
        DatabaseDriver::Postgres => {
            let pool = sqlx::postgres::PgPoolOptions::new()
                .max_connections(config.max_connections)
                .acquire_timeout(Duration::from_secs(10))
                .connect(&config.database_url)
                .await
                .map_err(|e| Error::Database {
                    message: e.to_string(),
                })?;

            Ok(Box::new(PostgresUserRepo::new(pool)))
        }
        DatabaseDriver::Mssql => {
            let tiberius_config = config.to_tiberius_config().map_err(|e| Error::Database {
                message: e.to_string(),
            })?;
            let mgr = bb8_tiberius::ConnectionManager::new(tiberius_config);
            let pool = bb8::Pool::builder()
                .max_size(config.max_connections)
                .build(mgr)
                .await
                .map_err(|e| Error::Database {
                    message: e.to_string(),
                })?;

            Ok(Box::new(MssqlUserRepo::new(pool)))
        }
    }
}
