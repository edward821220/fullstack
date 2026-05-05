use std::str::FromStr;

#[derive(Debug, Clone)]
pub enum AuthzError {
    Forbidden(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    Admin,
    Manager,
    User,
}

impl FromStr for Role {
    type Err = ();

    fn from_str(role: &str) -> Result<Self, Self::Err> {
        match role {
            "admin" => Ok(Role::Admin),
            "manager" => Ok(Role::Manager),
            "user" => Ok(Role::User),
            _ => Err(()),
        }
    }
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Admin => "admin",
            Role::Manager => "manager",
            Role::User => "user",
        }
    }
}

pub fn has_permission(user_role: &Role, required_role: &Role) -> bool {
    matches!(
        (user_role, required_role),
        (Role::Admin, _) | (Role::Manager, Role::Manager | Role::User) | (Role::User, Role::User)
    )
}

pub fn authorize_role(role: &str, minimum_role: &Role) -> Result<(), AuthzError> {
    let user_role = Role::from_str(role).map_err(|_| {
        AuthzError::Forbidden(format!(
            "The role '{role}' assigned to your identity is not recognized by this system"
        ))
    })?;

    if has_permission(&user_role, minimum_role) {
        Ok(())
    } else {
        Err(AuthzError::Forbidden(format!(
            "Role '{}' is not authorized for this operation (requires '{}' or higher)",
            user_role.as_str(),
            minimum_role.as_str()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin_can_access_all_roles() {
        assert!(has_permission(&Role::Admin, &Role::Admin));
        assert!(has_permission(&Role::Admin, &Role::Manager));
        assert!(has_permission(&Role::Admin, &Role::User));
    }

    #[test]
    fn manager_can_access_manager_and_user() {
        assert!(has_permission(&Role::Manager, &Role::Manager));
        assert!(has_permission(&Role::Manager, &Role::User));
        assert!(!has_permission(&Role::Manager, &Role::Admin));
    }

    #[test]
    fn user_can_only_access_user() {
        assert!(has_permission(&Role::User, &Role::User));
        assert!(!has_permission(&Role::User, &Role::Manager));
        assert!(!has_permission(&Role::User, &Role::Admin));
    }

    #[test]
    fn role_from_str_should_parse_known_roles() {
        assert_eq!(Role::from_str("admin"), Ok(Role::Admin));
        assert_eq!(Role::from_str("manager"), Ok(Role::Manager));
        assert_eq!(Role::from_str("user"), Ok(Role::User));
        assert_eq!(Role::from_str("unknown"), Err(()));
    }
}
