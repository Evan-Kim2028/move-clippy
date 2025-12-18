#![cfg(feature = "full")]

use move_clippy::absint_lints;
use move_clippy::lint::{LintSettings, RuleGroup};
use move_clippy::semantic::lint_package;
use std::path::PathBuf;

#[test]
fn phase2_visitors_disabled_when_preview_false() {
    assert!(
        absint_lints::create_visitors(false, false).is_empty(),
        "Phase II visitors must not run unless preview is enabled"
    );
    assert!(
        !absint_lints::create_visitors(true, false).is_empty(),
        "Phase II visitors should be constructible when preview is enabled"
    );
}

#[test]
fn full_mode_filters_preview_diagnostics_when_preview_false() {
    let manifest_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    // Phase II fixture: emits Preview-group diagnostics when preview=true.
    let unchecked_div_pkg = manifest_root.join("tests/fixtures/phase2/unchecked_div_pkg");
    let diags = lint_package(&unchecked_div_pkg, &LintSettings::default(), false, false)
        .expect("lint_package should succeed");
    assert!(
        diags.iter().all(|d| d.lint.group != RuleGroup::Preview),
        "preview=false must filter all Preview-group diagnostics"
    );

    // Note: Phase II preview_gating_pkg fixture would trigger missing_access_control,
    // but that lint was removed (used name-based heuristics).
    // Preview gating is still validated via Phase III cap_leak_pkg.
}

#[test]
fn full_mode_emits_preview_diagnostics_when_preview_true() {
    let manifest_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let unchecked_div_pkg = manifest_root.join("tests/fixtures/phase2/unchecked_div_pkg");
    let diags = lint_package(&unchecked_div_pkg, &LintSettings::default(), true, false)
        .expect("lint_package should succeed");
    assert!(
        diags.iter().any(|d| d.lint.group == RuleGroup::Preview),
        "expected at least one Preview-group diagnostic when preview=true"
    );

    // Note: missing_access_control lint was removed (used name-based heuristics).
    // This test validates preview gating works using transitive_capability_leak instead.
}

#[test]
fn full_mode_emits_experimental_diagnostics_when_experimental_true() {
    let manifest_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let cap_leak_pkg = manifest_root.join("tests/fixtures/phase3/cap_leak_pkg");

    let diags = lint_package(&cap_leak_pkg, &LintSettings::default(), true, false)
        .expect("lint_package should succeed");
    assert!(
        diags
            .iter()
            .all(|d| d.lint.group != RuleGroup::Experimental),
        "experimental=false must filter/skip Experimental-group diagnostics"
    );

    let diags = lint_package(&cap_leak_pkg, &LintSettings::default(), true, true)
        .expect("lint_package should succeed");
    assert!(
        diags
            .iter()
            .any(|d| d.lint.name == "transitive_capability_leak"),
        "expected transitive_capability_leak when experimental=true"
    );
}
