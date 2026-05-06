pub use model::role::Role;

#[derive(Debug, Clone)]
pub enum AuthzError {
    Forbidden(String),
}

pub fn authorize_role(role: &Role, minimum_role: &Role) -> Result<(), AuthzError> {
    if role.has_permission(minimum_role) {
        Ok(())
    } else {
        Err(AuthzError::Forbidden(format!(
            "Role '{}' is not authorized for this operation (requires '{}' or higher)",
            role.as_str(),
            minimum_role.as_str()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin_can_access_all_roles() {
        assert!(Role::Admin.has_permission(&Role::Admin));
        assert!(Role::Admin.has_permission(&Role::Manager));
        assert!(Role::Admin.has_permission(&Role::User));
    }

    #[test]
    fn manager_can_access_manager_and_user() {
        assert!(Role::Manager.has_permission(&Role::Manager));
        assert!(Role::Manager.has_permission(&Role::User));
        assert!(!Role::Manager.has_permission(&Role::Admin));
    }

    #[test]
    fn user_can_only_access_user() {
        assert!(Role::User.has_permission(&Role::User));
        assert!(!Role::User.has_permission(&Role::Manager));
        assert!(!Role::User.has_permission(&Role::Admin));
    }

    #[test]
    fn authorize_role_should_allow_sufficient_role() {
        assert!(authorize_role(&Role::Admin, &Role::Manager).is_ok());
        assert!(authorize_role(&Role::Manager, &Role::User).is_ok());
    }

    #[test]
    fn authorize_role_should_deny_insufficient_role() {
        assert!(authorize_role(&Role::User, &Role::Admin).is_err());
        assert!(authorize_role(&Role::Manager, &Role::Admin).is_err());
    }
}
