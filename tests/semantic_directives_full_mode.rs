#![cfg(feature = "full")]

mod support;

use move_clippy::lint::LintSettings;
use support::semantic_spec_harness::create_temp_package;

const UNFULFILLED_EXPECTATION_NAME: &str = "unfulfilled_expectation";

const MOVE_TOML: &str = r#"
[package]
name = "semantic_directives_pkg"
edition = "2024"

[addresses]
semantic_directives_pkg = "0x0"
"#;

fn lint_pkg(source: &str, settings: LintSettings) -> Vec<move_clippy::diagnostics::Diagnostic> {
    let temp_dir =
        create_temp_package(MOVE_TOML, &[("test.move", source)]).expect("setup should succeed");
    move_clippy::semantic::lint_package(temp_dir.path(), &settings, false, false)
        .expect("lint_package should succeed")
}

#[test]
fn allow_suppresses_semantic_lint() {
    let source = r#"
module semantic_directives_pkg::m {
    #[ext(move_clippy(allow(entry_function_returns_value)))]
    public entry fun returns_value(): u64 { 0 }
}
"#;

    let diags = lint_pkg(source, LintSettings::default());
    assert!(
        diags
            .iter()
            .all(|d| d.lint.name != "entry_function_returns_value"),
        "expected entry_function_returns_value to be suppressed, got: {diags:?}"
    );
}

#[test]
fn deny_promotes_semantic_lint_to_error() {
    let source = r#"
module semantic_directives_pkg::m {
    #[ext(move_clippy(deny(entry_function_returns_value)))]
    public entry fun returns_value(): u64 { 0 }
}
"#;

    let diags = lint_pkg(source, LintSettings::default());
    let Some(diag) = diags
        .iter()
        .find(|d| d.lint.name == "entry_function_returns_value")
    else {
        panic!("expected entry_function_returns_value diagnostic, got: {diags:?}");
    };

    assert_eq!(diag.level.as_str(), "error");
}

#[test]
fn expect_overrides_config_allow_and_does_not_error_unfulfilled() {
    let source = r#"
module semantic_directives_pkg::m {
    #[ext(move_clippy(expect(entry_function_returns_value)))]
    public entry fun returns_value(): u64 { 0 }
}
"#;

    let settings =
        LintSettings::default().disable(vec!["entry_function_returns_value".to_string()]);
    let diags = lint_pkg(source, settings);

    assert!(
        diags
            .iter()
            .any(|d| d.lint.name == "entry_function_returns_value"),
        "expected entry_function_returns_value to still be emitted under #[expect], got: {diags:?}"
    );
    assert!(
        diags
            .iter()
            .all(|d| d.lint.name != UNFULFILLED_EXPECTATION_NAME),
        "did not expect unfulfilled_expectation when the expected lint fired, got: {diags:?}"
    );
}

#[test]
fn unfulfilled_expect_is_reported_in_full_mode() {
    let source = r#"
module semantic_directives_pkg::m {
    #[ext(move_clippy(expect(entry_function_returns_value)))]
    public entry fun returns_unit() { }
}
"#;

    let diags = lint_pkg(source, LintSettings::default());
    assert!(
        diags
            .iter()
            .any(|d| d.lint.name == UNFULFILLED_EXPECTATION_NAME),
        "expected unfulfilled_expectation, got: {diags:?}"
    );
}
