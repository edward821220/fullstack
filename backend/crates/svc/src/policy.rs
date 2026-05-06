/// Information extracted from an OIDC token about the authenticated user.
pub struct OidcUserInfo {
    pub sub: String,
    pub issuer: String,
    pub email: String,
    pub name: String,
    pub email_verified: bool,
    pub roles: Vec<String>,
}

/// Policy that governs how an external OIDC user is mapped to a local `User`.
pub struct ProvisioningPolicy {
    pub allowed_email_domains: Vec<String>,
    default_role: String,
}

impl ProvisioningPolicy {
    pub fn new(allowed_email_domains: Vec<String>, default_role: String) -> Self {
        Self {
            allowed_email_domains,
            default_role,
        }
    }

    pub fn check_email_domain(&self, email: &str) -> crate::Result<()> {
        if self.allowed_email_domains.is_empty() {
            return Ok(());
        }
        let email_domain = email.split('@').nth(1).unwrap_or("").to_lowercase();
        let allowed = self
            .allowed_email_domains
            .iter()
            .any(|d| d.to_lowercase() == email_domain || d.to_lowercase() == email.to_lowercase());
        if !allowed {
            return Err(crate::Error::NotInWhitelist {
                email: email.to_owned(),
            });
        }
        Ok(())
    }

    pub fn resolve_role(&self, oidc_roles: &[String]) -> model::role::Role {
        let admin_set = ["admin", "administrator", "superuser"];
        let manager_set = ["manager", "supervisor"];

        for role in oidc_roles {
            let lower = role.to_lowercase();
            if admin_set.iter().any(|&i| i == lower) {
                return model::role::Role::Admin;
            }
            if manager_set.iter().any(|&i| i == lower) {
                return model::role::Role::Manager;
            }
        }

        self.default_role.parse().unwrap_or(model::role::Role::User)
    }
}

pub fn derive_provider_from_issuer(issuer: &str) -> &str {
    issuer
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or("unknown")
}
