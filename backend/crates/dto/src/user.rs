use model::role::Role;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserResponse {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub role: Role,
    pub email_verified: bool,
    pub created_at: String,
    pub updated_at: String,
    pub version: i64,
}

impl From<&model::user::User> for UserResponse {
    fn from(user: &model::user::User) -> Self {
        Self {
            id: user.id,
            email: user.email.clone(),
            display_name: user.display_name.clone(),
            role: user.role,
            email_verified: user.email_verified,
            created_at: user
                .created_at
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_default(),
            updated_at: user
                .updated_at
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_default(),
            version: user.version,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateUserRequest {
    #[schema(format = Email, min_length = 1, max_length = 100)]
    pub email: String,
    #[schema(min_length = 1, max_length = 100)]
    pub display_name: String,
}

/// Simple but stricter email format check (must have local@domain.tld).
fn is_valid_email_format(email: &str) -> bool {
    // RFC 5322-ish: one @, non-empty local and domain parts, domain has at least one dot.
    let email = email.trim();
    if email.len() > 100 {
        return false;
    }
    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() != 2 {
        return false;
    }
    let local = parts[0];
    let domain = parts[1];
    if local.is_empty() || domain.is_empty() {
        return false;
    }
    if !domain.contains('.') {
        return false;
    }
    // Reject obvious control characters or whitespace inside the string
    if email.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return false;
    }
    true
}

impl CreateUserRequest {
    pub fn validate(&self) -> Result<(), String> {
        let email = self.email.trim();
        if email.is_empty() {
            return Err("Email is required".to_owned());
        }
        if !is_valid_email_format(&self.email) {
            return Err("Email format is invalid".to_owned());
        }
        let name = self.display_name.trim();
        if name.is_empty() {
            return Err("Display name is required".to_owned());
        }
        if name.len() > 100 {
            return Err("Display name must not exceed 100 characters".to_owned());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateUserRequest {
    #[schema(min_length = 1, max_length = 100)]
    pub display_name: Option<String>,
}

impl UpdateUserRequest {
    pub fn validate(&self) -> Result<(), String> {
        if let Some(ref name) = self.display_name {
            let name = name.trim();
            if name.is_empty() {
                return Err("Display name cannot be blank".to_owned());
            }
            if name.len() > 100 {
                return Err("Display name must not exceed 100 characters".to_owned());
            }
        }
        Ok(())
    }
}
