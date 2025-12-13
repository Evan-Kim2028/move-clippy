use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LintLevel {
    Allow,
    Warn,
    Error,
}

impl LintLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LintLevel::Allow => "allow",
            LintLevel::Warn => "warning",
            LintLevel::Error => "error",
        }
    }
}

impl Default for LintLevel {
    fn default() -> Self {
        Self::Warn
    }
}
