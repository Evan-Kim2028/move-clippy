use move_clippy::create_default_engine;
use move_clippy::level::LintLevel;

#[test]
fn module_level_deny_promotes_lint_to_error() {
    let engine = create_default_engine();

    let src = r#"
#![deny(lint::prefer_vector_methods)]
module my_pkg::m;

use std::vector;

public fun demo() {
    let mut v = vector::empty<u64>();
    vector::push_back(&mut v, 1);
}
"#;

    let diags = engine.lint_source(src).expect("linting should succeed");
    let prefer = diags
        .iter()
        .find(|d| d.lint.name == "prefer_vector_methods")
        .expect("expected prefer_vector_methods to fire");

    assert_eq!(
        prefer.level,
        LintLevel::Error,
        "expected #[deny] to promote prefer_vector_methods to error, got: {prefer:#?}"
    );
}

#[test]
fn module_level_expect_is_satisfied_when_lint_fires() {
    let engine = create_default_engine();

    let src = r#"
#![expect(lint::prefer_vector_methods)]
module my_pkg::m;

use std::vector;

public fun demo() {
    let mut v = vector::empty<u64>();
    vector::push_back(&mut v, 1);
}
"#;

    let diags = engine.lint_source(src).expect("linting should succeed");
    assert!(
        !diags
            .iter()
            .any(|d| d.lint.name == "unfulfilled_expectation"),
        "did not expect unfulfilled_expectation when the expected lint fires, got: {diags:#?}"
    );
}

#[test]
fn module_level_expect_emits_error_when_lint_does_not_fire() {
    let engine = create_default_engine();

    let src = r#"
#![expect(lint::prefer_vector_methods)]
module my_pkg::m;

public fun demo() {
    let x = 1;
    let y = x + 1;
    y;
}
"#;

    let diags = engine.lint_source(src).expect("linting should succeed");
    let unmet = diags
        .iter()
        .find(|d| d.lint.name == "unfulfilled_expectation")
        .expect("expected an unfulfilled_expectation diagnostic");

    assert_eq!(unmet.level, LintLevel::Error);
    assert!(
        unmet.message.contains("lint::prefer_vector_methods"),
        "expected message to reference lint::prefer_vector_methods, got: {}",
        unmet.message
    );
}

#[test]
fn item_level_expect_emits_error_when_lint_does_not_fire_in_scope() {
    let engine = create_default_engine();

    let src = r#"
module my_pkg::m;

#[expect(lint::prefer_vector_methods)]
public fun demo() {
    let x = 1;
    x;
}
"#;

    let diags = engine.lint_source(src).expect("linting should succeed");
    let unmet = diags
        .iter()
        .find(|d| d.lint.name == "unfulfilled_expectation")
        .expect("expected an unfulfilled_expectation diagnostic");

    assert_eq!(unmet.level, LintLevel::Error);
    assert!(
        unmet.message.contains("lint::prefer_vector_methods"),
        "expected message to reference lint::prefer_vector_methods, got: {}",
        unmet.message
    );
}

#[test]
fn module_level_allow_style_suppresses_only_style_lints() {
    let engine = create_default_engine();

    let src = r#"
#![allow(lint::style)]
module my_pkg::m;

use std::vector;

public fun demo() {
    let mut v = vector::empty<u64>();
    vector::push_back(&mut v, 1);
    assert!(1 == 1);
}
"#;

    let diags = engine.lint_source(src).expect("linting should succeed");

    assert!(
        diags.iter().any(|d| d.lint.name == "prefer_vector_methods"),
        "expected modernization lint prefer_vector_methods to still fire, got: {diags:#?}"
    );
    assert!(
        !diags.iter().any(|d| d.lint.name == "equality_in_assert"),
        "expected style lint equality_in_assert to be suppressed by lint::style, got: {diags:#?}"
    );
}

#[test]
fn module_level_deny_modernization_promotes_only_modernization_lints() {
    let engine = create_default_engine();

    let src = r#"
#![deny(lint::modernization)]
module my_pkg::m;

use std::vector;

public fun demo() {
    let mut v = vector::empty<u64>();
    vector::push_back(&mut v, 1);
    assert!(1 == 1);
}
"#;

    let diags = engine.lint_source(src).expect("linting should succeed");

    let prefer = diags
        .iter()
        .find(|d| d.lint.name == "prefer_vector_methods")
        .expect("expected prefer_vector_methods to fire");
    assert_eq!(prefer.level, LintLevel::Error);

    let style = diags
        .iter()
        .find(|d| d.lint.name == "equality_in_assert")
        .expect("expected equality_in_assert to fire");
    assert_eq!(style.level, LintLevel::Warn);
}

#[test]
fn module_level_expect_style_is_satisfied_by_any_style_lint() {
    let engine = create_default_engine();

    let src = r#"
#![expect(lint::style)]
module my_pkg::m;

public fun demo() {
    assert!(1 == 1);
}
"#;

    let diags = engine.lint_source(src).expect("linting should succeed");
    assert!(
        diags.iter().any(|d| d.lint.name == "equality_in_assert"),
        "expected a style lint to fire, got: {diags:#?}"
    );
    assert!(
        !diags
            .iter()
            .any(|d| d.lint.name == "unfulfilled_expectation"),
        "did not expect unfulfilled_expectation when lint::style is satisfied, got: {diags:#?}"
    );
}

#[test]
fn module_level_expect_style_errors_when_no_style_lints_fire() {
    let engine = create_default_engine();

    let src = r#"
#![expect(lint::style)]
module my_pkg::m;

use std::vector;

public fun demo() {
    let mut v = vector::empty<u64>();
    vector::push_back(&mut v, 1);
}
"#;

    let diags = engine.lint_source(src).expect("linting should succeed");
    let unmet = diags
        .iter()
        .find(|d| d.lint.name == "unfulfilled_expectation")
        .expect("expected an unfulfilled_expectation diagnostic");
    assert_eq!(unmet.level, LintLevel::Error);
    assert!(
        unmet.message.contains("lint::style"),
        "expected message to reference lint::style, got: {}",
        unmet.message
    );
}
