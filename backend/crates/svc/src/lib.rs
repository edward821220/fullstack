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
            Some(u) => {
                tracing::info!(
                    email = %oidc_info.email,
                    sub = %oidc_info.sub,
                    issuer = %oidc_info.issuer,
                    user_id = %u.id,
                    "Linking existing user to OIDC identity and syncing attributes"
                );
                self.repo
                    .sync_oidc_attributes(u.id, &oidc_info.name, &role, oidc_info.email_verified)
                    .await?
            }
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
    use model::user_identity::UserIdentity;
    use std::sync::{Arc, Mutex};
    use time::OffsetDateTime;

    fn resolve_role_with_policy(roles: &[&str]) -> String {
        let policy = ProvisioningPolicy::new(vec![], "user".to_owned());
        let strings: Vec<String> = roles.iter().map(|s| s.to_string()).collect();
        policy.resolve_role(&strings)
    }

    fn test_user(
        id: Uuid,
        email: &str,
        display_name: &str,
        role: &str,
        email_verified: bool,
    ) -> User {
        let now = OffsetDateTime::now_utc();
        User {
            id,
            email: email.to_owned(),
            display_name: display_name.to_owned(),
            role: role.to_owned(),
            email_verified,
            created_at: now,
            updated_at: now,
        }
    }

    struct ProvisionTestLog {
        sync_odc_calls: Mutex<Vec<(Uuid, String, String, bool)>>,
        create_calls: Mutex<Vec<(String, String, String, bool)>>,
        create_identity_calls: Mutex<Vec<(Uuid, String, String, String)>>,
    }

    impl Default for ProvisionTestLog {
        fn default() -> Self {
            Self {
                sync_odc_calls: Mutex::new(Vec::new()),
                create_calls: Mutex::new(Vec::new()),
                create_identity_calls: Mutex::new(Vec::new()),
            }
        }
    }

    struct MockUserRepo {
        find_by_identity_result: Option<(User, UserIdentity)>,
        find_by_email_result: Option<User>,
        sync_result: User,
        create_result: User,
        log: Arc<ProvisionTestLog>,
    }

    #[async_trait]
    impl UserRepo for MockUserRepo {
        async fn find_by_id(&self, _id: Uuid) -> repo::Result<Option<User>> {
            unimplemented!()
        }
        async fn find_by_email(&self, _email: &str) -> repo::Result<Option<User>> {
            Ok(self.find_by_email_result.clone())
        }
        async fn create(
            &self,
            email: &str,
            display_name: &str,
            role: &str,
            email_verified: bool,
        ) -> repo::Result<User> {
            self.log.create_calls.lock().unwrap().push((
                email.to_owned(),
                display_name.to_owned(),
                role.to_owned(),
                email_verified,
            ));
            Ok(self.create_result.clone())
        }
        async fn update(&self, _id: Uuid, _display_name: Option<&str>) -> repo::Result<User> {
            unimplemented!()
        }
        async fn delete(&self, _id: Uuid) -> repo::Result<()> {
            unimplemented!()
        }
        async fn list(&self, _page: u64, _per_page: u64) -> repo::Result<(Vec<User>, u64)> {
            unimplemented!()
        }
        async fn find_by_identity(
            &self,
            _provider: &str,
            _issuer: &str,
            _external_sub: &str,
        ) -> repo::Result<Option<(User, UserIdentity)>> {
            Ok(self.find_by_identity_result.clone())
        }
        async fn find_identity(
            &self,
            _provider: &str,
            _issuer: &str,
            _external_sub: &str,
        ) -> repo::Result<Option<UserIdentity>> {
            unimplemented!()
        }
        async fn create_identity(
            &self,
            user_id: Uuid,
            provider: &str,
            issuer: &str,
            external_sub: &str,
        ) -> repo::Result<UserIdentity> {
            self.log.create_identity_calls.lock().unwrap().push((
                user_id,
                provider.to_owned(),
                issuer.to_owned(),
                external_sub.to_owned(),
            ));
            Ok(UserIdentity {
                id: Uuid::new_v4(),
                user_id,
                provider: provider.to_owned(),
                issuer: issuer.to_owned(),
                external_sub: external_sub.to_owned(),
                created_at: OffsetDateTime::now_utc(),
            })
        }
        async fn sync_oidc_attributes(
            &self,
            id: Uuid,
            display_name: &str,
            role: &str,
            email_verified: bool,
        ) -> repo::Result<User> {
            self.log.sync_odc_calls.lock().unwrap().push((
                id,
                display_name.to_owned(),
                role.to_owned(),
                email_verified,
            ));
            Ok(self.sync_result.clone())
        }
        async fn health_check(&self) -> repo::Result<()> {
            unimplemented!()
        }
    }

    fn test_osc_info() -> OidcUserInfo {
        OidcUserInfo {
            sub: "sub-123".to_owned(),
            issuer: "https://idp.example.com".to_owned(),
            email: "user@example.com".to_owned(),
            name: "New Name".to_owned(),
            email_verified: true,
            roles: vec!["admin".to_owned()],
        }
    }

    fn test_policy() -> ProvisioningPolicy {
        ProvisioningPolicy::new(vec![], "user".to_owned())
    }

    #[tokio::test]
    async fn provision_should_sync_and_link_when_user_exists_by_email_without_identity() {
        let existing_id = Uuid::new_v4();
        let log = Arc::new(ProvisionTestLog::default());

        let mock = MockUserRepo {
            find_by_identity_result: None,
            find_by_email_result: Some(test_user(
                existing_id,
                "user@example.com",
                "Old Name",
                "user",
                false,
            )),
            sync_result: test_user(existing_id, "user@example.com", "New Name", "admin", true),
            create_result: test_user(existing_id, "user@example.com", "New Name", "admin", true),
            log: Arc::clone(&log),
        };
        let svc = UserService::new(Box::new(mock));
        let policy = test_policy();
        let oidc_info = test_osc_info();

        let user = svc.provision_user(&oidc_info, &policy).await.unwrap();

        assert_eq!(user.display_name, "New Name");
        assert_eq!(user.role, "admin");
        assert!(user.email_verified);
        assert_eq!(log.sync_odc_calls.lock().unwrap().len(), 1);
        assert_eq!(log.create_calls.lock().unwrap().len(), 0);
        assert_eq!(log.create_identity_calls.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn provision_should_create_user_and_identity_when_user_does_not_exist() {
        let new_id = Uuid::new_v4();
        let log = Arc::new(ProvisionTestLog::default());

        let mock = MockUserRepo {
            find_by_identity_result: None,
            find_by_email_result: None,
            sync_result: test_user(new_id, "user@example.com", "New Name", "admin", true),
            create_result: test_user(new_id, "user@example.com", "New Name", "admin", true),
            log: Arc::clone(&log),
        };
        let svc = UserService::new(Box::new(mock));
        let policy = test_policy();
        let oidc_info = test_osc_info();

        let user = svc.provision_user(&oidc_info, &policy).await.unwrap();

        assert_eq!(user.display_name, "New Name");
        assert_eq!(user.role, "admin");
        assert_eq!(log.create_calls.lock().unwrap().len(), 1);
        assert_eq!(log.sync_odc_calls.lock().unwrap().len(), 0);
        assert_eq!(log.create_identity_calls.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn provision_should_sync_and_return_when_identity_already_exists() {
        let existing_id = Uuid::new_v4();
        let log = Arc::new(ProvisionTestLog::default());
        let now = OffsetDateTime::now_utc();

        let existing_identity = UserIdentity {
            id: Uuid::new_v4(),
            user_id: existing_id,
            provider: "idp.example.com".to_owned(),
            issuer: "https://idp.example.com".to_owned(),
            external_sub: "sub-123".to_owned(),
            created_at: now,
        };

        let mock = MockUserRepo {
            find_by_identity_result: Some((
                test_user(existing_id, "user@example.com", "Old Name", "user", false),
                existing_identity,
            )),
            find_by_email_result: None,
            sync_result: test_user(existing_id, "user@example.com", "New Name", "admin", true),
            create_result: test_user(existing_id, "user@example.com", "New Name", "admin", true),
            log: Arc::clone(&log),
        };
        let svc = UserService::new(Box::new(mock));
        let policy = test_policy();
        let oidc_info = test_osc_info();

        let user = svc.provision_user(&oidc_info, &policy).await.unwrap();

        assert_eq!(user.display_name, "New Name");
        assert_eq!(user.role, "admin");
        assert_eq!(log.sync_odc_calls.lock().unwrap().len(), 1);
        assert_eq!(log.create_identity_calls.lock().unwrap().len(), 0);
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
