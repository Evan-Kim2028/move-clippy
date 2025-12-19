//! Unified error types for move-clippy.
//!
//! Library code uses `Error` and `Result<T>`.
//! Binary code (`main.rs`) uses `anyhow` for ergonomic CLI error handling.

use std::path::PathBuf;
use thiserror::Error as ThisError;

/// Unified error type for move-clippy library operations.
///
/// This enum captures all error conditions that can occur during linting.
#[derive(Debug, ThisError)]
pub enum Error {
    /// Failed to parse Move source code.
    #[error("parse error: {message}")]
    Parse {
        /// Description of what went wrong during parsing.
        message: String,
    },

    /// Semantic analysis failure (requires `--mode full`).
    #[error("semantic analysis failed: {message}")]
    Semantic {
        /// Description of what went wrong.
        message: String,
    },

    /// Configuration file error.
    #[error("configuration error in {}: {message}", path.display())]
    Config {
        /// Path to the problematic configuration file.
        path: PathBuf,
        /// Description of what went wrong.
        message: String,
    },

    /// Configuration parse error.
    #[error("failed to parse configuration: {0}")]
    ConfigParse(#[from] toml::de::Error),

    /// Unknown lint name provided.
    #[error("unknown lint: {0}")]
    UnknownLint(String),

    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Package resolution error (for `--mode full`).
    #[error("package error: {0}")]
    Package(String),

    /// Test fixture failure.
    #[error("fixture failure: {0}")]
    Fixture(String),

    /// Generic error with context.
    #[error("{context}: {message}")]
    WithContext {
        /// Context describing where the error occurred.
        context: String,
        /// The underlying error message.
        message: String,
    },

    /// Generic error for other cases.
    #[error("{0}")]
    Other(String),
}

impl Error {
    /// Create a parse error.
    pub fn parse(message: impl Into<String>) -> Self {
        Self::Parse {
            message: message.into(),
        }
    }

    /// Create a semantic error.
    pub fn semantic(message: impl Into<String>) -> Self {
        Self::Semantic {
            message: message.into(),
        }
    }

    /// Create a config error.
    pub fn config(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self::Config {
            path: path.into(),
            message: message.into(),
        }
    }

    /// Create an unknown lint error.
    pub fn unknown_lint(name: impl Into<String>) -> Self {
        Self::UnknownLint(name.into())
    }

    /// Create a package error.
    pub fn package(message: impl Into<String>) -> Self {
        Self::Package(message.into())
    }

    /// Create a fixture error.
    pub fn fixture(message: impl Into<String>) -> Self {
        Self::Fixture(message.into())
    }

    /// Create a generic error.
    pub fn other(message: impl Into<String>) -> Self {
        Self::Other(message.into())
    }

    /// Add context to an error.
    #[must_use]
    pub fn with_context(self, context: impl Into<String>) -> Self {
        Self::WithContext {
            context: context.into(),
            message: self.to_string(),
        }
    }
}

/// Result type alias for move-clippy library operations.
pub type Result<T> = std::result::Result<T, Error>;

// Backward compatibility aliases
/// Alias for `Error` (deprecated, use `Error` directly).
#[deprecated(since = "0.1.1", note = "use `Error` instead")]
pub type MoveClippyError = Error;

/// Alias for `Result<T>` (deprecated, use `Result<T>` directly).
#[deprecated(since = "0.1.1", note = "use `Result<T>` instead")]
pub type ClippyResult<T> = Result<T>;

// For compatibility with anyhow
// Note: anyhow::Error already has From<E: std::error::Error> so our Error
// (which derives thiserror::Error) can be converted with `?` or `.into()`.

impl From<anyhow::Error> for Error {
    fn from(err: anyhow::Error) -> Self {
        Error::other(err.to_string())
    }
}

/// Convenience macro mirroring `anyhow::bail!` but returning `Error`.
#[macro_export]
macro_rules! clippy_bail {
    ($($arg:tt)*) => {
        return Err($crate::error::Error::other(format!($($arg)*)));
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
#[must_use]
pub fn format_error_chain(err: &Error) -> String {
    format!("{err}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = Error::parse("unexpected token");
        assert_eq!(err.to_string(), "parse error: unexpected token");

        let err = Error::semantic("type mismatch");
        assert_eq!(err.to_string(), "semantic analysis failed: type mismatch");

        let err = Error::unknown_lint("fake_lint");
        assert_eq!(err.to_string(), "unknown lint: fake_lint");
    }

    #[test]
    fn test_error_with_context() {
        let err = Error::parse("syntax error").with_context("processing file.move");
        assert!(err.to_string().contains("processing file.move"));
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err: Error = io_err.into();
        assert!(matches!(err, Error::Io(_)));
    }
}
