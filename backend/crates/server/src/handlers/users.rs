use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    middleware as axum_middleware,
    routing::{delete, get, post},
};
use dto::{
    CreateUserRequest, PaginatedUserResponse, PaginationParams, UpdateUserRequest, UserResponse,
};
use snafu::Snafu;
use std::sync::Arc;
use uuid::Uuid;

use crate::middleware::{AppState, require_admin, require_manager};
use crate::problem::ProblemResponse;
use svc::UserServiceTrait;

#[derive(Debug, Snafu)]
pub enum UsersError {
    #[snafu(display("User not found: {id}"))]
    UserNotFound { id: Uuid },
    #[snafu(display("Invalid input: {message}"))]
    InvalidInput { message: String },
    #[snafu(display("Internal error: {source}"))]
    Internal { source: svc::Error },
}

impl axum::response::IntoResponse for UsersError {
    fn into_response(self) -> axum::response::Response {
        let (status, detail) = match &self {
            UsersError::UserNotFound { id } => (
                StatusCode::NOT_FOUND,
                format!("User with id {} not found", id),
            ),
            UsersError::InvalidInput { message } => (StatusCode::BAD_REQUEST, message.clone()),
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
            get(list_users).layer(axum_middleware::from_fn(require_manager)),
        )
        .route(
            "/users",
            post(create_user).layer(axum_middleware::from_fn(require_admin)),
        )
        .route(
            "/users/{id}",
            get(get_user)
                .put(update_user)
                .layer(axum_middleware::from_fn(require_manager)),
        )
        .route(
            "/users/{id}",
            delete(delete_user).layer(axum_middleware::from_fn(require_admin)),
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
    params(
        ("page" = Option<u64>, Query, description = "Page number (1-based)"),
        ("per_page" = Option<u64>, Query, description = "Items per page (1-100)"),
    ),
    responses(
        (status = 200, description = "Paginated list of users", body = PaginatedUserResponse),
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

    let (users, total) = state
        .svc
        .list_users(page, per_page)
        .await
        .map_err(|e| UsersError::Internal { source: e })?;

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
    params(
        ("id" = Uuid, Path, description = "User ID"),
    ),
    responses(
        (status = 200, description = "User found", body = UserResponse),
        (status = 404, description = "User not found"),
    )
)]
async fn get_user(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<UserResponse>, UsersError> {
    let user = state.svc.get_user(id).await.map_err(|e| match &e {
        svc::Error::NotFound { id } => UsersError::UserNotFound { id: *id },
        _ => UsersError::Internal { source: e },
    })?;

    Ok(Json(to_response(&user)))
}

#[utoipa::path(
    post,
    path = "/api/v1/users",
    tag = "users",
    request_body = CreateUserRequest,
    responses(
        (status = 201, description = "User created", body = UserResponse),
        (status = 400, description = "Invalid input"),
    )
)]
async fn create_user(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<UserResponse>), UsersError> {
    let user = state
        .svc
        .create_user(&req.email, &req.display_name, "user", false)
        .await
        .map_err(|e| match &e {
            svc::Error::InvalidInput { message } => UsersError::InvalidInput {
                message: message.clone(),
            },
            _ => UsersError::Internal { source: e },
        })?;

    Ok((StatusCode::CREATED, Json(to_response(&user))))
}

#[utoipa::path(
    put,
    path = "/api/v1/users/{id}",
    tag = "users",
    params(
        ("id" = Uuid, Path, description = "User ID"),
    ),
    request_body = UpdateUserRequest,
    responses(
        (status = 200, description = "User updated", body = UserResponse),
        (status = 404, description = "User not found"),
    )
)]
async fn update_user(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateUserRequest>,
) -> Result<Json<UserResponse>, UsersError> {
    let user = state
        .svc
        .update_user(id, req.display_name.as_deref())
        .await
        .map_err(|e| match &e {
            svc::Error::NotFound { id } => UsersError::UserNotFound { id: *id },
            _ => UsersError::Internal { source: e },
        })?;

    Ok(Json(to_response(&user)))
}

#[utoipa::path(
    delete,
    path = "/api/v1/users/{id}",
    tag = "users",
    params(
        ("id" = Uuid, Path, description = "User ID"),
    ),
    responses(
        (status = 204, description = "User deleted"),
        (status = 404, description = "User not found"),
    )
)]
async fn delete_user(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, UsersError> {
    state.svc.delete_user(id).await.map_err(|e| match &e {
        svc::Error::NotFound { id } => UsersError::UserNotFound { id: *id },
        _ => UsersError::Internal { source: e },
    })?;

    Ok(StatusCode::NO_CONTENT)
}
