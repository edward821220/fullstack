use async_trait::async_trait;
use model::role::Role;
use model::user::User;
use model::user_identity::UserIdentity;
use uuid::Uuid;

use crate::error::Result;

/// A database transaction that can be committed or rolled back.
#[async_trait]
pub trait Transaction: Send + Sync {
    async fn commit(self) -> Result<()>;
    async fn rollback(self) -> Result<()>;
}

/// The persistence seam for `User` and `UserIdentity`.
///
/// All adapters (Postgres, MSSQL, Mock) implement this trait.
/// Because the trait has an associated type (`Tx`), it is **not object-safe**.
/// Use [`AnyUserRepo`](crate::user_repo::AnyUserRepo) when you need to erase the concrete type at API boundaries.
#[async_trait]
pub trait UserRepo: Send + Sync + Clone {
    type Tx: Transaction;

    async fn begin_transaction(&self) -> Result<Self::Tx>;

    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>>;
    async fn find_by_email(&self, email: &str) -> Result<Option<User>>;
    async fn create(
        &self,
        email: &str,
        display_name: &str,
        role: Role,
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
        role: Role,
        email_verified: bool,
    ) -> Result<User>;

    /// Find a user by email **inside a transaction**, acquiring a row lock.
    async fn find_by_email_in_tx(&self, tx: &mut Self::Tx, email: &str) -> Result<Option<User>>;

    /// Create a user **inside a transaction**.
    async fn create_in_tx(
        &self,
        tx: &mut Self::Tx,
        email: &str,
        display_name: &str,
        role: Role,
        email_verified: bool,
    ) -> Result<User>;

    /// Sync OIDC attributes **inside a transaction**.
    async fn sync_oidc_attributes_in_tx(
        &self,
        tx: &mut Self::Tx,
        id: Uuid,
        display_name: &str,
        role: Role,
        email_verified: bool,
    ) -> Result<User>;

    /// Create an identity **inside a transaction**.
    async fn create_identity_in_tx(
        &self,
        tx: &mut Self::Tx,
        user_id: Uuid,
        provider: &str,
        issuer: &str,
        external_sub: &str,
    ) -> Result<UserIdentity>;
}
