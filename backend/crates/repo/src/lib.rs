use async_trait::async_trait;
use model::user::User;
use model::user_identity::UserIdentity;
use snafu::Snafu;
use std::time::Duration;
use uuid::Uuid;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Database error: {message}"))]
    Database { message: String },

    #[snafu(display("User with id {id} not found"))]
    UserNotFound { id: Uuid },

    #[snafu(display("User with email {email} already exists"))]
    UserAlreadyExists { email: String },

    #[snafu(display("Identity for provider {provider} sub {external_sub} not found"))]
    IdentityNotFound {
        provider: String,
        external_sub: String,
    },
}

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

// ── PostgreSQL Implementation ──────────────────────────────────────────────

pub struct PostgresUserRepo {
    pool: sqlx::PgPool,
}

impl PostgresUserRepo {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

fn to_user_pg(row: sqlx::postgres::PgRow) -> User {
    use sqlx::Row;
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

#[async_trait]
impl UserRepo for PostgresUserRepo {
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
        sqlx::query_as::<_, UserIdentity>(
            "SELECT id, user_id, provider, issuer, external_sub, created_at
             FROM user_identities WHERE provider = $1 AND issuer = $2 AND external_sub = $3",
        )
        .bind(provider)
        .bind(issuer)
        .bind(external_sub)
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
}

// ── MSSQL Implementation ───────────────────────────────────────────────────

pub struct MssqlUserRepo {
    pool: bb8::Pool<bb8_tiberius::ConnectionManager>,
}

impl MssqlUserRepo {
    pub fn new(pool: bb8::Pool<bb8_tiberius::ConnectionManager>) -> Self {
        Self { pool }
    }
}

fn row_to_user(row: &tiberius::Row) -> Result<User> {
    Ok(User {
        id: row.get::<Uuid, _>("id").ok_or_else(|| Error::Database {
            message: "id column missing".to_owned(),
        })?,
        email: row.get::<&str, _>("email").unwrap_or("").to_owned(),
        display_name: row.get::<&str, _>("display_name").unwrap_or("").to_owned(),
        role: row.get::<&str, _>("role").unwrap_or("user").to_owned(),
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
    })
}

#[async_trait]
impl UserRepo for MssqlUserRepo {
    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>> {
        let mut client = self.pool.get().await.map_err(|e| Error::Database {
            message: e.to_string(),
        })?;

        let rows = client
            .query(
                "SELECT id, email, display_name, role, email_verified, created_at, updated_at FROM users WHERE id = @P1",
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
                "SELECT id, email, display_name, role, email_verified, created_at, updated_at FROM users WHERE email = @P1",
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

        let mut client = self.pool.get().await.map_err(|e| Error::Database {
            message: e.to_string(),
        })?;

        let rows = client.query(
            "INSERT INTO users (id, email, display_name, role, email_verified, created_at, updated_at)
             OUTPUT INSERTED.id, INSERTED.email, INSERTED.display_name, INSERTED.role, INSERTED.email_verified, INSERTED.created_at, INSERTED.updated_at
             VALUES (@P1, @P2, @P3, @P4, @P5, @P6, @P7)",
            &[
                &id,
                &email,
                &display_name,
                &role,
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

    async fn update(&self, id: Uuid, display_name: Option<&str>) -> Result<User> {
        let user = self
            .find_by_id(id)
            .await?
            .ok_or(Error::UserNotFound { id })?;

        let new_name = display_name.unwrap_or(&user.display_name);
        let now = time::OffsetDateTime::now_utc();

        let mut client = self.pool.get().await.map_err(|e| Error::Database {
            message: e.to_string(),
        })?;

        let rows = client.query(
            "UPDATE users SET display_name = @P1, updated_at = @P2
             OUTPUT INSERTED.id, INSERTED.email, INSERTED.display_name, INSERTED.role, INSERTED.email_verified, INSERTED.created_at, INSERTED.updated_at
             WHERE id = @P3",
            &[&new_name, &time::PrimitiveDateTime::new(now.date(), now.time()), &id],
        )
        .await
        .map_err(|e| Error::Database { message: e.to_string() })?;

        let row = rows.into_row().await.map_err(|e| Error::Database {
            message: e.to_string(),
        })?;

        row.ok_or(Error::UserNotFound { id })
            .and_then(|r| row_to_user(&r))
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
                "SELECT id, email, display_name, role, email_verified, created_at, updated_at
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
        role: &str,
        email_verified: bool,
    ) -> Result<User> {
        let now = time::OffsetDateTime::now_utc();

        let mut client = self.pool.get().await.map_err(|e| Error::Database {
            message: e.to_string(),
        })?;

        let rows = client.query(
            "UPDATE users SET display_name = @P1, role = @P2, email_verified = @P3, updated_at = @P4
             OUTPUT INSERTED.id, INSERTED.email, INSERTED.display_name, INSERTED.role, INSERTED.email_verified, INSERTED.created_at, INSERTED.updated_at
             WHERE id = @P5",
            &[&display_name, &role, &email_verified, &time::PrimitiveDateTime::new(now.date(), now.time()), &id],
        )
        .await
        .map_err(|e| Error::Database { message: e.to_string() })?;

        let row = rows.into_row().await.map_err(|e| Error::Database {
            message: e.to_string(),
        })?;

        row.ok_or(Error::UserNotFound { id })
            .and_then(|r| row_to_user(&r))
    }

    async fn health_check(&self) -> Result<()> {
        let mut client = self.pool.get().await.map_err(|e| Error::Database {
            message: e.to_string(),
        })?;

        client
            .query("SELECT 1", &[])
            .await
            .map_err(|e| Error::Database {
                message: e.to_string(),
            })?;

        Ok(())
    }
}

// ── Connection factory ─────────────────────────────────────────────────────
// Strategy: select PostgresUserRepo or MssqlUserRepo at runtime from config.

use config::DatabaseConfig;

pub async fn connect(config: &DatabaseConfig) -> Result<Box<dyn UserRepo>> {
    use config::DatabaseDriver;

    match config.driver() {
        DatabaseDriver::Postgres => {
            let pool = sqlx::postgres::PgPoolOptions::new()
                .max_connections(config.max_connections)
                .acquire_timeout(Duration::from_secs(10))
                .connect(&config.database_url)
                .await
                .map_err(|e| Error::Database {
                    message: e.to_string(),
                })?;

            Ok(Box::new(PostgresUserRepo::new(pool)))
        }
        DatabaseDriver::Mssql => {
            let tiberius_config = config.to_tiberius_config().map_err(|e| Error::Database {
                message: e.to_string(),
            })?;
            let mgr = bb8_tiberius::ConnectionManager::new(tiberius_config);
            let pool = bb8::Pool::builder()
                .max_size(config.max_connections)
                .build(mgr)
                .await
                .map_err(|e| Error::Database {
                    message: e.to_string(),
                })?;

            Ok(Box::new(MssqlUserRepo::new(pool)))
        }
    }
}
