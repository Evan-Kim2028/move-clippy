use crate::level::LintLevel;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Deserialize)]
pub struct MoveClippyConfig {
    #[serde(default)]
    pub lints: LintsConfig,
}

#[derive(Debug, Default, Deserialize)]
pub struct LintsConfig {
    #[serde(default)]
    pub disabled: Vec<String>,

    #[serde(flatten)]
    pub levels: HashMap<String, LintLevel>,
}

pub const DEFAULT_CONFIG_FILE_NAME: &str = "move-clippy.toml";

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

pub fn load_config_file(path: &Path) -> Result<MoveClippyConfig> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read config file: {}", path.display()))?;
    let cfg: MoveClippyConfig = toml::from_str(&raw)
        .with_context(|| format!("failed to parse config file: {}", path.display()))?;
    Ok(cfg)
}

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
