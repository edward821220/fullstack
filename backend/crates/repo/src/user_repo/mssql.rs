use super::{Transaction, UserRepo};
use crate::{Error, Result};
use async_trait::async_trait;
use model::role::Role;
use model::user::User;
use model::user_identity::UserIdentity;
use tokio_util::compat::TokioAsyncWriteCompatExt;
use uuid::Uuid;
pub struct MssqlUserRepo {
    pool: bb8::Pool<bb8_tiberius::ConnectionManager>,
    config: tiberius::Config,
}
impl MssqlUserRepo {
    pub fn new(pool: bb8::Pool<bb8_tiberius::ConnectionManager>, config: tiberius::Config) -> Self {
        Self { pool, config }
    }
}
impl Clone for MssqlUserRepo {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
            config: self.config.clone(),
        }
    }
}
pub struct MssqlTransaction {
    client: tiberius::Client<tokio_util::compat::Compat<tokio::net::TcpStream>>,
}
#[async_trait]
impl Transaction for MssqlTransaction {
    async fn commit(mut self) -> Result<()> {
        self.client
            .simple_query("COMMIT TRAN")
            .await
            .map_err(|e| Error::Database {
                message: e.to_string(),
            })?;
        Ok(())
    }
    async fn rollback(mut self) -> Result<()> {
        self.client
            .simple_query("ROLLBACK TRAN")
            .await
            .map_err(|e| Error::Database {
                message: e.to_string(),
            })?;
        Ok(())
    }
}
fn row_to_user(row: &tiberius::Row) -> Result<User> {
    Ok(User {
        id: row.get::<Uuid, _>("id").ok_or_else(|| Error::Database {
            message: "id column missing".to_owned(),
        })?,
        email: row.get::<&str, _>("email").unwrap_or("").to_owned(),
        display_name: row.get::<&str, _>("display_name").unwrap_or("").to_owned(),
        role: row
            .get::<&str, _>("role")
            .unwrap_or("user")
            .parse()
            .map_err(|e: model::role::UnknownRoleError| Error::Database {
                message: format!("Invalid role in database: {e}"),
            })?,
        email_verified: row.get::<bool, _>("email_verified").unwrap_or(false),
        created_at: row
            .get::<time::PrimitiveDateTime, _>("created_at")
            .map(|dt| dt.assume_utc())
            .ok_or_else(|| Error::Database {
                message: "created_at column missing".to_owned(),
            })?,
        updated_at: row
            .get::<time::PrimitiveDateTime, _>("updated_at")
            .map(|dt| dt.assume_utc())
            .ok_or_else(|| Error::Database {
                message: "updated_at column missing".to_owned(),
            })?,
        version: row.get::<i64, _>("version").unwrap_or(1),
    })
}
#[async_trait]
impl UserRepo for MssqlUserRepo {
    type Tx = MssqlTransaction;
    async fn begin_transaction(&self) -> Result<Self::Tx> {
        let tcp = tokio::net::TcpStream::connect(self.config.get_addr())
            .await
            .map_err(|e| Error::Database {
                message: e.to_string(),
            })?;
        tcp.set_nodelay(true).map_err(|e| Error::Database {
            message: e.to_string(),
        })?;
        let mut client = tiberius::Client::connect(self.config.clone(), tcp.compat_write())
            .await
            .map_err(|e| Error::Database {
                message: e.to_string(),
            })?;
        client
            .simple_query("BEGIN TRAN")
            .await
            .map_err(|e| Error::Database {
                message: e.to_string(),
            })?;
        Ok(MssqlTransaction { client })
    }
    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>> {
        let mut client = self.pool.get().await.map_err(|e| Error::Database {
            message: e.to_string(),
        })?;
        let rows = client
            .query(
                "SELECT id, email, display_name, role, email_verified, created_at, updated_at, version FROM users WHERE id = @P1",
                &[&id],
            )
            .await
            .map_err(|e| Error::Database { message: e.to_string() })?;
        let row = rows.into_row().await.map_err(|e| Error::Database {
            message: e.to_string(),
        })?;
        match row {
            Some(ref r) => Ok(Some(row_to_user(r)?)),
            None => Ok(None),
        }
    }
    async fn find_by_email(&self, email: &str) -> Result<Option<User>> {
        let mut client = self.pool.get().await.map_err(|e| Error::Database {
            message: e.to_string(),
        })?;
        let rows = client
            .query(
                "SELECT id, email, display_name, role, email_verified, created_at, updated_at, version FROM users WHERE email = @P1",
                &[&email],
            )
            .await
            .map_err(|e| Error::Database { message: e.to_string() })?;
        let row = rows.into_row().await.map_err(|e| Error::Database {
            message: e.to_string(),
        })?;
        match row {
            Some(ref r) => Ok(Some(row_to_user(r)?)),
            None => Ok(None),
        }
    }
    async fn create(
        &self,
        email: &str,
        display_name: &str,
        role: Role,
        email_verified: bool,
    ) -> Result<User> {
        if self.find_by_email(email).await?.is_some() {
            return Err(Error::UserAlreadyExists {
                email: email.to_owned(),
            });
        }
        let now = time::OffsetDateTime::now_utc();
        let id = Uuid::new_v4();
        let mut client = self.pool.get().await.map_err(|e| Error::Database {
            message: e.to_string(),
        })?;
        let rows = client.query(
            "INSERT INTO users (id, email, display_name, role, email_verified, created_at, updated_at, version)
             OUTPUT INSERTED.id, INSERTED.email, INSERTED.display_name, INSERTED.role, INSERTED.email_verified, INSERTED.created_at, INSERTED.updated_at, INSERTED.version
             VALUES (@P1, @P2, @P3, @P4, @P5, @P6, @P7, 1)",
            &[
                &id,
                &email,
                &display_name,
                &role.as_str(),
                &email_verified,
                &time::PrimitiveDateTime::new(now.date(), now.time()),
                &time::PrimitiveDateTime::new(now.date(), now.time()),
            ],
        )
        .await
        .map_err(|e| Error::Database { message: e.to_string() })?;
        let row = rows.into_row().await.map_err(|e| Error::Database {
            message: e.to_string(),
        })?;
        row.ok_or_else(|| Error::Database {
            message: "INSERT did not return a row".to_owned(),
        })
        .and_then(|r| row_to_user(&r))
    }
    async fn update(
        &self,
        id: Uuid,
        display_name: Option<&str>,
        version: Option<i64>,
    ) -> Result<User> {
        let user = self
            .find_by_id(id)
            .await?
            .ok_or(Error::UserNotFound { id })?;
        let new_name = display_name.unwrap_or(&user.display_name);
        let expected_version = version.unwrap_or(user.version);
        let now = time::OffsetDateTime::now_utc();
        let mut client = self.pool.get().await.map_err(|e| Error::Database {
            message: e.to_string(),
        })?;
        let rows = client.query(
            "UPDATE users SET display_name = @P1, updated_at = @P2, version = version + 1
             OUTPUT INSERTED.id, INSERTED.email, INSERTED.display_name, INSERTED.role, INSERTED.email_verified, INSERTED.created_at, INSERTED.updated_at, INSERTED.version
             WHERE id = @P3 AND version = @P4",
            &[&new_name, &time::PrimitiveDateTime::new(now.date(), now.time()), &id, &expected_version],
        )
        .await
        .map_err(|e| Error::Database { message: e.to_string() })?;
        let row = rows.into_row().await.map_err(|e| Error::Database {
            message: e.to_string(),
        })?;
        match row {
            Some(r) => row_to_user(&r),
            None => Err(Error::Conflict {
                resource: "user".to_owned(),
                expected_version,
            }),
        }
    }
    async fn delete(&self, id: Uuid) -> Result<()> {
        let mut client = self.pool.get().await.map_err(|e| Error::Database {
            message: e.to_string(),
        })?;
        let result = client
            .execute("DELETE FROM users WHERE id = @P1", &[&id])
            .await
            .map_err(|e| Error::Database {
                message: e.to_string(),
            })?;
        if result.total() == 0 {
            return Err(Error::UserNotFound { id });
        }
        Ok(())
    }
    async fn list(&self, page: u64, per_page: u64) -> Result<(Vec<User>, u64)> {
        let mut client = self.pool.get().await.map_err(|e| Error::Database {
            message: e.to_string(),
        })?;
        let count_row = client
            .query("SELECT COUNT(*) FROM users", &[])
            .await
            .map_err(|e| Error::Database {
                message: e.to_string(),
            })?
            .into_row()
            .await
            .map_err(|e| Error::Database {
                message: e.to_string(),
            })?
            .ok_or_else(|| Error::Database {
                message: "COUNT query returned no rows".to_owned(),
            })?;
        let total: i32 = count_row.get(0).ok_or_else(|| Error::Database {
            message: "COUNT column missing".to_owned(),
        })?;
        let offset = ((page - 1) * per_page) as i32;
        let limit = per_page as i32;
        let rows = client
            .query(
                "SELECT id, email, display_name, role, email_verified, created_at, updated_at, version
                 FROM users ORDER BY created_at DESC OFFSET @P1 ROWS FETCH NEXT @P2 ROWS ONLY",
                &[&offset, &limit],
            )
            .await
            .map_err(|e| Error::Database {
                message: e.to_string(),
            })?;
        let users: Vec<User> = rows
            .into_results()
            .await
            .map_err(|e| Error::Database {
                message: e.to_string(),
            })?
            .into_iter()
            .flat_map(|rs| rs.into_iter())
            .map(|row| row_to_user(&row))
            .collect::<Result<Vec<_>>>()?;
        Ok((users, total as u64))
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
        let mut client = self.pool.get().await.map_err(|e| Error::Database {
            message: e.to_string(),
        })?;
        let row = client
            .query(
                "SELECT id, user_id, provider, issuer, external_sub, created_at
                 FROM user_identities WHERE provider = @P1 AND issuer = @P2 AND external_sub = @P3",
                &[&provider, &issuer, &external_sub],
            )
            .await
            .map_err(|e| Error::Database {
                message: e.to_string(),
            })?
            .into_row()
            .await
            .map_err(|e| Error::Database {
                message: e.to_string(),
            })?;
        match row {
            Some(r) => Ok(Some(UserIdentity {
                id: r.get("id").ok_or_else(|| Error::Database {
                    message: "id missing".to_owned(),
                })?,
                user_id: r.get("user_id").ok_or_else(|| Error::Database {
                    message: "user_id missing".to_owned(),
                })?,
                provider: r.get::<&str, _>("provider").unwrap_or("").to_owned(),
                issuer: r.get::<&str, _>("issuer").unwrap_or("").to_owned(),
                external_sub: r.get::<&str, _>("external_sub").unwrap_or("").to_owned(),
                created_at: r
                    .get::<time::PrimitiveDateTime, _>("created_at")
                    .map(|dt| dt.assume_utc())
                    .ok_or_else(|| Error::Database {
                        message: "created_at missing".to_owned(),
                    })?,
            })),
            None => Ok(None),
        }
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
        let mut client = self.pool.get().await.map_err(|e| Error::Database {
            message: e.to_string(),
        })?;
        client
            .execute(
                "INSERT INTO user_identities (id, user_id, provider, issuer, external_sub, created_at)
                 VALUES (@P1, @P2, @P3, @P4, @P5, @P6)",
                &[
                    &id,
                    &user_id,
                    &provider,
                    &issuer,
                    &external_sub,
                    &time::PrimitiveDateTime::new(now.date(), now.time()),
                ],
            )
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
        role: Role,
        email_verified: bool,
    ) -> Result<User> {
        let now = time::OffsetDateTime::now_utc();
        let mut client = self.pool.get().await.map_err(|e| Error::Database {
            message: e.to_string(),
        })?;
        let rows = client.query(
            "UPDATE users SET display_name = @P1, role = @P2, email_verified = @P3, updated_at = @P4, version = version + 1
             OUTPUT INSERTED.id, INSERTED.email, INSERTED.display_name, INSERTED.role, INSERTED.email_verified, INSERTED.created_at, INSERTED.updated_at, INSERTED.version
             WHERE id = @P5",
            &[&display_name, &role.as_str(), &email_verified, &time::PrimitiveDateTime::new(now.date(), now.time()), &id],
        )
        .await
        .map_err(|e| Error::Database { message: e.to_string() })?;
        let row = rows.into_row().await.map_err(|e| Error::Database {
            message: e.to_string(),
        })?;
        row.ok_or(Error::UserNotFound { id })
            .and_then(|r| row_to_user(&r))
    }
    async fn find_by_email_in_tx(&self, tx: &mut Self::Tx, email: &str) -> Result<Option<User>> {
        let rows = tx
            .client
            .query(
                "SELECT id, email, display_name, role, email_verified, created_at, updated_at, version
                 FROM users WITH (UPDLOCK, HOLDLOCK) WHERE email = @P1",
                &[&email],
            )
            .await
            .map_err(|e| Error::Database {
                message: e.to_string(),
            })?;
        let row = rows.into_row().await.map_err(|e| Error::Database {
            message: e.to_string(),
        })?;
        match row {
            Some(ref r) => Ok(Some(row_to_user(r)?)),
            None => Ok(None),
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
        let now = time::OffsetDateTime::now_utc();
        let id = Uuid::new_v4();
        let rows = tx.client.query(
            "INSERT INTO users (id, email, display_name, role, email_verified, created_at, updated_at, version)
             OUTPUT INSERTED.id, INSERTED.email, INSERTED.display_name, INSERTED.role, INSERTED.email_verified, INSERTED.created_at, INSERTED.updated_at, INSERTED.version
             VALUES (@P1, @P2, @P3, @P4, @P5, @P6, @P7, 1)",
            &[
                &id,
                &email,
                &display_name,
                &role.as_str(),
                &email_verified,
                &time::PrimitiveDateTime::new(now.date(), now.time()),
                &time::PrimitiveDateTime::new(now.date(), now.time()),
            ],
        )
        .await
        .map_err(|e| Error::Database { message: e.to_string() })?;
        let row = rows.into_row().await.map_err(|e| Error::Database {
            message: e.to_string(),
        })?;
        row.ok_or_else(|| Error::Database {
            message: "INSERT did not return a row".to_owned(),
        })
        .and_then(|r| row_to_user(&r))
    }
    async fn sync_oidc_attributes_in_tx(
        &self,
        tx: &mut Self::Tx,
        id: Uuid,
        display_name: &str,
        role: Role,
        email_verified: bool,
    ) -> Result<User> {
        let now = time::OffsetDateTime::now_utc();
        let rows = tx.client.query(
            "UPDATE users SET display_name = @P1, role = @P2, email_verified = @P3, updated_at = @P4, version = version + 1
             OUTPUT INSERTED.id, INSERTED.email, INSERTED.display_name, INSERTED.role, INSERTED.email_verified, INSERTED.created_at, INSERTED.updated_at, INSERTED.version
             WHERE id = @P5",
            &[
                &display_name,
                &role.as_str(),
                &email_verified,
                &time::PrimitiveDateTime::new(now.date(), now.time()),
                &id,
            ],
        )
        .await
        .map_err(|e| Error::Database { message: e.to_string() })?;
        let row = rows.into_row().await.map_err(|e| Error::Database {
            message: e.to_string(),
        })?;
        row.ok_or(Error::UserNotFound { id })
            .and_then(|r| row_to_user(&r))
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
        tx.client
            .execute(
                "INSERT INTO user_identities (id, user_id, provider, issuer, external_sub, created_at)
                 VALUES (@P1, @P2, @P3, @P4, @P5, @P6)",
                &[
                    &id,
                    &user_id,
                    &provider,
                    &issuer,
                    &external_sub,
                    &time::PrimitiveDateTime::new(now.date(), now.time()),
                ],
            )
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
    use super::{Transaction, UserRepo};
    use config::DatabaseConfig;
    use testcontainers::{ImageExt, runners::AsyncRunner};
    use testcontainers_modules::mssql_server::MssqlServer;

    fn db_config(port: u16) -> DatabaseConfig {
        DatabaseConfig {
            driver: config::DatabaseDriver::Mssql,
            host: "127.0.0.1".to_owned(),
            port,
            database: "master".to_owned(),
            username: "sa".to_owned(),
            password: "yourStrong(!)Password".to_owned(),
            password_file: None,
            max_connections: 2,
            connect_retry_attempts: 5,
            connect_retry_delay_ms: 2000,
            encrypt: false,
            trust_cert: false,
            ca_cert_path: None,
            run_migrations_on_startup: true,
        }
    }

    #[tokio::test]
    async fn create_user_should_persist() {
        let container = MssqlServer::default()
            .with_env_var("ACCEPT_EULA", "Y")
            .with_env_var("MSSQL_PID", "Developer")
            .start()
            .await
            .unwrap();
        let port = container.get_host_port_ipv4(1433).await.unwrap();
        let config = db_config(port);

        migration::run(&config).await.unwrap();
        let (repo, _probe, _metrics) = crate::connect(
            &config,
            tokio_util::sync::CancellationToken::new(),
            std::time::Duration::from_secs(15),
        )
        .await
        .unwrap();

        let user = repo
            .create("alice@example.com", "Alice", model::role::Role::User, true)
            .await
            .unwrap();

        assert_eq!(user.email, "alice@example.com");
        assert_eq!(user.display_name, "Alice");
        assert_eq!(user.role, model::role::Role::User);
        assert!(user.email_verified);
    }

    #[tokio::test]
    async fn find_by_id_should_return_user() {
        let container = MssqlServer::default()
            .with_env_var("ACCEPT_EULA", "Y")
            .with_env_var("MSSQL_PID", "Developer")
            .start()
            .await
            .unwrap();
        let port = container.get_host_port_ipv4(1433).await.unwrap();
        let config = db_config(port);

        migration::run(&config).await.unwrap();
        let (repo, _probe, _metrics) = crate::connect(
            &config,
            tokio_util::sync::CancellationToken::new(),
            std::time::Duration::from_secs(15),
        )
        .await
        .unwrap();

        let created = repo
            .create("bob@example.com", "Bob", model::role::Role::User, true)
            .await
            .unwrap();

        let found = repo.find_by_id(created.id).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().email, "bob@example.com");
    }

    #[tokio::test]
    async fn find_by_id_not_found() {
        let container = MssqlServer::default()
            .with_env_var("ACCEPT_EULA", "Y")
            .with_env_var("MSSQL_PID", "Developer")
            .start()
            .await
            .unwrap();
        let port = container.get_host_port_ipv4(1433).await.unwrap();
        let config = db_config(port);

        migration::run(&config).await.unwrap();
        let (repo, _probe, _metrics) = crate::connect(
            &config,
            tokio_util::sync::CancellationToken::new(),
            std::time::Duration::from_secs(15),
        )
        .await
        .unwrap();

        let result = repo.find_by_id(uuid::Uuid::new_v4()).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn delete_user_should_remove() {
        let container = MssqlServer::default()
            .with_env_var("ACCEPT_EULA", "Y")
            .with_env_var("MSSQL_PID", "Developer")
            .start()
            .await
            .unwrap();
        let port = container.get_host_port_ipv4(1433).await.unwrap();
        let config = db_config(port);

        migration::run(&config).await.unwrap();
        let (repo, _probe, _metrics) = crate::connect(
            &config,
            tokio_util::sync::CancellationToken::new(),
            std::time::Duration::from_secs(15),
        )
        .await
        .unwrap();

        let created = repo
            .create("carol@example.com", "Carol", model::role::Role::User, true)
            .await
            .unwrap();

        repo.delete(created.id).await.unwrap();
        let found = repo.find_by_id(created.id).await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn jit_identity_flow() {
        let container = MssqlServer::default()
            .with_env_var("ACCEPT_EULA", "Y")
            .with_env_var("MSSQL_PID", "Developer")
            .start()
            .await
            .unwrap();
        let port = container.get_host_port_ipv4(1433).await.unwrap();
        let config = db_config(port);

        migration::run(&config).await.unwrap();
        let (repo, _probe, _metrics) = crate::connect(
            &config,
            tokio_util::sync::CancellationToken::new(),
            std::time::Duration::from_secs(15),
        )
        .await
        .unwrap();

        let user = repo
            .create("jit@example.com", "JIT User", model::role::Role::User, true)
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
    async fn jit_provision_should_be_atomic_in_mssql() {
        let container = MssqlServer::default()
            .with_env_var("ACCEPT_EULA", "Y")
            .with_env_var("MSSQL_PID", "Developer")
            .start()
            .await
            .unwrap();
        let port = container.get_host_port_ipv4(1433).await.unwrap();
        let config = db_config(port);

        migration::run(&config).await.unwrap();
        let (repo, _probe, _metrics) = crate::connect(
            &config,
            tokio_util::sync::CancellationToken::new(),
            std::time::Duration::from_secs(15),
        )
        .await
        .unwrap();

        let mut tx = repo.begin_transaction().await.unwrap();
        let user = repo
            .create_in_tx(
                &mut tx,
                "jit-tx@example.com",
                "JIT TX User",
                model::role::Role::User,
                true,
            )
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

        let found_user = repo.find_by_id(user.id).await.unwrap();
        assert!(found_user.is_some());
        let found_identity = repo
            .find_identity("oidc", "https://accounts.google.com", "google-tx-12345")
            .await
            .unwrap();
        assert!(found_identity.is_some());
    }
}
