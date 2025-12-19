use crate::level::LintLevel;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Top-level configuration loaded from `move-clippy.toml`.
#[derive(Debug, Default, Deserialize)]
pub struct MoveClippyConfig {
    #[serde(default)]
    pub lints: LintsConfig,
}

/// Per-lint configuration under the `[lints]` section.
#[derive(Debug, Default, Deserialize)]
pub struct LintsConfig {
    /// Lints that should be treated as effectively disabled.
    #[serde(default)]
    pub disabled: Vec<String>,

    /// Enable preview rules that are not yet stable.
    ///
    /// Preview rules may have higher false-positive rates or change behavior
    /// between versions.
    #[serde(default)]
    pub preview: bool,

    /// Apply unsafe fixes when running with --fix.
    ///
    /// Unsafe fixes may change runtime behavior.
    #[serde(default)]
    pub unsafe_fixes: bool,

    /// Explicit per-lint levels (e.g. `modern_module_syntax = "error"`).
    #[serde(flatten)]
    pub levels: HashMap<String, LintLevel>,
}

/// Default file name for configuration that `move-clippy` searches for.
pub const DEFAULT_CONFIG_FILE_NAME: &str = "move-clippy.toml";

/// Walk up from `start_dir` to find the nearest `move-clippy.toml`, if any.
#[must_use]
pub fn find_config_file(start_dir: &Path) -> Option<PathBuf> {
    let mut cur = Some(start_dir);
    while let Some(dir) = cur {
        let candidate = dir.join(DEFAULT_CONFIG_FILE_NAME);
        if candidate.is_file() {
            return Some(candidate);
        }
        cur = dir.parent();
    }
    None
}

/// Load and parse a configuration file from disk.
#[must_use = "configuration may contain important settings"]
pub fn load_config_file(path: &Path) -> Result<MoveClippyConfig> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read config file: {}", path.display()))?;
    let cfg: MoveClippyConfig = toml::from_str(&raw)
        .with_context(|| format!("failed to parse config file: {}", path.display()))?;
    Ok(cfg)
}

/// Load configuration from an explicit path or by searching from `start_dir`.
#[must_use = "configuration may contain important settings"]
pub fn load_config(
    explicit_path: Option<&Path>,
    start_dir: &Path,
) -> Result<Option<(PathBuf, MoveClippyConfig)>> {
    if let Some(p) = explicit_path {
        let cfg = load_config_file(p)?;
        return Ok(Some((p.to_path_buf(), cfg)));
    }

    let Some(p) = find_config_file(start_dir) else {
        return Ok(None);
    };
    let cfg = load_config_file(&p)?;
    Ok(Some((p, cfg)))
}
