use std::fmt;

use crate::protocol::ErrorCode;

/// An error that either goes back to the user as-is or is logged and hidden.
#[derive(Debug)]
pub enum Error {
    User(ErrorCode, String),
    Internal(anyhow::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    pub fn user(code: ErrorCode, message: impl Into<String>) -> Self {
        Self::User(code, message.into())
    }

    pub fn bad(message: impl Into<String>) -> Self {
        Self::user(ErrorCode::BadRequest, message)
    }

    /// What the client is told. Internal details stay in the server log.
    pub fn public(&self) -> (ErrorCode, String) {
        match self {
            Self::User(code, message) => (*code, message.clone()),
            Self::Internal(_) => (
                ErrorCode::Internal,
                "Something went wrong on the server. Try again.".into(),
            ),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::User(code, message) => write!(f, "{code:?}: {message}"),
            Self::Internal(err) => write!(f, "internal: {err:#}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<rusqlite::Error> for Error {
    fn from(err: rusqlite::Error) -> Self {
        Self::Internal(err.into())
    }
}

impl From<anyhow::Error> for Error {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err)
    }
}

impl From<tokio::task::JoinError> for Error {
    fn from(err: tokio::task::JoinError) -> Self {
        Self::Internal(err.into())
    }
}
