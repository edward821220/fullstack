use crate::health::HealthProbe;
use crate::user_repo::AnyUserRepo;
use crate::user_repo::mssql::MssqlUserRepo;
use crate::user_repo::postgres::PostgresUserRepo;
use crate::{Error, Result};
use config::DatabaseConfig;
use metrics::gauge;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

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

/// Connect to the database and return the repo, health probe, and a handle to the
/// background pool-metrics task.
///
/// The caller owns the metrics task lifecycle via the returned `JoinHandle` and the
/// `CancellationToken` passed in.
pub async fn connect(
    config: &DatabaseConfig,
    cancel: CancellationToken,
    metrics_interval: Duration,
) -> Result<(
    AnyUserRepo,
    Arc<dyn HealthProbe>,
    tokio::task::JoinHandle<()>,
)> {
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

            let metrics_handle =
                spawn_postgres_pool_metrics(pool.clone(), cancel, metrics_interval);

            let probe: Arc<dyn HealthProbe> = Arc::new(pool.clone());
            let repo = AnyUserRepo::Postgres(PostgresUserRepo::new(pool));
            Ok((repo, probe, metrics_handle))
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

            let metrics_handle = spawn_mssql_pool_metrics(pool.clone(), cancel, metrics_interval);

            let probe: Arc<dyn HealthProbe> = Arc::new(pool.clone());
            let repo = AnyUserRepo::Mssql(MssqlUserRepo::new(pool, tiberius_config));
            Ok((repo, probe, metrics_handle))
        }
    }
}

fn spawn_postgres_pool_metrics(
    pool: sqlx::PgPool,
    cancel: CancellationToken,
    interval: Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        let size = pool.size() as f64;
        let idle = pool.num_idle() as f64;
        gauge!("db_pool_size", "driver" => "postgres").set(size);
        gauge!("db_pool_idle", "driver" => "postgres").set(idle);
        gauge!("db_pool_active", "driver" => "postgres").set(size - idle);

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let size = pool.size() as f64;
                    let idle = pool.num_idle() as f64;
                    gauge!("db_pool_size", "driver" => "postgres").set(size);
                    gauge!("db_pool_idle", "driver" => "postgres").set(idle);
                    gauge!("db_pool_active", "driver" => "postgres").set(size - idle);
                }
                _ = cancel.cancelled() => break,
            }
        }
    })
}

fn spawn_mssql_pool_metrics(
    pool: bb8::Pool<bb8_tiberius::ConnectionManager>,
    cancel: CancellationToken,
    interval: Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        let state = pool.state();
        let size = state.connections as f64;
        let idle = state.idle_connections as f64;
        gauge!("db_pool_size", "driver" => "mssql").set(size);
        gauge!("db_pool_idle", "driver" => "mssql").set(idle);
        gauge!("db_pool_active", "driver" => "mssql").set(size - idle);

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let state = pool.state();
                    let size = state.connections as f64;
                    let idle = state.idle_connections as f64;
                    gauge!("db_pool_size", "driver" => "mssql").set(size);
                    gauge!("db_pool_idle", "driver" => "mssql").set(idle);
                    gauge!("db_pool_active", "driver" => "mssql").set(size - idle);
                }
                _ = cancel.cancelled() => break,
            }
        }
    })
}
