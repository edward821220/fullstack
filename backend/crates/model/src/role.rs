use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Hierarchical role used across the domain layer.
///
/// Ordering: Admin > Manager > User.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Admin,
    Manager,
    User,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Admin => "admin",
            Role::Manager => "manager",
            Role::User => "user",
        }
    }

    /// Returns true if this role has permission to access a resource
    /// that requires `required_role`.
    pub fn has_permission(&self, required_role: &Role) -> bool {
        matches!(
            (self, required_role),
            (Role::Admin, _)
                | (Role::Manager, Role::Manager | Role::User)
                | (Role::User, Role::User)
        )
    }
}

impl FromStr for Role {
    type Err = UnknownRoleError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "admin" => Ok(Role::Admin),
            "manager" => Ok(Role::Manager),
            "user" => Ok(Role::User),
            _ => Err(UnknownRoleError(s.to_owned())),
        }
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnknownRoleError(pub String);

impl fmt::Display for UnknownRoleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Unknown role: '{}'", self.0)
    }
}

impl std::error::Error for UnknownRoleError {}

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
    fn role_from_str_should_parse_known_roles() {
        assert_eq!("admin".parse::<Role>(), Ok(Role::Admin));
        assert_eq!("manager".parse::<Role>(), Ok(Role::Manager));
        assert_eq!("user".parse::<Role>(), Ok(Role::User));
    }

    #[test]
    fn role_from_str_should_reject_unknown() {
        assert!("superadmin".parse::<Role>().is_err());
    }

    #[test]
    fn role_from_str_should_be_case_insensitive() {
        assert_eq!("Admin".parse::<Role>(), Ok(Role::Admin));
        assert_eq!("MANAGER".parse::<Role>(), Ok(Role::Manager));
    }
}
