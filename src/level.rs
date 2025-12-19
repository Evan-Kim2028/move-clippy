use serde::{Deserialize, Serialize};

/// Per-lint severity level used by diagnostics and configuration.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "lowercase")]
pub enum LintLevel {
    Allow,
    #[default]
    Warn,
    Error,
}

impl LintLevel {
    /// String representation used in CLI output and config files.
    pub fn as_str(&self) -> &'static str {
        match self {
            LintLevel::Allow => "allow",
            LintLevel::Warn => "warning",
            LintLevel::Error => "error",
        }
    }
}
