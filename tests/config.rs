use move_clippy::LintEngine;
use move_clippy::config;
use move_clippy::level::LintLevel;
use move_clippy::lint::{LintRegistry, LintSettings};
use std::path::Path;

#[test]
fn config_can_promote_lint_to_error() {
    let cfg_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/config/error_level/move-clippy.toml");
    let cfg = config::load_config_file(&cfg_path).expect("config should load");

    let empty: Vec<String> = Vec::new();
    let preview = cfg.lints.preview;
    let registry =
        LintRegistry::default_rules_filtered(&empty, &empty, &cfg.lints.disabled, false, preview)
            .expect("registry");
    let settings = LintSettings::default()
        .with_config_levels(cfg.lints.levels)
        .disable(cfg.lints.disabled);
    let engine = LintEngine::new_with_settings(registry, settings);

    let src = include_str!("fixtures/prefer_vector_methods/positive.move");
    let diags = engine.lint_source(src).expect("linting should succeed");

    assert!(
        diags
            .iter()
            .any(|d| d.lint.name == "prefer_vector_methods" && d.level == LintLevel::Error)
    );
}

#[test]
fn config_can_disable_lint() {
    let cfg_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/config/disabled/move-clippy.toml");
    let cfg = config::load_config_file(&cfg_path).expect("config should load");

    let empty: Vec<String> = Vec::new();
    let preview = cfg.lints.preview;
    let registry =
        LintRegistry::default_rules_filtered(&empty, &empty, &cfg.lints.disabled, false, preview)
            .expect("registry");
    let settings = LintSettings::default()
        .with_config_levels(cfg.lints.levels)
        .disable(cfg.lints.disabled);
    let engine = LintEngine::new_with_settings(registry, settings);

    let src = include_str!("fixtures/prefer_vector_methods/positive.move");
    let diags = engine.lint_source(src).expect("linting should succeed");

    assert!(!diags.iter().any(|d| d.lint.name == "prefer_vector_methods"));
}
