use async_trait::async_trait;

use crate::{Error, Result};

/// A lightweight infrastructure probe that verifies an external dependency
/// is reachable without exercising business logic.
#[async_trait]
pub trait HealthProbe: Send + Sync {
    async fn ping(&self) -> Result<()>;
}

#[async_trait]
impl HealthProbe for sqlx::PgPool {
    async fn ping(&self) -> Result<()> {
        sqlx::query("SELECT 1")
            .execute(self)
            .await
            .map_err(|e| Error::Database {
                message: e.to_string(),
            })?;
        Ok(())
    }
}

#[async_trait]
impl HealthProbe for bb8::Pool<bb8_tiberius::ConnectionManager> {
    async fn ping(&self) -> Result<()> {
        let mut client = self.get().await.map_err(|e| Error::Database {
            message: e.to_string(),
        })?;

        client
            .query("SELECT 1", &[])
            .await
            .map_err(|e| Error::Database {
                message: e.to_string(),
            })?;

        Ok(())
    }
}
