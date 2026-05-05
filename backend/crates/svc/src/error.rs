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
}

impl From<RepoError> for Error {
    fn from(source: RepoError) -> Self {
        match &source {
            RepoError::UserNotFound { id } => Error::NotFound { id: *id },
            _ => Error::Repository { source },
        }
    }
}
