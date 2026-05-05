use server::middleware::oidc::AuthUser;
use uuid::Uuid;

pub fn make_auth_user(role: &str) -> AuthUser {
    AuthUser {
        user_id: Uuid::new_v4(),
        email: format!("{role}@example.com"),
        display_name: format!("{role}-user"),
        role: role.to_owned(),
        sub: format!("sub-{role}"),
    }
}
