pub mod layer;

pub use crate::authz::domain::{AuthzError, Role, authorize_role};
use axum::response::{IntoResponse, Response};

use crate::problem::ProblemResponse;

impl IntoResponse for AuthzError {
    fn into_response(self) -> Response {
        let AuthzError::Forbidden(detail) = self;
        ProblemResponse::forbidden(detail).into_response()
    }
}
