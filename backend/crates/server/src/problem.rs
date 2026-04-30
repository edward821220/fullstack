use axum::{Json, http::StatusCode, response::IntoResponse};

pub struct ProblemResponse {
    status: StatusCode,
    detail: String,
}

impl ProblemResponse {
    pub fn new(status: StatusCode, detail: impl Into<String>) -> Self {
        Self {
            status,
            detail: detail.into(),
        }
    }

    pub fn unauthorized(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, detail)
    }

    pub fn forbidden(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, detail)
    }

    pub fn internal_error(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, detail)
    }

    pub fn service_unavailable(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::SERVICE_UNAVAILABLE, detail)
    }
}

impl IntoResponse for ProblemResponse {
    fn into_response(self) -> axum::response::Response {
        let body = serde_json::json!({
            "type": "about:blank",
            "title": self.status.canonical_reason().unwrap_or("Unknown"),
            "status": self.status.as_u16(),
            "detail": self.detail,
        });
        (self.status, Json(body)).into_response()
    }
}

impl From<ProblemResponse> for axum::response::Response {
    fn from(p: ProblemResponse) -> Self {
        p.into_response()
    }
}
