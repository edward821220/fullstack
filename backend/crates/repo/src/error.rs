use snafu::Snafu;
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
