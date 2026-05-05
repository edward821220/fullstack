use std::sync::Arc;

use axum::{Json, response::Response};
use dto::{ErrorResponse, HealthResponse};
use repo::AnyUserRepo;
use svc::{UserService, UserServiceTrait};

use crate::problem::ProblemResponse;

#[utoipa::path(
    get,
    path = "/health",
    tag = "health",
    responses(
        (status = 200, description = "Service is alive", body = HealthResponse),
    )
)]
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_owned(),
    })
}

#[utoipa::path(
    get,
    path = "/health/ready",
    tag = "health",
    responses(
        (status = 200, description = "Service is ready (database reachable)", body = HealthResponse),
        (status = 503, description = "Service is not ready", body = ErrorResponse),
    )
)]
pub async fn health_ready(
    svc: Arc<UserService<AnyUserRepo>>,
) -> Result<Json<HealthResponse>, Response> {
    match svc.health_check().await {
        Ok(_) => Ok(Json(HealthResponse {
            status: "ready".to_owned(),
        })),
        Err(e) => {
            tracing::error!("Health ready check failed: {e}");
            Err(Response::from(ProblemResponse::service_unavailable(
                "Service is not ready".to_owned(),
            )))
        }
    }
}
