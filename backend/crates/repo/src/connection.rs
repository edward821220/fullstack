use crate::health::HealthProbe;
use crate::user_repo::AnyUserRepo;
use crate::user_repo::mssql::MssqlUserRepo;
use crate::user_repo::postgres::PostgresUserRepo;
use crate::{Error, Result};
use config::DatabaseConfig;
use std::sync::Arc;
use std::time::Duration;

/// Build a [`tiberius::Config`] from the neutral [`DatabaseConfig`].
///
/// This conversion lives in the `repo` crate (not `config`) so the neutral
/// config module does not depend on the MSSQL driver.
pub fn tiberius_config_from(cfg: &DatabaseConfig) -> std::result::Result<tiberius::Config, String> {
    if cfg.host.is_empty() {
        return Err("database host is empty".to_owned());
    }
    if cfg.database.is_empty() {
        return Err("database name is empty".to_owned());
    }

    let password = cfg.resolve_password();
    let mut config = tiberius::Config::new();
    config.host(&cfg.host);
    config.port(cfg.port);
    config.database(&cfg.database);
    config.authentication(tiberius::AuthMethod::sql_server(&cfg.username, &password));

    if cfg.encrypt {
        config.encryption(tiberius::EncryptionLevel::Required);
        if cfg.trust_cert {
            config.trust_cert();
        } else if let Some(ref ca_path) = cfg.ca_cert_path
            && !ca_path.is_empty()
        {
            config.trust_cert_ca(ca_path);
        }
    } else {
        config.encryption(tiberius::EncryptionLevel::NotSupported);
    }

    Ok(config)
}

pub async fn connect(config: &DatabaseConfig) -> Result<(AnyUserRepo, Arc<dyn HealthProbe>)> {
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

            let probe: Arc<dyn HealthProbe> = Arc::new(pool.clone());
            let repo = AnyUserRepo::Postgres(PostgresUserRepo::new(pool));
            Ok((repo, probe))
        }
        DatabaseDriver::Mssql => {
            let tiberius_config =
                tiberius_config_from(config).map_err(|e| Error::Database { message: e })?;
            let mgr = bb8_tiberius::ConnectionManager::new(tiberius_config.clone());
            let pool = bb8::Pool::builder()
                .max_size(config.max_connections)
                .build(mgr)
                .await
                .map_err(|e| Error::Database {
                    message: e.to_string(),
                })?;

            let probe: Arc<dyn HealthProbe> = Arc::new(pool.clone());
            let repo = AnyUserRepo::Mssql(MssqlUserRepo::new(pool, tiberius_config));
            Ok((repo, probe))
        }
    }
}
