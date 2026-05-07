pub mod connection;
pub mod error;
pub mod health;
pub mod user_repo;

pub use connection::connect;
pub use error::{Error, Result};
pub use health::HealthProbe;
pub use user_repo::{AnyTransaction, AnyUserRepo, MssqlUserRepo, PostgresUserRepo, UserRepo};

#[cfg(feature = "test-helpers")]
pub use user_repo::MockUserRepo;
