use repo::Error as RepoError;
use snafu::Snafu;
use uuid::Uuid;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Repository error: {source}"))]
    Repository { source: RepoError },

    #[snafu(display("Invalid input: {message}"))]
    InvalidInput { message: String },

    #[snafu(display("User with id {id} not found"))]
    NotFound { id: Uuid },

    #[snafu(display("User with email {email} not in whitelist"))]
    NotInWhitelist { email: String },

    #[snafu(display("{resource} was modified concurrently (expected version {expected_version})"))]
    Conflict {
        resource: String,
        expected_version: i64,
    },
}

impl From<RepoError> for Error {
    fn from(source: RepoError) -> Self {
        match &source {
            RepoError::UserNotFound { id } => Error::NotFound { id: *id },
            RepoError::Conflict {
                resource,
                expected_version,
            } => Error::Conflict {
                resource: resource.clone(),
                expected_version: *expected_version,
            },
            _ => Error::Repository { source },
        }
    }
}
