//! Application errors and their exit codes (A§5.1, D-61).

use thiserror::Error;

use crate::storage::StorageError;

#[derive(Debug, Error)]
pub enum AppError {
    /// Bad input or usage: a bad dice expression, an unknown id.
    #[error("{0}")]
    Input(String),
    #[error(transparent)]
    Storage(#[from] StorageError),
    /// Writing to stdout or stderr failed.
    #[error("cannot write output: {0}")]
    Output(String),
}

impl AppError {
    /// 2 for invalid input or usage, 1 for environment failures.
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Input(_) | Self::Storage(StorageError::InvalidSessionName(_)) => 2,
            Self::Storage(_) | Self::Output(_) => 1,
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        Self::Output(e.to_string())
    }
}
