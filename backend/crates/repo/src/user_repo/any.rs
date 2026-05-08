use async_trait::async_trait;
use model::role::Role;
use model::user::User;
use model::user_identity::UserIdentity;
use uuid::Uuid;

use super::{
    Transaction, UserRepo,
    mssql::{MssqlTransaction, MssqlUserRepo},
    postgres::{PgTransaction, PostgresUserRepo},
};
use crate::error::{Error, Result};

#[cfg(feature = "test-helpers")]
use super::test_helpers;

/// A type-erased [`UserRepo`] that can hold either a Postgres or MSSQL adapter.
///
/// This enum lets `AppState` and server bootstrap code stay concrete (no generics)
/// while still benefiting from static dispatch inside the `svc` crate.
#[derive(Clone)]
pub enum AnyUserRepo {
    Postgres(PostgresUserRepo),
    Mssql(MssqlUserRepo),
    #[cfg(feature = "test-helpers")]
    Mock(test_helpers::MockUserRepo),
}

/// A type-erased [`Transaction`] that matches the variant held by [`AnyUserRepo`].
pub enum AnyTransaction {
    Postgres(PgTransaction),
    Mssql(MssqlTransaction),
    #[cfg(feature = "test-helpers")]
    Mock(test_helpers::MockTransaction),
}

#[async_trait]
impl Transaction for AnyTransaction {
    async fn commit(self) -> Result<()> {
        match self {
            AnyTransaction::Postgres(tx) => tx.commit().await,
            AnyTransaction::Mssql(tx) => tx.commit().await,
            #[cfg(feature = "test-helpers")]
            AnyTransaction::Mock(tx) => tx.commit().await,
        }
    }

    async fn rollback(self) -> Result<()> {
        match self {
            AnyTransaction::Postgres(tx) => tx.rollback().await,
            AnyTransaction::Mssql(tx) => tx.rollback().await,
            #[cfg(feature = "test-helpers")]
            AnyTransaction::Mock(tx) => tx.rollback().await,
        }
    }
}

#[async_trait]
impl UserRepo for AnyUserRepo {
    type Tx = AnyTransaction;

    async fn begin_transaction(&self) -> Result<Self::Tx> {
        match self {
            AnyUserRepo::Postgres(repo) => {
                repo.begin_transaction().await.map(AnyTransaction::Postgres)
            }
            AnyUserRepo::Mssql(repo) => repo.begin_transaction().await.map(AnyTransaction::Mssql),
            #[cfg(feature = "test-helpers")]
            AnyUserRepo::Mock(repo) => repo.begin_transaction().await.map(AnyTransaction::Mock),
        }
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>> {
        match self {
            AnyUserRepo::Postgres(repo) => repo.find_by_id(id).await,
            AnyUserRepo::Mssql(repo) => repo.find_by_id(id).await,
            #[cfg(feature = "test-helpers")]
            AnyUserRepo::Mock(repo) => repo.find_by_id(id).await,
        }
    }

    async fn find_by_email(&self, email: &str) -> Result<Option<User>> {
        match self {
            AnyUserRepo::Postgres(repo) => repo.find_by_email(email).await,
            AnyUserRepo::Mssql(repo) => repo.find_by_email(email).await,
            #[cfg(feature = "test-helpers")]
            AnyUserRepo::Mock(repo) => repo.find_by_email(email).await,
        }
    }

    async fn create(
        &self,
        email: &str,
        display_name: &str,
        role: Role,
        email_verified: bool,
    ) -> Result<User> {
        match self {
            AnyUserRepo::Postgres(repo) => {
                repo.create(email, display_name, role, email_verified).await
            }
            AnyUserRepo::Mssql(repo) => {
                repo.create(email, display_name, role, email_verified).await
            }
            #[cfg(feature = "test-helpers")]
            AnyUserRepo::Mock(repo) => repo.create(email, display_name, role, email_verified).await,
        }
    }

    async fn update(
        &self,
        id: Uuid,
        display_name: Option<&str>,
        version: Option<i64>,
    ) -> Result<User> {
        match self {
            AnyUserRepo::Postgres(repo) => repo.update(id, display_name, version).await,
            AnyUserRepo::Mssql(repo) => repo.update(id, display_name, version).await,
            #[cfg(feature = "test-helpers")]
            AnyUserRepo::Mock(repo) => repo.update(id, display_name, version).await,
        }
    }

    async fn delete(&self, id: Uuid) -> Result<()> {
        match self {
            AnyUserRepo::Postgres(repo) => repo.delete(id).await,
            AnyUserRepo::Mssql(repo) => repo.delete(id).await,
            #[cfg(feature = "test-helpers")]
            AnyUserRepo::Mock(repo) => repo.delete(id).await,
        }
    }

    async fn list(&self, page: u64, per_page: u64) -> Result<(Vec<User>, u64)> {
        match self {
            AnyUserRepo::Postgres(repo) => repo.list(page, per_page).await,
            AnyUserRepo::Mssql(repo) => repo.list(page, per_page).await,
            #[cfg(feature = "test-helpers")]
            AnyUserRepo::Mock(repo) => repo.list(page, per_page).await,
        }
    }

    async fn find_by_identity(
        &self,
        provider: &str,
        issuer: &str,
        external_sub: &str,
    ) -> Result<Option<(User, UserIdentity)>> {
        match self {
            AnyUserRepo::Postgres(repo) => {
                repo.find_by_identity(provider, issuer, external_sub).await
            }
            AnyUserRepo::Mssql(repo) => repo.find_by_identity(provider, issuer, external_sub).await,
            #[cfg(feature = "test-helpers")]
            AnyUserRepo::Mock(repo) => repo.find_by_identity(provider, issuer, external_sub).await,
        }
    }

    async fn find_identity(
        &self,
        provider: &str,
        issuer: &str,
        external_sub: &str,
    ) -> Result<Option<UserIdentity>> {
        match self {
            AnyUserRepo::Postgres(repo) => repo.find_identity(provider, issuer, external_sub).await,
            AnyUserRepo::Mssql(repo) => repo.find_identity(provider, issuer, external_sub).await,
            #[cfg(feature = "test-helpers")]
            AnyUserRepo::Mock(repo) => repo.find_identity(provider, issuer, external_sub).await,
        }
    }

    async fn create_identity(
        &self,
        user_id: Uuid,
        provider: &str,
        issuer: &str,
        external_sub: &str,
    ) -> Result<UserIdentity> {
        match self {
            AnyUserRepo::Postgres(repo) => {
                repo.create_identity(user_id, provider, issuer, external_sub)
                    .await
            }
            AnyUserRepo::Mssql(repo) => {
                repo.create_identity(user_id, provider, issuer, external_sub)
                    .await
            }
            #[cfg(feature = "test-helpers")]
            AnyUserRepo::Mock(repo) => {
                repo.create_identity(user_id, provider, issuer, external_sub)
                    .await
            }
        }
    }

    async fn sync_oidc_attributes(
        &self,
        id: Uuid,
        display_name: &str,
        role: Role,
        email_verified: bool,
    ) -> Result<User> {
        match self {
            AnyUserRepo::Postgres(repo) => {
                repo.sync_oidc_attributes(id, display_name, role, email_verified)
                    .await
            }
            AnyUserRepo::Mssql(repo) => {
                repo.sync_oidc_attributes(id, display_name, role, email_verified)
                    .await
            }
            #[cfg(feature = "test-helpers")]
            AnyUserRepo::Mock(repo) => {
                repo.sync_oidc_attributes(id, display_name, role, email_verified)
                    .await
            }
        }
    }

    // --- transaction helpers ---

    async fn find_by_email_in_tx(&self, tx: &mut Self::Tx, email: &str) -> Result<Option<User>> {
        match (self, tx) {
            (AnyUserRepo::Postgres(repo), AnyTransaction::Postgres(tx)) => {
                repo.find_by_email_in_tx(tx, email).await
            }
            (AnyUserRepo::Mssql(repo), AnyTransaction::Mssql(tx)) => {
                repo.find_by_email_in_tx(tx, email).await
            }
            #[cfg(feature = "test-helpers")]
            (AnyUserRepo::Mock(repo), AnyTransaction::Mock(tx)) => {
                repo.find_by_email_in_tx(tx, email).await
            }
            _ => Err(Error::Transaction {
                message: "Repo and transaction type mismatch".to_owned(),
            }),
        }
    }

    async fn create_in_tx(
        &self,
        tx: &mut Self::Tx,
        email: &str,
        display_name: &str,
        role: Role,
        email_verified: bool,
    ) -> Result<User> {
        match (self, tx) {
            (AnyUserRepo::Postgres(repo), AnyTransaction::Postgres(tx)) => {
                repo.create_in_tx(tx, email, display_name, role, email_verified)
                    .await
            }
            (AnyUserRepo::Mssql(repo), AnyTransaction::Mssql(tx)) => {
                repo.create_in_tx(tx, email, display_name, role, email_verified)
                    .await
            }
            #[cfg(feature = "test-helpers")]
            (AnyUserRepo::Mock(repo), AnyTransaction::Mock(tx)) => {
                repo.create_in_tx(tx, email, display_name, role, email_verified)
                    .await
            }
            _ => Err(Error::Transaction {
                message: "Repo and transaction type mismatch".to_owned(),
            }),
        }
    }

    async fn sync_oidc_attributes_in_tx(
        &self,
        tx: &mut Self::Tx,
        id: Uuid,
        display_name: &str,
        role: Role,
        email_verified: bool,
    ) -> Result<User> {
        match (self, tx) {
            (AnyUserRepo::Postgres(repo), AnyTransaction::Postgres(tx)) => {
                repo.sync_oidc_attributes_in_tx(tx, id, display_name, role, email_verified)
                    .await
            }
            (AnyUserRepo::Mssql(repo), AnyTransaction::Mssql(tx)) => {
                repo.sync_oidc_attributes_in_tx(tx, id, display_name, role, email_verified)
                    .await
            }
            #[cfg(feature = "test-helpers")]
            (AnyUserRepo::Mock(repo), AnyTransaction::Mock(tx)) => {
                repo.sync_oidc_attributes_in_tx(tx, id, display_name, role, email_verified)
                    .await
            }
            _ => Err(Error::Transaction {
                message: "Repo and transaction type mismatch".to_owned(),
            }),
        }
    }

    async fn create_identity_in_tx(
        &self,
        tx: &mut Self::Tx,
        user_id: Uuid,
        provider: &str,
        issuer: &str,
        external_sub: &str,
    ) -> Result<UserIdentity> {
        match (self, tx) {
            (AnyUserRepo::Postgres(repo), AnyTransaction::Postgres(tx)) => {
                repo.create_identity_in_tx(tx, user_id, provider, issuer, external_sub)
                    .await
            }
            (AnyUserRepo::Mssql(repo), AnyTransaction::Mssql(tx)) => {
                repo.create_identity_in_tx(tx, user_id, provider, issuer, external_sub)
                    .await
            }
            #[cfg(feature = "test-helpers")]
            (AnyUserRepo::Mock(repo), AnyTransaction::Mock(tx)) => {
                repo.create_identity_in_tx(tx, user_id, provider, issuer, external_sub)
                    .await
            }
            _ => Err(Error::Transaction {
                message: "Repo and transaction type mismatch".to_owned(),
            }),
        }
    }
}
