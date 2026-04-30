use async_trait::async_trait;
use model::user::User;
use repo::{Error as RepoError, UserRepo};
use snafu::Snafu;
use tracing::instrument;
use uuid::Uuid;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Repository error: {source}"))]
    Repository { source: RepoError },

    #[snafu(display("Invalid input: {message}"))]
    InvalidInput { message: String },

    #[snafu(display("User with id {id} not found"))]
    NotFound { id: Uuid },

    #[snafu(display("User with email {email} not in whitelist"))]
    NotInWhitelist { email: String },
}

impl From<RepoError> for Error {
    fn from(source: RepoError) -> Self {
        match &source {
            RepoError::UserNotFound { id } => Error::NotFound { id: *id },
            _ => Error::Repository { source },
        }
    }
}

pub struct UserService {
    repo: Box<dyn UserRepo>,
}

impl UserService {
    pub fn new(repo: Box<dyn UserRepo>) -> Self {
        Self { repo }
    }
}

pub struct OidcUserInfo {
    pub sub: String,
    pub issuer: String,
    pub email: String,
    pub name: String,
    pub email_verified: bool,
    pub roles: Vec<String>,
}

pub struct ProvisioningPolicy {
    pub allowed_email_domains: Vec<String>,
    pub default_role: String,
}

impl ProvisioningPolicy {
    pub fn new(allowed_email_domains: Vec<String>, default_role: String) -> Self {
        Self {
            allowed_email_domains,
            default_role,
        }
    }

    pub fn check_email_domain(&self, email: &str) -> Result<()> {
        if self.allowed_email_domains.is_empty() {
            return Ok(());
        }
        let email_domain = email.split('@').nth(1).unwrap_or("").to_lowercase();
        let allowed = self
            .allowed_email_domains
            .iter()
            .any(|d| d.to_lowercase() == email_domain || d.to_lowercase() == email.to_lowercase());
        if !allowed {
            return Err(Error::NotInWhitelist {
                email: email.to_owned(),
            });
        }
        Ok(())
    }

    pub fn resolve_role(&self, oidc_roles: &[String]) -> String {
        let admin_set = ["admin", "administrator", "superuser"];
        let manager_set = ["manager", "supervisor"];

        for role in oidc_roles {
            let lower = role.to_lowercase();
            if admin_set.iter().any(|&i| i == lower) {
                return "admin".to_owned();
            }
            if manager_set.iter().any(|&i| i == lower) {
                return "manager".to_owned();
            }
        }

        self.default_role.clone()
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

#[async_trait]
pub trait UserServiceTrait: Send + Sync {
    async fn get_user(&self, id: Uuid) -> Result<User>;
    async fn list_users(&self, page: u64, per_page: u64) -> Result<(Vec<User>, u64)>;
    async fn create_user(
        &self,
        email: &str,
        display_name: &str,
        role: &str,
        email_verified: bool,
    ) -> Result<User>;
    async fn update_user(&self, id: Uuid, display_name: Option<&str>) -> Result<User>;
    async fn delete_user(&self, id: Uuid) -> Result<()>;

    async fn provision_user(
        &self,
        oidc_info: &OidcUserInfo,
        policy: &ProvisioningPolicy,
    ) -> Result<User>;

    async fn health_check(&self) -> Result<()>;

    async fn sync_oidc_attributes(
        &self,
        id: Uuid,
        display_name: &str,
        role: &str,
        email_verified: bool,
    ) -> Result<User>;
}

#[async_trait]
impl UserServiceTrait for UserService {
    #[instrument(skip(self), fields(user_id = %id))]
    async fn get_user(&self, id: Uuid) -> Result<User> {
        self.repo
            .find_by_id(id)
            .await?
            .ok_or(Error::NotFound { id })
    }

    #[instrument(skip(self))]
    async fn list_users(&self, page: u64, per_page: u64) -> Result<(Vec<User>, u64)> {
        self.repo.list(page, per_page).await.map_err(Into::into)
    }

    #[instrument(skip(self), fields(email = %email))]
    async fn create_user(
        &self,
        email: &str,
        display_name: &str,
        role: &str,
        email_verified: bool,
    ) -> Result<User> {
        if email.is_empty() {
            return Err(Error::InvalidInput {
                message: "Email cannot be empty".to_owned(),
            });
        }
        Ok(self
            .repo
            .create(email, display_name, role, email_verified)
            .await?)
    }

    #[instrument(skip(self), fields(user_id = %id))]
    async fn update_user(&self, id: Uuid, display_name: Option<&str>) -> Result<User> {
        Ok(self.repo.update(id, display_name).await?)
    }

    #[instrument(skip(self), fields(user_id = %id))]
    async fn delete_user(&self, id: Uuid) -> Result<()> {
        Ok(self.repo.delete(id).await?)
    }

    #[instrument(skip(self, oidc_info, policy), fields(email = %oidc_info.email, sub = %oidc_info.sub, issuer = %oidc_info.issuer))]
    async fn provision_user(
        &self,
        oidc_info: &OidcUserInfo,
        policy: &ProvisioningPolicy,
    ) -> Result<User> {
        policy.check_email_domain(&oidc_info.email)?;

        let provider = derive_provider_from_issuer(&oidc_info.issuer);

        let existing = self
            .repo
            .find_by_identity(provider, &oidc_info.issuer, &oidc_info.sub)
            .await?;

        if let Some((user, _identity)) = existing {
            let role = policy.resolve_role(&oidc_info.roles);
            return self
                .repo
                .sync_oidc_attributes(user.id, &oidc_info.name, &role, oidc_info.email_verified)
                .await
                .map_err(Into::into);
        }

        let existing_user = self.repo.find_by_email(&oidc_info.email).await?;
        let role = policy.resolve_role(&oidc_info.roles);

        let user = match existing_user {
            Some(u) => u,
            None => {
                tracing::info!(
                    email = %oidc_info.email,
                    sub = %oidc_info.sub,
                    issuer = %oidc_info.issuer,
                    "Creating new user via JIT provisioning"
                );
                self.repo
                    .create(
                        &oidc_info.email,
                        &oidc_info.name,
                        &role,
                        oidc_info.email_verified,
                    )
                    .await?
            }
        };

        self.repo
            .create_identity(user.id, provider, &oidc_info.issuer, &oidc_info.sub)
            .await?;

        Ok(user)
    }

    async fn health_check(&self) -> Result<()> {
        self.repo.health_check().await.map_err(Into::into)
    }

    async fn sync_oidc_attributes(
        &self,
        id: Uuid,
        display_name: &str,
        role: &str,
        email_verified: bool,
    ) -> Result<User> {
        self.repo
            .sync_oidc_attributes(id, display_name, role, email_verified)
            .await
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve_role_with_policy(roles: &[&str]) -> String {
        let policy = ProvisioningPolicy::new(vec![], "user".to_owned());
        let strings: Vec<String> = roles.iter().map(|s| s.to_string()).collect();
        policy.resolve_role(&strings)
    }

    #[test]
    fn resolve_role_should_return_admin_for_exact_match() {
        assert_eq!(resolve_role_with_policy(&["admin"]), "admin");
        assert_eq!(resolve_role_with_policy(&["administrator"]), "admin");
        assert_eq!(resolve_role_with_policy(&["superuser"]), "admin");
    }

    #[test]
    fn resolve_role_should_return_manager_for_exact_match() {
        assert_eq!(resolve_role_with_policy(&["manager"]), "manager");
        assert_eq!(resolve_role_with_policy(&["supervisor"]), "manager");
    }

    #[test]
    fn resolve_role_should_return_user_for_unknown_role() {
        assert_eq!(resolve_role_with_policy(&["viewer"]), "user");
    }

    #[test]
    fn resolve_role_should_return_user_for_empty_roles() {
        assert_eq!(resolve_role_with_policy(&[]), "user");
    }

    #[test]
    fn resolve_role_should_be_case_insensitive() {
        assert_eq!(resolve_role_with_policy(&["Admin"]), "admin");
    }

    #[test]
    fn resolve_role_should_not_match_substrings() {
        assert_eq!(resolve_role_with_policy(&["superadministrator"]), "user");
        assert_eq!(resolve_role_with_policy(&["micro_manager"]), "user");
    }

    #[test]
    fn resolve_role_first_match_wins() {
        assert_eq!(resolve_role_with_policy(&["admin", "manager"]), "admin");
    }

    #[test]
    fn derive_provider_should_extract_hostname() {
        assert_eq!(
            derive_provider_from_issuer("https://auth.example.com"),
            "auth.example.com"
        );
        assert_eq!(
            derive_provider_from_issuer("http://localhost:8080"),
            "localhost:8080"
        );
        assert_eq!(
            derive_provider_from_issuer("https://idp.bank.com/oauth2"),
            "idp.bank.com"
        );
    }

    #[test]
    fn policy_should_allow_empty_domains() {
        let policy = ProvisioningPolicy::new(vec![], "user".to_owned());
        assert!(policy.check_email_domain("any@example.com").is_ok());
    }

    #[test]
    fn policy_should_reject_non_whitelisted_domains() {
        let policy = ProvisioningPolicy::new(vec!["allowed.com".to_owned()], "user".to_owned());
        assert!(policy.check_email_domain("user@blocked.com").is_err());
    }

    #[test]
    fn policy_should_allow_whitelisted_domains() {
        let policy = ProvisioningPolicy::new(vec!["allowed.com".to_owned()], "user".to_owned());
        assert!(policy.check_email_domain("user@allowed.com").is_ok());
    }
}
