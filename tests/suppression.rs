use move_clippy::create_default_engine;

#[test]
fn module_level_allow_suppresses_lints_inside_functions() {
    let engine = create_default_engine();

    let src = r#"
#![allow(lint::prefer_vector_methods)]
module my_pkg::m;

use std::vector;

public fun demo() {
    let mut v = vector::empty<u64>();
    vector::push_back(&mut v, 1);
}
"#;

    let diags = engine.lint_source(src).expect("linting should succeed");
    assert!(
        !diags.iter().any(|d| d.lint.name == "prefer_vector_methods"),
        "expected module-level allow to suppress prefer_vector_methods, got: {diags:#?}"
    );
}

#[test]
fn without_allow_the_lint_fires() {
    let engine = create_default_engine();

    let src = r#"
module my_pkg::m;

use std::vector;

public fun demo() {
    let mut v = vector::empty<u64>();
    vector::push_back(&mut v, 1);
}
"#;

    let diags = engine.lint_source(src).expect("linting should succeed");
    assert!(
        diags.iter().any(|d| d.lint.name == "prefer_vector_methods"),
        "expected prefer_vector_methods to fire without allow, got: {diags:#?}"
    );
}
