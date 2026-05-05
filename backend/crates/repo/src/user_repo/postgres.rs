use async_trait::async_trait;
use model::user::User;
use model::user_identity::UserIdentity;
use sqlx::Row;
use uuid::Uuid;

use super::{Transaction, UserRepo};
use crate::{Error, Result};

pub struct PostgresUserRepo {
    pool: sqlx::PgPool,
}

impl PostgresUserRepo {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

impl Clone for PostgresUserRepo {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
        }
    }
}

pub struct PgTransaction {
    inner: sqlx::Transaction<'static, sqlx::Postgres>,
}

#[async_trait]
impl Transaction for PgTransaction {
    async fn commit(mut self) -> Result<()> {
        self.inner.commit().await.map_err(|e| Error::Database {
            message: e.to_string(),
        })
    }

    async fn rollback(mut self) -> Result<()> {
        self.inner.rollback().await.map_err(|e| Error::Database {
            message: e.to_string(),
        })
    }
}

fn to_user_pg(row: sqlx::postgres::PgRow) -> User {
    User {
        id: row.get("id"),
        email: row.get("email"),
        display_name: row.get("display_name"),
        role: row.get("role"),
        email_verified: row.get("email_verified"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn to_identity_pg(row: sqlx::postgres::PgRow) -> UserIdentity {
    UserIdentity {
        id: row.get("id"),
        user_id: row.get("user_id"),
        provider: row.get("provider"),
        issuer: row.get("issuer"),
        external_sub: row.get("external_sub"),
        created_at: row.get("created_at"),
    }
}

#[async_trait]
impl UserRepo for PostgresUserRepo {
    type Tx = PgTransaction;

    async fn begin_transaction(&self) -> Result<Self::Tx> {
        let tx = self.pool.begin().await.map_err(|e| Error::Database {
            message: e.to_string(),
        })?;
        Ok(PgTransaction { inner: tx })
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>> {
        sqlx::query(
            "SELECT id, email, display_name, role, email_verified, created_at, updated_at FROM users WHERE id = $1",
        )
        .bind(id)
        .map(to_user_pg)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database { message: e.to_string() })
    }

    async fn find_by_email(&self, email: &str) -> Result<Option<User>> {
        sqlx::query(
            "SELECT id, email, display_name, role, email_verified, created_at, updated_at FROM users WHERE email = $1",
        )
        .bind(email)
        .map(to_user_pg)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database { message: e.to_string() })
    }

    async fn create(
        &self,
        email: &str,
        display_name: &str,
        role: &str,
        email_verified: bool,
    ) -> Result<User> {
        if self.find_by_email(email).await?.is_some() {
            return Err(Error::UserAlreadyExists {
                email: email.to_owned(),
            });
        }

        let now = time::OffsetDateTime::now_utc();
        let id = Uuid::new_v4();

        sqlx::query(
            "INSERT INTO users (id, email, display_name, role, email_verified, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(id)
        .bind(email)
        .bind(display_name)
        .bind(role)
        .bind(email_verified)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database { message: e.to_string() })?;

        let user = self
            .find_by_id(id)
            .await?
            .ok_or(Error::UserNotFound { id })?;
        Ok(user)
    }

    async fn update(&self, id: Uuid, display_name: Option<&str>) -> Result<User> {
        let user = self
            .find_by_id(id)
            .await?
            .ok_or(Error::UserNotFound { id })?;

        let new_name = display_name.unwrap_or(&user.display_name);

        sqlx::query("UPDATE users SET display_name = $1, updated_at = $2 WHERE id = $3")
            .bind(new_name)
            .bind(time::OffsetDateTime::now_utc())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| Error::Database {
                message: e.to_string(),
            })?;

        self.find_by_id(id).await?.ok_or(Error::UserNotFound { id })
    }

    async fn delete(&self, id: Uuid) -> Result<()> {
        let result = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| Error::Database {
                message: e.to_string(),
            })?;

        if result.rows_affected() == 0 {
            return Err(Error::UserNotFound { id });
        }
        Ok(())
    }

    async fn list(&self, page: u64, per_page: u64) -> Result<(Vec<User>, u64)> {
        let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| Error::Database {
                message: e.to_string(),
            })?;

        let offset = (page - 1) * per_page;

        let users: Vec<User> = sqlx::query(
            "SELECT id, email, display_name, role, email_verified, created_at, updated_at
             FROM users ORDER BY created_at DESC LIMIT $1 OFFSET $2",
        )
        .bind(per_page as i64)
        .bind(offset as i64)
        .map(to_user_pg)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database {
            message: e.to_string(),
        })?;

        Ok((users, total.0 as u64))
    }

    async fn find_by_identity(
        &self,
        provider: &str,
        issuer: &str,
        external_sub: &str,
    ) -> Result<Option<(User, UserIdentity)>> {
        let identity = self.find_identity(provider, issuer, external_sub).await?;
        match identity {
            Some(id) => {
                let user = self
                    .find_by_id(id.user_id)
                    .await?
                    .ok_or(Error::UserNotFound { id: id.user_id })?;
                Ok(Some((user, id)))
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
        sqlx::query(
            "SELECT id, user_id, provider, issuer, external_sub, created_at
             FROM user_identities WHERE provider = $1 AND issuer = $2 AND external_sub = $3",
        )
        .bind(provider)
        .bind(issuer)
        .bind(external_sub)
        .map(to_identity_pg)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database {
            message: e.to_string(),
        })
    }

    async fn create_identity(
        &self,
        user_id: Uuid,
        provider: &str,
        issuer: &str,
        external_sub: &str,
    ) -> Result<UserIdentity> {
        let id = Uuid::new_v4();
        let now = time::OffsetDateTime::now_utc();

        sqlx::query(
            "INSERT INTO user_identities (id, user_id, provider, issuer, external_sub, created_at)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(id)
        .bind(user_id)
        .bind(provider)
        .bind(issuer)
        .bind(external_sub)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database {
            message: e.to_string(),
        })?;

        Ok(UserIdentity {
            id,
            user_id,
            provider: provider.to_owned(),
            issuer: issuer.to_owned(),
            external_sub: external_sub.to_owned(),
            created_at: now,
        })
    }

    async fn sync_oidc_attributes(
        &self,
        id: Uuid,
        display_name: &str,
        role: &str,
        email_verified: bool,
    ) -> Result<User> {
        let now = time::OffsetDateTime::now_utc();

        sqlx::query(
            "UPDATE users SET display_name = $1, role = $2, email_verified = $3, updated_at = $4 WHERE id = $5",
        )
        .bind(display_name)
        .bind(role)
        .bind(email_verified)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database { message: e.to_string() })?;

        self.find_by_id(id).await?.ok_or(Error::UserNotFound { id })
    }

    async fn health_check(&self) -> Result<()> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(|e| Error::Database {
                message: e.to_string(),
            })?;
        Ok(())
    }

    async fn find_by_email_in_tx(&self, tx: &mut Self::Tx, email: &str) -> Result<Option<User>> {
        sqlx::query(
            "SELECT id, email, display_name, role, email_verified, created_at, updated_at
             FROM users WHERE email = $1 FOR UPDATE",
        )
        .bind(email)
        .map(to_user_pg)
        .fetch_optional(&mut *tx.inner)
        .await
        .map_err(|e| Error::Database {
            message: e.to_string(),
        })
    }

    async fn create_in_tx(
        &self,
        tx: &mut Self::Tx,
        email: &str,
        display_name: &str,
        role: &str,
        email_verified: bool,
    ) -> Result<User> {
        let now = time::OffsetDateTime::now_utc();
        let id = Uuid::new_v4();

        sqlx::query(
            "INSERT INTO users (id, email, display_name, role, email_verified, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(id)
        .bind(email)
        .bind(display_name)
        .bind(role)
        .bind(email_verified)
        .bind(now)
        .bind(now)
        .execute(&mut *tx.inner)
        .await
        .map_err(|e| Error::Database { message: e.to_string() })?;

        let user = sqlx::query(
            "SELECT id, email, display_name, role, email_verified, created_at, updated_at
             FROM users WHERE id = $1",
        )
        .bind(id)
        .map(to_user_pg)
        .fetch_one(&mut *tx.inner)
        .await
        .map_err(|e| Error::Database {
            message: e.to_string(),
        })?;

        Ok(user)
    }

    async fn sync_oidc_attributes_in_tx(
        &self,
        tx: &mut Self::Tx,
        id: Uuid,
        display_name: &str,
        role: &str,
        email_verified: bool,
    ) -> Result<User> {
        let now = time::OffsetDateTime::now_utc();

        sqlx::query(
            "UPDATE users SET display_name = $1, role = $2, email_verified = $3, updated_at = $4
             WHERE id = $5",
        )
        .bind(display_name)
        .bind(role)
        .bind(email_verified)
        .bind(now)
        .bind(id)
        .execute(&mut *tx.inner)
        .await
        .map_err(|e| Error::Database {
            message: e.to_string(),
        })?;

        sqlx::query(
            "SELECT id, email, display_name, role, email_verified, created_at, updated_at
             FROM users WHERE id = $1",
        )
        .bind(id)
        .map(to_user_pg)
        .fetch_one(&mut *tx.inner)
        .await
        .map_err(|e| Error::Database {
            message: e.to_string(),
        })
    }

    async fn create_identity_in_tx(
        &self,
        tx: &mut Self::Tx,
        user_id: Uuid,
        provider: &str,
        issuer: &str,
        external_sub: &str,
    ) -> Result<UserIdentity> {
        let id = Uuid::new_v4();
        let now = time::OffsetDateTime::now_utc();

        sqlx::query(
            "INSERT INTO user_identities (id, user_id, provider, issuer, external_sub, created_at)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(id)
        .bind(user_id)
        .bind(provider)
        .bind(issuer)
        .bind(external_sub)
        .bind(now)
        .execute(&mut *tx.inner)
        .await
        .map_err(|e| Error::Database {
            message: e.to_string(),
        })?;

        Ok(UserIdentity {
            id,
            user_id,
            provider: provider.to_owned(),
            issuer: issuer.to_owned(),
            external_sub: external_sub.to_owned(),
            created_at: now,
        })
    }
}

#[cfg(test)]
mod tests {
    use config::DatabaseConfig;
    use testcontainers::runners::AsyncRunner;
    use testcontainers_modules::postgres::Postgres;

    use super::{Transaction, UserRepo};

    fn db_config(port: u16) -> DatabaseConfig {
        DatabaseConfig {
            driver: config::DatabaseDriver::Postgres,
            database_url: format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres"),
            max_connections: 2,
            connect_retry_attempts: 2,
            connect_retry_delay_ms: 1000,
            encrypt: false,
        }
    }

    #[tokio::test]
    async fn create_user_should_persist() {
        let container = Postgres::default().start().await.unwrap();
        let port = container.get_host_port_ipv4(5432).await.unwrap();
        let config = db_config(port);

        migration::run(&config).await.unwrap();

        let repo = crate::connect(&config).await.unwrap();

        let user = repo
            .create("alice@example.com", "Alice", "user", true)
            .await
            .unwrap();

        assert_eq!(user.email, "alice@example.com");
        assert_eq!(user.display_name, "Alice");
        assert_eq!(user.role, "user");
        assert!(user.email_verified);
    }

    #[tokio::test]
    async fn find_by_id_should_return_user() {
        let container = Postgres::default().start().await.unwrap();
        let port = container.get_host_port_ipv4(5432).await.unwrap();
        let config = db_config(port);

        migration::run(&config).await.unwrap();

        let repo = crate::connect(&config).await.unwrap();
        let created = repo
            .create("bob@example.com", "Bob", "user", true)
            .await
            .unwrap();

        let found = repo.find_by_id(created.id).await.unwrap();

        assert!(found.is_some());
        assert_eq!(found.unwrap().email, "bob@example.com");
    }

    #[tokio::test]
    async fn find_by_id_not_found() {
        let container = Postgres::default().start().await.unwrap();
        let port = container.get_host_port_ipv4(5432).await.unwrap();
        let config = db_config(port);

        migration::run(&config).await.unwrap();

        let repo = crate::connect(&config).await.unwrap();

        let result = repo.find_by_id(uuid::Uuid::new_v4()).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn delete_user_should_remove() {
        let container = Postgres::default().start().await.unwrap();
        let port = container.get_host_port_ipv4(5432).await.unwrap();
        let config = db_config(port);

        migration::run(&config).await.unwrap();

        let repo = crate::connect(&config).await.unwrap();
        let created = repo
            .create("carol@example.com", "Carol", "user", true)
            .await
            .unwrap();

        repo.delete(created.id).await.unwrap();

        let found = repo.find_by_id(created.id).await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn list_users_should_paginate() {
        let container = Postgres::default().start().await.unwrap();
        let port = container.get_host_port_ipv4(5432).await.unwrap();
        let config = db_config(port);

        migration::run(&config).await.unwrap();

        let repo = crate::connect(&config).await.unwrap();
        repo.create("a@example.com", "A", "user", true)
            .await
            .unwrap();
        repo.create("b@example.com", "B", "user", true)
            .await
            .unwrap();

        let (users, total) = repo.list(1, 10).await.unwrap();
        assert!(total >= 2);
        assert!(!users.is_empty());
    }

    #[tokio::test]
    async fn health_check_should_pass() {
        let container = Postgres::default().start().await.unwrap();
        let port = container.get_host_port_ipv4(5432).await.unwrap();
        let config = db_config(port);

        migration::run(&config).await.unwrap();

        let repo = crate::connect(&config).await.unwrap();
        repo.health_check().await.unwrap();
    }

    #[tokio::test]
    async fn jit_identity_flow() {
        let container = Postgres::default().start().await.unwrap();
        let port = container.get_host_port_ipv4(5432).await.unwrap();
        let config = db_config(port);

        migration::run(&config).await.unwrap();

        let repo = crate::connect(&config).await.unwrap();

        let user = repo
            .create("jit@example.com", "JIT User", "user", true)
            .await
            .unwrap();

        let identity = repo
            .create_identity(
                user.id,
                "oidc",
                "https://accounts.google.com",
                "google-12345",
            )
            .await
            .unwrap();

        assert_eq!(identity.provider, "oidc");
        assert_eq!(identity.issuer, "https://accounts.google.com");
        assert_eq!(identity.external_sub, "google-12345");
        assert_eq!(identity.user_id, user.id);

        let found = repo
            .find_identity("oidc", "https://accounts.google.com", "google-12345")
            .await
            .unwrap();
        assert!(found.is_some());

        let (found_user, _) = repo
            .find_by_identity("oidc", "https://accounts.google.com", "google-12345")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found_user.email, "jit@example.com");
    }

    #[tokio::test]
    async fn jit_provision_should_be_atomic_in_postgres() {
        let container = Postgres::default().start().await.unwrap();
        let port = container.get_host_port_ipv4(5432).await.unwrap();
        let config = db_config(port);

        migration::run(&config).await.unwrap();

        let repo = crate::connect(&config).await.unwrap();

        // 使用交易完成完整的 JIT provisioning
        let mut tx = repo.begin_transaction().await.unwrap();
        let user = repo
            .create_in_tx(&mut tx, "jit-tx@example.com", "JIT TX User", "user", true)
            .await
            .unwrap();
        let identity = repo
            .create_identity_in_tx(
                &mut tx,
                user.id,
                "oidc",
                "https://accounts.google.com",
                "google-tx-12345",
            )
            .await
            .unwrap();
        tx.commit().await.unwrap();

        assert_eq!(identity.user_id, user.id);

        // 驗證兩者都存在
        let found_user = repo.find_by_id(user.id).await.unwrap();
        assert!(found_user.is_some());
        let found_identity = repo
            .find_identity("oidc", "https://accounts.google.com", "google-tx-12345")
            .await
            .unwrap();
        assert!(found_identity.is_some());
    }
}
