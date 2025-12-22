//! Semantic Package Snapshot Tests (compiler-based)
//!
//! These tests exercise the full semantic pipeline via `semantic::lint_package`,
//! including typing-based lints and (when `preview=true`) CFG/AbsInt visitors.
//!
//! Run with:
//! - `cargo test --features full --test semantic_package_snapshots`

#![cfg(feature = "full")]

use insta::assert_snapshot;
use move_clippy::diagnostics::Diagnostic;
use move_clippy::lint::LintSettings;
use std::path::{Path, PathBuf};

fn format_semantic_diags(package_root: &Path, diags: &[Diagnostic]) -> String {
    let mut lines: Vec<String> = diags
        .iter()
        .map(|d| {
            let file = d.file.as_deref().unwrap_or("<unknown>");
            let rel = Path::new(file)
                .strip_prefix(package_root)
                .unwrap_or(Path::new(file));
            format!(
                "{}:{}:{}: {}: {}: {}",
                d.lint.name,
                rel.display(),
                d.span.start.row,
                d.span.start.column,
                d.level.as_str(),
                d.message
            )
        })
        .collect();

    lines.sort();
    if lines.is_empty() {
        "No findings.".to_string()
    } else {
        lines.join("\n")
    }
}

fn lint_fixture_package(rel: &str, preview: bool) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    let root = std::fs::canonicalize(&root).expect("fixture package should exist");
    let settings = LintSettings::default();

    let diags = move_clippy::semantic::lint_package(&root, &settings, preview, false)
        .expect("semantic linting should succeed");
    format_semantic_diags(&root, &diags)
}

fn lint_fixture_package_with_experimental(rel: &str, preview: bool, experimental: bool) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    let root = std::fs::canonicalize(&root).expect("fixture package should exist");
    let settings = LintSettings::default();

    let diags = move_clippy::semantic::lint_package(&root, &settings, preview, experimental)
        .expect("semantic linting should succeed");
    format_semantic_diags(&root, &diags)
}

#[test]
fn unchecked_div_pkg_preview() {
    let out = lint_fixture_package_with_experimental(
        "tests/fixtures/phase2/unchecked_div_pkg",
        true,
        true,
    );
    assert_snapshot!(out);
}

#[test]
fn destroy_zero_unchecked_pkg_preview() {
    let out = lint_fixture_package("tests/fixtures/phase2/destroy_zero_unchecked_pkg", true);
    assert_snapshot!(out);
}

#[test]
fn fresh_address_reuse_pkg_preview() {
    let out = lint_fixture_package("tests/fixtures/phase2/fresh_address_reuse_pkg", true);
    assert_snapshot!(out);
}

#[test]
fn share_owned_authority_pkg_preview() {
    let out = lint_fixture_package_with_experimental(
        "tests/fixtures/phase2/share_owned_authority_pkg",
        true,  // preview = true (now Preview tier)
        false, // experimental = false (no longer needs experimental)
    );
    assert_snapshot!(out);
}

// =============================================================================
// New Lint Fixture Tests (Tier 1 and Tier 2)
// =============================================================================

#[test]
fn public_package_single_module_pkg_stable() {
    let out = lint_fixture_package(
        "tests/fixtures/phase2/public_package_single_module_pkg",
        false,
    );
    assert_snapshot!(out);
}

#[test]
fn copyable_capability_pkg_stable() {
    let out = lint_fixture_package("tests/fixtures/phase2/copyable_capability_pkg", false);
    assert_snapshot!(out);
}

#[test]
fn droppable_capability_pkg_stable() {
    let out = lint_fixture_package("tests/fixtures/phase2/droppable_capability_pkg", false);
    assert_snapshot!(out);
}

#[test]
fn non_transferable_fungible_object_pkg_stable() {
    let out = lint_fixture_package(
        "tests/fixtures/phase2/non_transferable_fungible_object_pkg",
        false,
    );
    assert_snapshot!(out);
}

#[test]
fn event_past_tense_pkg_stable() {
    let out = lint_fixture_package("tests/fixtures/phase2/event_past_tense_pkg", false);
    assert_snapshot!(out);
}

#[test]
fn invalid_otw_pkg_stable() {
    let out = lint_fixture_package("tests/fixtures/phase2/invalid_otw_pkg", false);
    assert_snapshot!(out);
}

#[test]
fn witness_antipatterns_pkg_stable() {
    let out = lint_fixture_package("tests/fixtures/phase2/witness_antipatterns_pkg", false);
    assert_snapshot!(out);
}

#[test]
fn capability_antipatterns_pkg_stable() {
    let out = lint_fixture_package("tests/fixtures/phase2/capability_antipatterns_pkg", false);
    assert_snapshot!(out);
}

#[test]
fn public_random_access_v2_pkg_stable() {
    let out = lint_fixture_package("tests/fixtures/phase2/public_random_access_v2_pkg", false);
    assert_snapshot!(out);
}

// =============================================================================
// Phase 4 Preview/Experimental Fixture Tests
// =============================================================================

#[test]
fn shared_capability_object_pkg_preview() {
    let out = lint_fixture_package("tests/fixtures/phase4/shared_capability_object_pkg", true);
    assert_snapshot!(out);
}

#[test]
fn capability_transfer_literal_address_pkg_stable() {
    let out = lint_fixture_package(
        "tests/fixtures/phase4/capability_transfer_literal_address_pkg",
        true,
    );
    assert_snapshot!(out);
}

#[test]
fn mut_key_param_missing_authority_pkg_preview() {
    let out = lint_fixture_package(
        "tests/fixtures/phase4/mut_key_param_missing_authority_pkg",
        true,
    );
    assert_snapshot!(out);
}

#[test]
fn unbounded_iteration_over_param_vector_pkg_preview() {
    let out = lint_fixture_package(
        "tests/fixtures/phase4/unbounded_iteration_over_param_vector_pkg",
        true,
    );
    assert_snapshot!(out);
}

#[test]
fn generic_type_witness_unused_pkg_experimental() {
    let out = lint_fixture_package_with_experimental(
        "tests/fixtures/phase4/generic_type_witness_unused_pkg",
        false,
        true,
    );
    assert_snapshot!(out);
}

#[test]
fn droppable_flash_loan_receipt_pkg_experimental() {
    let out = lint_fixture_package_with_experimental(
        "tests/fixtures/phase4/droppable_flash_loan_receipt_pkg",
        false,
        true,
    );
    assert_snapshot!(out);
}

#[test]
fn receipt_missing_phantom_type_pkg_experimental() {
    let out = lint_fixture_package_with_experimental(
        "tests/fixtures/phase4/receipt_missing_phantom_type_pkg",
        false,
        true,
    );
    assert_snapshot!(out);
}

#[test]
fn copyable_fungible_type_pkg_experimental() {
    let out = lint_fixture_package_with_experimental(
        "tests/fixtures/phase4/copyable_fungible_type_pkg",
        false,
        true,
    );
    assert_snapshot!(out);
}
