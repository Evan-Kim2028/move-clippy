use anyhow::Error as AnyhowError;
use thiserror::Error;

/// Result alias for errors emitted by Move Clippy internals.
pub type ClippyResult<T> = Result<T, MoveClippyError>;

/// Structured error type for Move Clippy subsystems.
#[derive(Debug, Error)]
pub enum MoveClippyError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("semantic lint failure: {0}")]
    Semantic(String),

    #[error("fixture failure: {0}")]
    Fixture(String),

    #[error("{0}")]
    Other(String),
}

impl MoveClippyError {
    pub fn semantic(msg: impl Into<String>) -> Self {
        Self::Semantic(msg.into())
    }

    pub fn fixture(msg: impl Into<String>) -> Self {
        Self::Fixture(msg.into())
    }

    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }

    /// Convert to anyhow::Error for interop with anyhow-based code.
    pub fn into_anyhow(self) -> AnyhowError {
        AnyhowError::new(self)
    }
}

impl From<AnyhowError> for MoveClippyError {
    fn from(err: AnyhowError) -> Self {
        MoveClippyError::other(err.to_string())
    }
}

/// Convenience macro mirroring `anyhow::bail!` but returning MoveClippyError.
#[macro_export]
macro_rules! clippy_bail {
    ($($arg:tt)*) => {
        return Err($crate::error::MoveClippyError::other(format!($($arg)*)));
    };
}

/// Convenience macro mirroring `anyhow::ensure!`.
#[macro_export]
macro_rules! clippy_ensure {
    ($cond:expr, $($arg:tt)*) => {
        if !($cond) {
            $crate::clippy_bail!($($arg)*);
        }
    };
}

/// Utility for pretty printing aggregated errors inside tests.
pub fn format_error_chain(err: &MoveClippyError) -> String {
    format!("{err}")
}
