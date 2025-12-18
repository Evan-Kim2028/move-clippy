#![allow(dead_code)]

use std::fs;
use tempfile::TempDir;

pub const DEFAULT_MOVE_TOML_WITH_SUI_ADDR: &str = r#"[package]
name = "spec_test_pkg"
edition = "2024"

[addresses]
spec_test_pkg = "0x0"
sui = "0x2"
"#;

pub fn create_temp_package(move_toml: &str, sources: &[(&str, &str)]) -> std::io::Result<TempDir> {
    let tmp = tempfile::tempdir()?;
    fs::write(tmp.path().join("Move.toml"), move_toml)?;

    let sources_dir = tmp.path().join("sources");
    fs::create_dir_all(&sources_dir)?;
    for (filename, content) in sources {
        fs::write(sources_dir.join(filename), content)?;
    }

    Ok(tmp)
}

/// Creates a minimal Move package for semantic spec tests.
///
/// Includes a local `sui` address so tests can define `module sui::...` shims.
pub fn create_temp_sui_package(source: &str) -> std::io::Result<TempDir> {
    create_temp_package(DEFAULT_MOVE_TOML_WITH_SUI_ADDR, &[("test.move", source)])
}

pub fn extract_struct_name(message: &str) -> Option<String> {
    let (_, rest) = message.split_once("Struct `")?;
    let (name, _) = rest.split_once('`')?;
    Some(name.to_string())
}
