use utoipa::OpenApi;
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};

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
        handlers::health::health_ready,
    ),
    components(schemas(
        dto::UserResponse,
        dto::CreateUserRequest,
        dto::UpdateUserRequest,
        dto::HealthResponse,
        dto::ErrorResponse,
        dto::PaginatedUserResponse,
    )),
    modifiers(&SecurityAddon),
    tags(
        (name = "users", description = "User management endpoints"),
        (name = "health", description = "Health check endpoints"),
    )
)]
pub struct ApiDoc;

struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .description(Some("OIDC Bearer JWT token obtained from the IdP"))
                        .build(),
                ),
            );
        }
    }
}
