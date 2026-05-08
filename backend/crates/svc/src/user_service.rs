use crate::policy::{OidcUserInfo, ProvisioningPolicy, derive_provider_from_issuer};
use crate::{Error, Result};
use async_trait::async_trait;
use model::user::User;
use repo::UserRepo;
use repo::user_repo::Transaction;
use tracing::instrument;
use uuid::Uuid;
pub struct UserService<R: UserRepo> {
    repo: R,
}
impl<R: UserRepo> UserService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }
}
#[async_trait]
pub trait UserServiceTrait<R: UserRepo>: Send + Sync {
    async fn get_user(&self, id: Uuid) -> Result<User>;
    async fn list_users(&self, page: u64, per_page: u64) -> Result<(Vec<User>, u64)>;
    async fn create_user(
        &self,
        email: &str,
        display_name: &str,
        role: model::role::Role,
        email_verified: bool,
    ) -> Result<User>;
    async fn update_user(
        &self,
        id: Uuid,
        display_name: Option<&str>,
        version: Option<i64>,
    ) -> Result<User>;
    async fn delete_user(&self, id: Uuid) -> Result<()>;
    async fn provision_user(
        &self,
        oidc_info: &OidcUserInfo,
        policy: &ProvisioningPolicy,
    ) -> Result<User>;
    async fn sync_oidc_attributes(
        &self,
        id: Uuid,
        display_name: &str,
        role: model::role::Role,
        email_verified: bool,
    ) -> Result<User>;
}
#[async_trait]
impl<R: UserRepo> UserServiceTrait<R> for UserService<R> {
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
    #[instrument(skip(self))]
    async fn create_user(
        &self,
        email: &str,
        display_name: &str,
        role: model::role::Role,
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
    async fn update_user(
        &self,
        id: Uuid,
        display_name: Option<&str>,
        version: Option<i64>,
    ) -> Result<User> {
        Ok(self.repo.update(id, display_name, version).await?)
    }
    #[instrument(skip(self), fields(user_id = %id))]
    async fn delete_user(&self, id: Uuid) -> Result<()> {
        Ok(self.repo.delete(id).await?)
    }
    #[instrument(skip(self, oidc_info, policy), fields(issuer = %oidc_info.issuer))]
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
                .sync_oidc_attributes(user.id, &oidc_info.name, role, oidc_info.email_verified)
                .await
                .map_err(Into::into);
        }
        let mut tx = self.repo.begin_transaction().await?;
        let role = policy.resolve_role(&oidc_info.roles);
        let existing_user = self
            .repo
            .find_by_email_in_tx(&mut tx, &oidc_info.email)
            .await?;
        let user = match existing_user {
            Some(u) => {
                tracing::info!(
                    issuer = %oidc_info.issuer,
                    user_id = %u.id,
                    "Linking existing user to OIDC identity and syncing attributes"
                );
                self.repo
                    .sync_oidc_attributes_in_tx(
                        &mut tx,
                        u.id,
                        &oidc_info.name,
                        role,
                        oidc_info.email_verified,
                    )
                    .await?
            }
            None => {
                tracing::info!(
                    issuer = %oidc_info.issuer,
                    "Creating new user via JIT provisioning"
                );
                self.repo
                    .create_in_tx(
                        &mut tx,
                        &oidc_info.email,
                        &oidc_info.name,
                        role,
                        oidc_info.email_verified,
                    )
                    .await?
            }
        };
        self.repo
            .create_identity_in_tx(
                &mut tx,
                user.id,
                provider,
                &oidc_info.issuer,
                &oidc_info.sub,
            )
            .await?;
        tx.commit().await?;
        Ok(user)
    }
    async fn sync_oidc_attributes(
        &self,
        id: Uuid,
        display_name: &str,
        role: model::role::Role,
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
    use model::role::Role;
    use model::user_identity::UserIdentity;
    use time::OffsetDateTime;

    fn resolve_role_with_policy(roles: &[&str]) -> Role {
        let policy = ProvisioningPolicy::new(vec![], "user".to_owned());
        let strings: Vec<String> = roles.iter().map(|s| s.to_string()).collect();
        policy.resolve_role(&strings)
    }

    fn test_user(
        id: Uuid,
        email: &str,
        display_name: &str,
        role: Role,
        email_verified: bool,
    ) -> User {
        let now = OffsetDateTime::now_utc();
        User {
            id,
            email: email.to_owned(),
            display_name: display_name.to_owned(),
            role,
            email_verified,
            created_at: now,
            updated_at: now,
            version: 1,
        }
    }

    fn test_oidc_info() -> OidcUserInfo {
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
        let repo = repo::MockUserRepo::new();
        repo.users.lock().unwrap().push(test_user(
            existing_id,
            "user@example.com",
            "Old Name",
            Role::User,
            false,
        ));

        let svc = UserService::new(repo);
        let policy = test_policy();
        let oidc_info = test_oidc_info();

        let user = svc.provision_user(&oidc_info, &policy).await.unwrap();

        assert_eq!(user.display_name, "New Name");
        assert_eq!(user.role, Role::Admin);
        assert!(user.email_verified);

        let identities = svc
            .repo
            .find_identity("idp.example.com", "https://idp.example.com", "sub-123")
            .await
            .unwrap();
        assert!(identities.is_some());
    }

    #[tokio::test]
    async fn provision_should_create_user_and_identity_when_user_does_not_exist() {
        let repo = repo::MockUserRepo::new();
        let svc = UserService::new(repo);
        let policy = test_policy();
        let oidc_info = test_oidc_info();

        let user = svc.provision_user(&oidc_info, &policy).await.unwrap();

        assert_eq!(user.display_name, "New Name");
        assert_eq!(user.role, Role::Admin);

        let users = svc.repo.list(1, 10).await.unwrap();
        assert_eq!(users.0.len(), 1);

        let identities = svc
            .repo
            .find_identity("idp.example.com", "https://idp.example.com", "sub-123")
            .await
            .unwrap();
        assert!(identities.is_some());
    }

    #[tokio::test]
    async fn provision_should_sync_and_return_when_identity_already_exists() {
        let existing_id = Uuid::new_v4();
        let repo = repo::MockUserRepo::new();
        repo.users.lock().unwrap().push(test_user(
            existing_id,
            "user@example.com",
            "Old Name",
            Role::User,
            false,
        ));
        repo.identities.lock().unwrap().push(UserIdentity {
            id: Uuid::new_v4(),
            user_id: existing_id,
            provider: "idp.example.com".to_owned(),
            issuer: "https://idp.example.com".to_owned(),
            external_sub: "sub-123".to_owned(),
            created_at: OffsetDateTime::now_utc(),
        });

        let svc = UserService::new(repo);
        let policy = test_policy();
        let oidc_info = test_oidc_info();

        let user = svc.provision_user(&oidc_info, &policy).await.unwrap();

        assert_eq!(user.display_name, "New Name");
        assert_eq!(user.role, Role::Admin);

        let identities = svc
            .repo
            .find_identity("idp.example.com", "https://idp.example.com", "sub-123")
            .await
            .unwrap();
        assert!(identities.is_some());
    }

    #[tokio::test]
    async fn provision_should_rollback_when_identity_creation_fails() {
        let repo = repo::MockUserRepo::new();
        let svc = UserService::new(repo);
        let policy = test_policy();
        let oidc_info = test_oidc_info();

        let user = svc.provision_user(&oidc_info, &policy).await.unwrap();
        assert_eq!(user.email, "user@example.com");

        let users = svc.repo.list(1, 10).await.unwrap();
        assert_eq!(users.0.len(), 1);
    }

    #[test]
    fn resolve_role_should_return_admin_for_exact_match() {
        assert_eq!(resolve_role_with_policy(&["admin"]), Role::Admin);
        assert_eq!(resolve_role_with_policy(&["administrator"]), Role::Admin);
        assert_eq!(resolve_role_with_policy(&["superuser"]), Role::Admin);
    }

    #[test]
    fn resolve_role_should_return_manager_for_exact_match() {
        assert_eq!(resolve_role_with_policy(&["manager"]), Role::Manager);
        assert_eq!(resolve_role_with_policy(&["supervisor"]), Role::Manager);
    }

    #[test]
    fn resolve_role_should_return_user_for_unknown_role() {
        assert_eq!(resolve_role_with_policy(&["viewer"]), Role::User);
    }

    #[test]
    fn resolve_role_should_return_user_for_empty_roles() {
        assert_eq!(resolve_role_with_policy(&[]), Role::User);
    }

    #[test]
    fn resolve_role_should_be_case_insensitive() {
        assert_eq!(resolve_role_with_policy(&["Admin"]), Role::Admin);
    }

    #[test]
    fn resolve_role_should_not_match_substrings() {
        assert_eq!(
            resolve_role_with_policy(&["superadministrator"]),
            Role::User
        );
        assert_eq!(resolve_role_with_policy(&["micro_manager"]), Role::User);
    }

    #[test]
    fn resolve_role_first_match_wins() {
        assert_eq!(resolve_role_with_policy(&["admin", "manager"]), Role::Admin);
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
