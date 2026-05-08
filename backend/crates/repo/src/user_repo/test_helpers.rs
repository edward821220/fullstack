use super::{Transaction, UserRepo};
use crate::{Error, Result};
use async_trait::async_trait;
use model::user::User;
use model::user_identity::UserIdentity;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// An in-memory fake implementation of [`UserRepo`] for testing.
pub struct MockUserRepo {
    pub users: Arc<Mutex<Vec<User>>>,
    pub identities: Arc<Mutex<Vec<UserIdentity>>>,
}

impl MockUserRepo {
    pub fn new() -> Self {
        Self {
            users: Arc::new(Mutex::new(Vec::new())),
            identities: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl Default for MockUserRepo {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for MockUserRepo {
    fn clone(&self) -> Self {
        Self {
            users: Arc::clone(&self.users),
            identities: Arc::clone(&self.identities),
        }
    }
}

pub struct MockTransaction {
    original_users: Arc<Mutex<Vec<User>>>,
    original_identities: Arc<Mutex<Vec<UserIdentity>>>,
    users: Vec<User>,
    identities: Vec<UserIdentity>,
}

#[async_trait]
impl Transaction for MockTransaction {
    async fn commit(mut self) -> Result<()> {
        let mut orig_users = self.original_users.lock().unwrap();
        let mut orig_identities = self.original_identities.lock().unwrap();
        *orig_users = self.users;
        *orig_identities = self.identities;
        Ok(())
    }

    async fn rollback(self) -> Result<()> {
        // Discard staged changes, original data remains unchanged
        Ok(())
    }
}

#[async_trait]
impl UserRepo for MockUserRepo {
    type Tx = MockTransaction;

    async fn begin_transaction(&self) -> Result<Self::Tx> {
        let users = self.users.lock().unwrap().clone();
        let identities = self.identities.lock().unwrap().clone();
        Ok(MockTransaction {
            original_users: Arc::clone(&self.users),
            original_identities: Arc::clone(&self.identities),
            users,
            identities,
        })
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>> {
        Ok(self
            .users
            .lock()
            .unwrap()
            .iter()
            .find(|u| u.id == id)
            .cloned())
    }

    async fn find_by_email(&self, email: &str) -> Result<Option<User>> {
        Ok(self
            .users
            .lock()
            .unwrap()
            .iter()
            .find(|u| u.email == email)
            .cloned())
    }

    async fn create(
        &self,
        email: &str,
        display_name: &str,
        role: model::role::Role,
        email_verified: bool,
    ) -> Result<User> {
        let mut users = self.users.lock().unwrap();
        if users.iter().any(|u| u.email == email) {
            return Err(Error::UserAlreadyExists {
                email: email.to_owned(),
            });
        }
        let now = time::OffsetDateTime::now_utc();
        let user = User {
            id: Uuid::new_v4(),
            email: email.to_owned(),
            display_name: display_name.to_owned(),
            role,
            email_verified,
            created_at: now,
            updated_at: now,
            version: 1,
        };
        users.push(user.clone());
        Ok(user)
    }

    async fn update(
        &self,
        id: Uuid,
        display_name: Option<&str>,
        _version: Option<i64>,
    ) -> Result<User> {
        let mut users = self.users.lock().unwrap();
        let user = users
            .iter_mut()
            .find(|u| u.id == id)
            .ok_or(Error::UserNotFound { id })?;
        if let Some(n) = display_name {
            user.display_name = n.to_owned();
        }
        user.updated_at = time::OffsetDateTime::now_utc();
        user.version += 1;
        Ok(user.clone())
    }

    async fn delete(&self, id: Uuid) -> Result<()> {
        let mut users = self.users.lock().unwrap();
        let pos = users
            .iter()
            .position(|u| u.id == id)
            .ok_or(Error::UserNotFound { id })?;
        users.remove(pos);
        Ok(())
    }

    async fn list(&self, page: u64, per_page: u64) -> Result<(Vec<User>, u64)> {
        let users = self.users.lock().unwrap();
        let total = users.len() as u64;
        let start = ((page - 1) * per_page) as usize;
        let end = (start + per_page as usize).min(users.len());
        Ok((users[start..end].to_vec(), total))
    }

    async fn find_by_identity(
        &self,
        provider: &str,
        issuer: &str,
        external_sub: &str,
    ) -> Result<Option<(User, UserIdentity)>> {
        let id = self
            .identities
            .lock()
            .unwrap()
            .iter()
            .find(|i| {
                i.provider == provider && i.issuer == issuer && i.external_sub == external_sub
            })
            .cloned();
        match id {
            Some(i) => {
                let u = self
                    .users
                    .lock()
                    .unwrap()
                    .iter()
                    .find(|u| u.id == i.user_id)
                    .cloned()
                    .ok_or(Error::UserNotFound { id: i.user_id })?;
                Ok(Some((u, i)))
            }
            None => Ok(None),
        }
    }

    async fn find_identity(
        &self,
        provider: &str,
        issuer: &str,
        external_sub: &str,
    ) -> Result<Option<UserIdentity>> {
        Ok(self
            .identities
            .lock()
            .unwrap()
            .iter()
            .find(|i| {
                i.provider == provider && i.issuer == issuer && i.external_sub == external_sub
            })
            .cloned())
    }

    async fn create_identity(
        &self,
        user_id: Uuid,
        provider: &str,
        issuer: &str,
        external_sub: &str,
    ) -> Result<UserIdentity> {
        let mut ids = self.identities.lock().unwrap();
        let now = time::OffsetDateTime::now_utc();
        let id = UserIdentity {
            id: Uuid::new_v4(),
            user_id,
            provider: provider.to_owned(),
            issuer: issuer.to_owned(),
            external_sub: external_sub.to_owned(),
            created_at: now,
        };
        ids.push(id.clone());
        Ok(id)
    }

    async fn sync_oidc_attributes(
        &self,
        id: Uuid,
        display_name: &str,
        role: model::role::Role,
        email_verified: bool,
    ) -> Result<User> {
        let mut users = self.users.lock().unwrap();
        let u = users
            .iter_mut()
            .find(|u| u.id == id)
            .ok_or(Error::UserNotFound { id })?;
        u.display_name = display_name.to_owned();
        u.role = role;
        u.email_verified = email_verified;
        u.updated_at = time::OffsetDateTime::now_utc();
        u.version += 1;
        Ok(u.clone())
    }

    async fn find_by_email_in_tx(&self, tx: &mut Self::Tx, email: &str) -> Result<Option<User>> {
        Ok(tx.users.iter().find(|u| u.email == email).cloned())
    }

    async fn create_in_tx(
        &self,
        tx: &mut Self::Tx,
        email: &str,
        display_name: &str,
        role: model::role::Role,
        email_verified: bool,
    ) -> Result<User> {
        if tx.users.iter().any(|u| u.email == email) {
            return Err(Error::UserAlreadyExists {
                email: email.to_owned(),
            });
        }
        let now = time::OffsetDateTime::now_utc();
        let user = User {
            id: Uuid::new_v4(),
            email: email.to_owned(),
            display_name: display_name.to_owned(),
            role,
            email_verified,
            created_at: now,
            updated_at: now,
            version: 1,
        };
        tx.users.push(user.clone());
        Ok(user)
    }

    async fn sync_oidc_attributes_in_tx(
        &self,
        tx: &mut Self::Tx,
        id: Uuid,
        display_name: &str,
        role: model::role::Role,
        email_verified: bool,
    ) -> Result<User> {
        let u = tx
            .users
            .iter_mut()
            .find(|u| u.id == id)
            .ok_or(Error::UserNotFound { id })?;
        u.display_name = display_name.to_owned();
        u.role = role;
        u.email_verified = email_verified;
        u.updated_at = time::OffsetDateTime::now_utc();
        u.version += 1;
        Ok(u.clone())
    }

    async fn create_identity_in_tx(
        &self,
        tx: &mut Self::Tx,
        user_id: Uuid,
        provider: &str,
        issuer: &str,
        external_sub: &str,
    ) -> Result<UserIdentity> {
        let now = time::OffsetDateTime::now_utc();
        let id = UserIdentity {
            id: Uuid::new_v4(),
            user_id,
            provider: provider.to_owned(),
            issuer: issuer.to_owned(),
            external_sub: external_sub.to_owned(),
            created_at: now,
        };
        tx.identities.push(id.clone());
        Ok(id)
    }
}
