use std::sync::Arc;

use repo::HealthProbe;
use svc::health::{HealthChecker, HealthError};

/// Database health-check adapter.
///
/// Holds a type-erased [`HealthProbe`] and adapts it to the [`HealthChecker`] seam.
/// This module knows nothing about `UserRepo` or `AnyUserRepo`;
/// it only knows how to call `ping()` on a database connection pool.
#[derive(Clone)]
pub struct DbHealthChecker {
    probe: Arc<dyn HealthProbe>,
}

impl DbHealthChecker {
    pub fn new(probe: Arc<dyn HealthProbe>) -> Self {
        Self { probe }
    }
}

#[async_trait::async_trait]
impl HealthChecker for DbHealthChecker {
    async fn check(&self) -> Result<(), HealthError> {
        self.probe
            .ping()
            .await
            .map_err(|e| HealthError::CheckFailed {
                message: e.to_string(),
            })
    }
}

/// Always-healthy adapter for tests.
pub struct AlwaysHealthy;

#[async_trait::async_trait]
impl HealthChecker for AlwaysHealthy {
    async fn check(&self) -> Result<(), HealthError> {
        Ok(())
    }
}
