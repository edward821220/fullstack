use std::sync::Arc;

use axum::{Json, response::Response};
use dto::HealthResponse;
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
        version: env!("CARGO_PKG_VERSION").to_owned(),
    })
}

pub async fn health_ready(svc: Arc<UserService>) -> Result<Json<HealthResponse>, Response> {
    match svc.health_check().await {
        Ok(_) => Ok(Json(HealthResponse {
            status: "ready".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
        })),
        Err(e) => {
            tracing::error!("Health ready check failed: {e}");
            Err(Response::from(ProblemResponse::service_unavailable(
                format!("Database health check failed: {e}"),
            )))
        }
    }
}
