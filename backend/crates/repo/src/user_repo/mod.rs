pub mod mssql;
pub mod postgres;

#[cfg(feature = "test-helpers")]
pub mod test_helpers;

use async_trait::async_trait;
use model::user::User;
use model::user_identity::UserIdentity;
use uuid::Uuid;

pub use crate::error::{Error, Result};
pub use mssql::MssqlUserRepo;
pub use postgres::PostgresUserRepo;

#[cfg(feature = "test-helpers")]
pub use test_helpers::MockUserRepo;

#[async_trait]
pub trait UserRepo: Send + Sync {
    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>>;
    async fn find_by_email(&self, email: &str) -> Result<Option<User>>;
    async fn create(
        &self,
        email: &str,
        display_name: &str,
        role: &str,
        email_verified: bool,
    ) -> Result<User>;
    async fn update(&self, id: Uuid, display_name: Option<&str>) -> Result<User>;
    async fn delete(&self, id: Uuid) -> Result<()>;
    async fn list(&self, page: u64, per_page: u64) -> Result<(Vec<User>, u64)>;

    async fn find_by_identity(
        &self,
        provider: &str,
        issuer: &str,
        external_sub: &str,
    ) -> Result<Option<(User, UserIdentity)>>;
    async fn find_identity(
        &self,
        provider: &str,
        issuer: &str,
        external_sub: &str,
    ) -> Result<Option<UserIdentity>>;
    async fn create_identity(
        &self,
        user_id: Uuid,
        provider: &str,
        issuer: &str,
        external_sub: &str,
    ) -> Result<UserIdentity>;

    async fn sync_oidc_attributes(
        &self,
        id: Uuid,
        display_name: &str,
        role: &str,
        email_verified: bool,
    ) -> Result<User>;

    async fn health_check(&self) -> Result<()>;
}
