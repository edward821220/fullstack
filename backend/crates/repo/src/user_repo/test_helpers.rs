use std::sync::Mutex;

use async_trait::async_trait;
use model::user::User;
use model::user_identity::UserIdentity;
use uuid::Uuid;

use super::{Error, Result, UserRepo};

/// An in-memory fake implementation of [`UserRepo`] for testing.
pub struct MockUserRepo {
    pub users: Mutex<Vec<User>>,
    pub identities: Mutex<Vec<UserIdentity>>,
}

impl MockUserRepo {
    pub fn new() -> Self {
        Self {
            users: Mutex::new(Vec::new()),
            identities: Mutex::new(Vec::new()),
        }
    }
}

impl Default for MockUserRepo {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl UserRepo for MockUserRepo {
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
        role: &str,
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
            role: role.to_owned(),
            email_verified,
            created_at: now,
            updated_at: now,
        };
        users.push(user.clone());
        Ok(user)
    }

    async fn update(&self, id: Uuid, display_name: Option<&str>) -> Result<User> {
        let mut users = self.users.lock().unwrap();
        let user = users
            .iter_mut()
            .find(|u| u.id == id)
            .ok_or(Error::UserNotFound { id })?;
        if let Some(n) = display_name {
            user.display_name = n.to_owned();
        }
        user.updated_at = time::OffsetDateTime::now_utc();
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
        role: &str,
        email_verified: bool,
    ) -> Result<User> {
        let mut users = self.users.lock().unwrap();
        let u = users
            .iter_mut()
            .find(|u| u.id == id)
            .ok_or(Error::UserNotFound { id })?;
        u.display_name = display_name.to_owned();
        u.role = role.to_owned();
        u.email_verified = email_verified;
        u.updated_at = time::OffsetDateTime::now_utc();
        Ok(u.clone())
    }

    async fn health_check(&self) -> Result<()> {
        Ok(())
    }
}
