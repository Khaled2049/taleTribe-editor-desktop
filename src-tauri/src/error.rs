use serde::{ser::SerializeStruct, Serialize, Serializer};

use crate::ai::AiError;
use crate::secrets::KeychainError;
use crate::sync::SyncError;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0} not found")]
    NotFound(&'static str),
    #[error("{0}")]
    Validation(String),
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Ai(#[from] AiError),
    #[error(transparent)]
    Keychain(#[from] KeychainError),
    #[error(transparent)]
    Sync(#[from] SyncError),
}

impl AppError {
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation(message.into())
    }

    fn kind(&self) -> &'static str {
        match self {
            Self::NotFound(_) => "notFound",
            Self::Validation(_) => "validation",
            Self::Db(_) => "database",
            Self::Io(_) => "io",
            Self::Ai(_) => "ai",
            Self::Keychain(_) => "keychain",
            Self::Sync(_) => "sync",
        }
    }
}

// The webview receives `{ kind, message }`, plus `code` for an AI failure, so
// it can tell a bad input from a failure and point at Settings when that helps.
impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut out = serializer.serialize_struct("AppError", 3)?;
        out.serialize_field("kind", self.kind())?;
        out.serialize_field("message", &self.to_string())?;
        match self {
            Self::Ai(error) => out.serialize_field("code", error.code())?,
            Self::Sync(error) => out.serialize_field("code", error.code())?,
            _ => out.skip_field("code")?,
        }
        out.end()
    }
}

pub type AppResult<T> = Result<T, AppError>;
