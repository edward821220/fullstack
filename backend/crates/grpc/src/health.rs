use crate::proto::health::v1::{
    HealthCheckRequest, HealthCheckResponse, health_service_server::HealthService,
};
use std::sync::Arc;
use tonic::{Request, Response, Status};

#[derive(Clone)]
pub struct HealthGrpcService {
    health: Arc<dyn svc::HealthChecker>,
}

impl HealthGrpcService {
    pub fn new(health: Arc<dyn svc::HealthChecker>) -> Self {
        Self { health }
    }
}

#[tonic::async_trait]
impl HealthService for HealthGrpcService {
    #[tracing::instrument(skip(self, _request))]
    async fn health_check(
        &self,
        _request: Request<HealthCheckRequest>,
    ) -> Result<Response<HealthCheckResponse>, Status> {
        self.health.check().await.map_err(|e| {
            tracing::error!("gRPC health check failed: {e}");
            Status::internal("Health check failed")
        })?;

        Ok(Response::new(HealthCheckResponse {
            status: "SERVING".to_owned(),
        }))
    }
}
