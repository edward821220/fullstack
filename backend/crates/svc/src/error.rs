use snafu::Snafu;
use uuid::Uuid;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Database error: {message}"))]
    Database { message: String },

    #[snafu(display("User with id {id} not found"))]
    NotFound { id: Uuid },

    #[snafu(display("User with email {email} already exists"))]
    UserAlreadyExists { email: String },

    #[snafu(display("Identity for provider {provider} sub {external_sub} not found"))]
    IdentityNotFound {
        provider: String,
        external_sub: String,
    },

    #[snafu(display("{resource} was modified concurrently (expected version {expected_version})"))]
    Conflict {
        resource: String,
        expected_version: i64,
    },

    #[snafu(display("Invalid role: {role}"))]
    InvalidRole { role: String },

    #[snafu(display("Invalid input: {message}"))]
    InvalidInput { message: String },

    #[snafu(display("User with email {email} not in whitelist"))]
    NotInWhitelist { email: String },
}

impl From<repo::Error> for Error {
    fn from(source: repo::Error) -> Self {
        match source {
            repo::Error::Database { message } => Error::Database { message },
            repo::Error::UserNotFound { id } => Error::NotFound { id },
            repo::Error::UserAlreadyExists { email } => Error::UserAlreadyExists { email },
            repo::Error::IdentityNotFound {
                provider,
                external_sub,
            } => Error::IdentityNotFound {
                provider,
                external_sub,
            },
            repo::Error::Transaction { message } => Error::Database { message },
            repo::Error::Conflict {
                resource,
                expected_version,
            } => Error::Conflict {
                resource,
                expected_version,
            },
            repo::Error::InvalidRole { role } => Error::InvalidRole { role },
        }
    }
}
