pub mod client;
pub mod layer;
pub mod validator;

use crate::problem::ProblemResponse;
use axum::response::{IntoResponse, Response};
pub use validator::{Claims, OidcValidator};

#[derive(Debug, Clone)]
pub enum AuthFailure {
    Unauthorized(String),
    Forbidden(String),
    Internal(String),
}

impl std::fmt::Display for AuthFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthFailure::Unauthorized(detail) => write!(f, "Unauthorized: {detail}"),
            AuthFailure::Forbidden(detail) => write!(f, "Forbidden: {detail}"),
            AuthFailure::Internal(detail) => write!(f, "Internal: {detail}"),
        }
    }
}

impl IntoResponse for AuthFailure {
    fn into_response(self) -> Response {
        let response = match self {
            AuthFailure::Unauthorized(detail) => ProblemResponse::unauthorized(detail),
            AuthFailure::Forbidden(detail) => ProblemResponse::forbidden(detail),
            AuthFailure::Internal(detail) => ProblemResponse::internal_error(detail),
        };
        response.into_response()
    }
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: uuid::Uuid,
    pub email: String,
    pub display_name: String,
    pub role: model::role::Role,
    pub sub: String,
}

/// Marker inserted by the OIDC middleware when authentication is disabled.
/// Allows downstream authz middleware to distinguish "auth disabled" from
/// "auth enabled but identity missing".
#[derive(Debug, Clone)]
pub struct AuthDisabledMarker;
