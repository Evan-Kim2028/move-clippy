use move_clippy::create_default_engine;

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
