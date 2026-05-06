use axum::{http::StatusCode, response::IntoResponse};

use crate::problem::ProblemResponse;

/// HTTP-facing wrapper around [`svc::Error`].
///
/// This is the single seam where domain errors become HTTP responses.
/// All handlers should return `Result<T, AppError>` instead of defining
/// their own per-handler error enums.
pub struct AppError(pub svc::Error);

impl From<svc::Error> for AppError {
    fn from(e: svc::Error) -> Self {
        Self(e)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, detail) = match &self.0 {
            svc::Error::NotFound { id } => (
                StatusCode::NOT_FOUND,
                format!("User with id {id} not found"),
            ),
            svc::Error::InvalidInput { message } => (StatusCode::BAD_REQUEST, message.clone()),
            svc::Error::Conflict {
                resource,
                expected_version,
            } => (
                StatusCode::CONFLICT,
                format!(
                    "{resource} was modified concurrently (expected version {expected_version})"
                ),
            ),
            svc::Error::NotInWhitelist { email } => (
                StatusCode::FORBIDDEN,
                format!("User with email {email} not in whitelist"),
            ),
            svc::Error::UserAlreadyExists { email } => (
                StatusCode::BAD_REQUEST,
                format!("User with email {email} already exists"),
            ),
            svc::Error::InvalidRole { role } => {
                (StatusCode::BAD_REQUEST, format!("Invalid role: {role}"))
            }
            svc::Error::Database { .. } | svc::Error::IdentityNotFound { .. } => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_owned(),
            ),
        };

        ProblemResponse::new(status, detail).into_response()
    }
}
