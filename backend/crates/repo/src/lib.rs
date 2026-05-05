pub mod connection;
pub mod error;
pub mod user_repo;

pub use connection::connect;
pub use error::{Error, Result};
pub use user_repo::{MssqlUserRepo, PostgresUserRepo, UserRepo};

#[cfg(feature = "test-helpers")]
pub use user_repo::MockUserRepo;
