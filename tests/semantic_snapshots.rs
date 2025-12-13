//! Semantic Lint Snapshot Tests
//!
//! These tests validate the semantic security lints against fixture files
//! to ensure they correctly identify bugs and don't fire false positives.

use insta::assert_snapshot;
use move_clippy::LintEngine;
use move_clippy::lint::{LintRegistry, LintSettings};
use std::path::PathBuf;

/// Helper to lint a semantic fixture file and return formatted diagnostics
fn lint_semantic_fixture(fixture_name: &str) -> String {
    let mut fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fixture_path.push("tests/fixtures/semantic");
    fixture_path.push(fixture_name);

    if !fixture_path.exists() {
        return format!("ERROR: Fixture file not found: {:?}", fixture_path);
    }

    let source = std::fs::read_to_string(&fixture_path)
        .unwrap_or_else(|e| panic!("Failed to read fixture {}: {}", fixture_name, e));

    // Enable preview lints for semantic tests
    let settings = LintSettings::default();

    let registry = LintRegistry::default_rules_filtered(
        &[],   // only
        &[],   // skip
        &[],   // disabled
        false, // full_mode
        true,  // preview (enable preview lints)
    )
    .expect("Failed to create registry");

    let engine = LintEngine::new_with_settings(registry, settings);

    match engine.lint_source(&source) {
        Ok(diags) => {
            if diags.is_empty() {
                "No findings.".to_string()
            } else {
                diags
                    .iter()
                    .map(|d| {
                        format!(
                            "{}:{} - {}: {}",
                            d.span.start.row, d.span.start.column, d.lint.name, d.message
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
        Err(e) => format!("ERROR: {}", e),
    }
}

// ============================================================================
// Oracle Zero Price Tests
// ============================================================================

#[test]
fn oracle_zero_price_positive() {
    let output = lint_semantic_fixture("oracle_zero_price_positive.move");
    assert_snapshot!(output);
}

#[test]
fn oracle_zero_price_negative() {
    let output = lint_semantic_fixture("oracle_zero_price_negative.move");
    assert_snapshot!(output);
}

// ============================================================================
// Unused Return Value Tests
// ============================================================================

#[test]
fn unused_return_value_positive() {
    let output = lint_semantic_fixture("unused_return_value_positive.move");
    assert_snapshot!(output);
}

#[test]
fn unused_return_value_negative() {
    let output = lint_semantic_fixture("unused_return_value_negative.move");
    assert_snapshot!(output);
}

// ============================================================================
// Missing Access Control Tests
// ============================================================================

#[test]
fn missing_access_control_positive() {
    let output = lint_semantic_fixture("missing_access_control_positive.move");
    assert_snapshot!(output);
}

#[test]
fn missing_access_control_negative() {
    let output = lint_semantic_fixture("missing_access_control_negative.move");
    assert_snapshot!(output);
}
