use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserResponse {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub role: String,
    pub email_verified: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<&model::user::User> for UserResponse {
    fn from(user: &model::user::User) -> Self {
        Self {
            id: user.id,
            email: user.email.clone(),
            display_name: user.display_name.clone(),
            role: user.role.clone(),
            email_verified: user.email_verified,
            created_at: user
                .created_at
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_default(),
            updated_at: user
                .updated_at
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateUserRequest {
    #[schema(format = Email, min_length = 1, max_length = 100)]
    pub email: String,
    #[schema(min_length = 1, max_length = 100)]
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateUserRequest {
    #[schema(min_length = 1, max_length = 100)]
    pub display_name: Option<String>,
}
