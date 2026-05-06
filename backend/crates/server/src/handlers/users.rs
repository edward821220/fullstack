use crate::middleware::oidc::AuthUser;
use crate::middleware::{require_admin, require_manager};
use crate::problem::ProblemResponse;
use crate::state::AppState;
use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    middleware as axum_middleware,
    routing::{delete, get, post},
};
use dto::{
    CreateUserRequest, ErrorResponse, PaginatedUserResponse, PaginationParams, UpdateUserRequest,
    UserResponse,
};
use snafu::Snafu;
use std::sync::Arc;
use svc::AuditEvent;
use svc::UserServiceTrait;
use uuid::Uuid;

#[derive(Debug, Snafu)]
pub enum UsersError {
    #[snafu(display("User not found: {id}"))]
    UserNotFound { id: Uuid },
    #[snafu(display("Invalid input: {message}"))]
    InvalidInput { message: String },
    #[snafu(display("Conflict: {resource} was modified (expected version {expected_version})"))]
    Conflict {
        resource: String,
        expected_version: i64,
    },
    #[snafu(display("Internal error: {source}"))]
    Internal { source: svc::Error },
}

impl From<svc::Error> for UsersError {
    fn from(source: svc::Error) -> Self {
        match &source {
            svc::Error::NotFound { id } => UsersError::UserNotFound { id: *id },
            svc::Error::InvalidInput { message } => UsersError::InvalidInput {
                message: message.clone(),
            },
            svc::Error::Conflict {
                resource,
                expected_version,
            } => UsersError::Conflict {
                resource: resource.clone(),
                expected_version: *expected_version,
            },
            _ => UsersError::Internal { source },
        }
    }
}

impl axum::response::IntoResponse for UsersError {
    fn into_response(self) -> axum::response::Response {
        let (status, detail) = match &self {
            UsersError::UserNotFound { id } => (
                StatusCode::NOT_FOUND,
                format!("User with id {} not found", id),
            ),
            UsersError::InvalidInput { message } => (StatusCode::BAD_REQUEST, message.clone()),
            UsersError::Conflict {
                resource,
                expected_version,
            } => (
                StatusCode::CONFLICT,
                format!(
                    "{} was modified concurrently (expected version {})",
                    resource, expected_version
                ),
            ),
            UsersError::Internal { .. } => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_owned(),
            ),
        };

        ProblemResponse::new(status, detail).into_response()
    }
}

pub fn routes(state: Arc<AppState>) -> Router {
    Router::new()
        .route(
            "/users",
            get(list_users).layer(axum_middleware::from_fn_with_state(
                state.clone(),
                require_manager,
            )),
        )
        .route(
            "/users",
            post(create_user).layer(axum_middleware::from_fn_with_state(
                state.clone(),
                require_admin,
            )),
        )
        .route(
            "/users/{id}",
            get(get_user)
                .put(update_user)
                .layer(axum_middleware::from_fn_with_state(
                    state.clone(),
                    require_manager,
                )),
        )
        .route(
            "/users/{id}",
            delete(delete_user).layer(axum_middleware::from_fn_with_state(
                state.clone(),
                require_admin,
            )),
        )
        .with_state(state)
}

fn to_response(user: &model::user::User) -> UserResponse {
    UserResponse::from(user)
}

#[utoipa::path(
    get,
    path = "/api/v1/users",
    tag = "users",
    security(
        ("bearer_auth" = []),
    ),
    params(
        ("page" = Option<u64>, Query, description = "Page number (1-based)"),
        ("per_page" = Option<u64>, Query, description = "Items per page (1-100)"),
    ),
    responses(
        (status = 200, description = "Paginated list of users", body = PaginatedUserResponse),
        (status = 400, description = "Invalid pagination parameters", body = ErrorResponse),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse),
        (status = 403, description = "Insufficient role (requires manager)", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse),
    )
)]
async fn list_users(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedUserResponse>, UsersError> {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(20);

    if page < 1 {
        return Err(UsersError::InvalidInput {
            message: "page must be >= 1".to_owned(),
        });
    }
    if !(1..=100).contains(&per_page) {
        return Err(UsersError::InvalidInput {
            message: "per_page must be between 1 and 100".to_owned(),
        });
    }

    let (users, total) = state.svc.list_users(page, per_page).await?;

    let data: Vec<UserResponse> = users.iter().map(to_response).collect();

    Ok(Json(PaginatedUserResponse {
        data,
        total,
        page,
        per_page,
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/users/{id}",
    tag = "users",
    security(
        ("bearer_auth" = []),
    ),
    params(
        ("id" = Uuid, Path, description = "User ID"),
    ),
    responses(
        (status = 200, description = "User found", body = UserResponse),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse),
        (status = 403, description = "Insufficient role (requires manager)", body = ErrorResponse),
        (status = 404, description = "User not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse),
    )
)]
async fn get_user(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<UserResponse>, UsersError> {
    let user = state.svc.get_user(id).await?;

    Ok(Json(to_response(&user)))
}

#[utoipa::path(
    post,
    path = "/api/v1/users",
    tag = "users",
    security(
        ("bearer_auth" = []),
    ),
    request_body = CreateUserRequest,
    responses(
        (status = 201, description = "User created", body = UserResponse),
        (status = 400, description = "Invalid input", body = ErrorResponse),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse),
        (status = 403, description = "Insufficient role (requires admin)", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse),
    )
)]
async fn create_user(
    State(state): State<Arc<AppState>>,
    actor: Option<Extension<AuthUser>>,
    Json(req): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<UserResponse>), UsersError> {
    if let Err(e) = req.validate() {
        return Err(UsersError::InvalidInput { message: e });
    }
    let user = state
        .svc
        .create_user(
            &req.email,
            &req.display_name,
            model::role::Role::User,
            false,
        )
        .await?;

    if let Some(Extension(actor)) = actor {
        state.audit.record(AuditEvent::UserCreated {
            actor_id: actor.user_id,
            created_id: user.id,
            email: user.email.clone(),
        });
    }

    Ok((StatusCode::CREATED, Json(to_response(&user))))
}

#[utoipa::path(
    put,
    path = "/api/v1/users/{id}",
    tag = "users",
    security(
        ("bearer_auth" = []),
    ),
    params(
        ("id" = Uuid, Path, description = "User ID"),
    ),
    request_body = UpdateUserRequest,
    responses(
        (status = 200, description = "User updated", body = UserResponse),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse),
        (status = 403, description = "Insufficient role (requires manager)", body = ErrorResponse),
        (status = 404, description = "User not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse),
    )
)]
async fn update_user(
    State(state): State<Arc<AppState>>,
    actor: Option<Extension<AuthUser>>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateUserRequest>,
) -> Result<Json<UserResponse>, UsersError> {
    if let Err(e) = req.validate() {
        return Err(UsersError::InvalidInput { message: e });
    }
    let user = state
        .svc
        .update_user(id, req.display_name.as_deref())
        .await?;

    if let Some(Extension(actor)) = actor {
        state.audit.record(AuditEvent::UserUpdated {
            actor_id: actor.user_id,
            target_id: user.id,
        });
    }

    Ok(Json(to_response(&user)))
}

#[utoipa::path(
    delete,
    path = "/api/v1/users/{id}",
    tag = "users",
    security(
        ("bearer_auth" = []),
    ),
    params(
        ("id" = Uuid, Path, description = "User ID"),
    ),
    responses(
        (status = 204, description = "User deleted"),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse),
        (status = 403, description = "Insufficient role (requires admin)", body = ErrorResponse),
        (status = 404, description = "User not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse),
    )
)]
async fn delete_user(
    State(state): State<Arc<AppState>>,
    actor: Option<Extension<AuthUser>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, UsersError> {
    state.svc.delete_user(id).await?;

    if let Some(Extension(actor)) = actor {
        state.audit.record(AuditEvent::UserDeleted {
            actor_id: actor.user_id,
            target_id: id,
        });
    }

    Ok(StatusCode::NO_CONTENT)
}
