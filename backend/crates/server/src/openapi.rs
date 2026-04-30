use utoipa::OpenApi;

use crate::handlers;

#[derive(OpenApi)]
#[openapi(
    paths(
        handlers::users::list_users,
        handlers::users::get_user,
        handlers::users::create_user,
        handlers::users::update_user,
        handlers::users::delete_user,
        handlers::health::health,
    ),
    components(schemas(
        dto::UserResponse,
        dto::CreateUserRequest,
        dto::UpdateUserRequest,
        dto::HealthResponse,
        dto::ErrorResponse,
        dto::PaginatedUserResponse,
    )),
    tags(
        (name = "users", description = "User management endpoints"),
        (name = "health", description = "Health check endpoints"),
    )
)]
pub struct ApiDoc;
