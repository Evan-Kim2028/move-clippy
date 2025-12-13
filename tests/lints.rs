use move_clippy::create_default_engine;
use move_clippy::lint::{resolve_lint_alias, is_lint_alias, all_known_lints_with_aliases};

#[test]
fn modern_module_syntax_flags_legacy_block_form() {
    let engine = create_default_engine();

    let src = r#"
module my_pkg::m {
    public struct A has copy, drop {}
}
"#;

    let diags = engine.lint_source(src).expect("linting should succeed");
    assert!(diags.iter().any(|d| d.lint.name == "modern_module_syntax"));
}

#[test]
fn modern_module_syntax_allows_label_form() {
    let engine = create_default_engine();

    let src = r#"
module my_pkg::m;

public struct A has copy, drop {}
"#;

    let diags = engine.lint_source(src).expect("linting should succeed");
    assert!(!diags.iter().any(|d| d.lint.name == "modern_module_syntax"));
}

#[test]
fn redundant_self_import_flags_single_self_brace_form() {
    let engine = create_default_engine();

    let src = r#"
module my_pkg::m;

use my_pkg::m::{Self};
"#;

    let diags = engine.lint_source(src).expect("linting should succeed");
    assert!(diags.iter().any(|d| d.lint.name == "redundant_self_import"));
}

#[test]
fn redundant_self_import_does_not_flag_multi_item_brace_form() {
    let engine = create_default_engine();

    let src = r#"
module my_pkg::m;

use my_pkg::m::{Self, Foo};
"#;

    let diags = engine.lint_source(src).expect("linting should succeed");
    assert!(!diags.iter().any(|d| d.lint.name == "redundant_self_import"));
}

// ============================================================================
// Lint Alias Tests
// ============================================================================

#[test]
fn resolve_lint_alias_returns_canonical_for_known_lint() {
    // Known canonical names should return unchanged
    assert_eq!(resolve_lint_alias("modern_module_syntax"), "modern_module_syntax");
    assert_eq!(resolve_lint_alias("prefer_to_string"), "prefer_to_string");
    assert_eq!(resolve_lint_alias("constant_naming"), "constant_naming");
}

#[test]
fn resolve_lint_alias_returns_input_for_unknown() {
    // Unknown names should return unchanged
    assert_eq!(resolve_lint_alias("unknown_lint"), "unknown_lint");
    assert_eq!(resolve_lint_alias("not_a_lint"), "not_a_lint");
}

#[test]
fn is_lint_alias_false_for_canonical_names() {
    // Canonical names are not aliases
    assert!(!is_lint_alias("modern_module_syntax"));
    assert!(!is_lint_alias("prefer_to_string"));
}

#[test]
fn all_known_lints_with_aliases_includes_canonical() {
    let known = all_known_lints_with_aliases();
    assert!(known.contains("modern_module_syntax"));
    assert!(known.contains("prefer_to_string"));
    assert!(known.contains("constant_naming"));
    assert!(known.contains("capability_naming")); // semantic lint
}
