use insta::assert_snapshot;
use move_clippy::create_default_engine;

fn format_diags(diags: &[move_clippy::diagnostics::Diagnostic]) -> String {
    let mut lines: Vec<String> = diags
        .iter()
        .map(|d| {
            format!(
                "{}:{}:{}: {}: {}",
                d.lint.name,
                d.span.start.row,
                d.span.start.column,
                d.level.as_str(),
                d.message
            )
        })
        .collect();
    lines.sort();
    lines.join("\n")
}

#[test]
fn merge_test_attributes_positive() {
    let engine = create_default_engine();
    let src = include_str!("fixtures/merge_test_attributes/positive.move");

    let diags = engine.lint_source(src).expect("linting should succeed");
    assert_snapshot!(format_diags(&diags), @r###"merge_test_attributes:3:1: warning: Merge `#[test]` and `#[expected_failure]` into `#[test, expected_failure]`"###);
}

#[test]
fn merge_test_attributes_negative_merged_already() {
    let engine = create_default_engine();
    let src = include_str!("fixtures/merge_test_attributes/negative_merged.move");

    let diags = engine.lint_source(src).expect("linting should succeed");
    assert_snapshot!(format_diags(&diags), @r###""###);
}

#[test]
fn merge_test_attributes_negative_not_adjacent() {
    let engine = create_default_engine();
    let src = include_str!("fixtures/merge_test_attributes/negative_not_adjacent.move");

    let diags = engine.lint_source(src).expect("linting should succeed");
    assert_snapshot!(format_diags(&diags), @r###""###);
}

#[test]
fn prefer_to_string_positive() {
    let engine = create_default_engine();
    let src = include_str!("fixtures/prefer_to_string/positive.move");

    let diags = engine.lint_source(src).expect("linting should succeed");
    assert_snapshot!(
        format_diags(&diags),
        @r###"prefer_to_string:3:1: warning: Prefer `b"...".to_string()` over `std::string::utf8(b"...")`"###
    );
}

#[test]
fn prefer_to_string_positive_brace() {
    let engine = create_default_engine();
    let src = include_str!("fixtures/prefer_to_string/positive_brace.move");

    let diags = engine.lint_source(src).expect("linting should succeed");
    assert_snapshot!(
        format_diags(&diags),
        @r###"prefer_to_string:3:1: warning: Prefer `b"...".to_string()` over `std::string::utf8(b"...")`"###
    );
}

#[test]
fn prefer_to_string_negative_alias() {
    let engine = create_default_engine();
    let src = include_str!("fixtures/prefer_to_string/negative_alias.move");

    let diags = engine.lint_source(src).expect("linting should succeed");
    assert_snapshot!(format_diags(&diags), @r###""###);
}

#[test]
fn prefer_vector_methods_positive() {
    let engine = create_default_engine();
    let src = include_str!("fixtures/prefer_vector_methods/positive.move");

    let diags = engine.lint_source(src).expect("linting should succeed");
    assert_snapshot!(
        format_diags(&diags),
        @r###"
prefer_vector_methods:5:5: warning: Prefer method syntax: `v.push_back(...)`
prefer_vector_methods:6:14: warning: Prefer method syntax: `v.length()`
"###
    );
}

#[test]
fn prefer_vector_methods_negative_no_refs() {
    let engine = create_default_engine();
    let src = include_str!("fixtures/prefer_vector_methods/negative_no_refs.move");

    let diags = engine.lint_source(src).expect("linting should succeed");
    assert_snapshot!(format_diags(&diags), @r###""###);
}

#[test]
fn prefer_vector_methods_suppressed_allow_attribute() {
    let engine = create_default_engine();
    let src = include_str!("fixtures/prefer_vector_methods/positive_suppressed.move");

    let diags = engine.lint_source(src).expect("linting should succeed");
    assert_snapshot!(format_diags(&diags), @r###""###);
}

#[test]
fn modern_method_syntax_positive() {
    let engine = create_default_engine();
    let src = include_str!("fixtures/modern_method_syntax/positive.move");

    let diags = engine.lint_source(src).expect("linting should succeed");
    assert_snapshot!(
        format_diags(&diags),
        @r###"
modern_method_syntax:4:14: warning: Prefer method syntax: `ctx.sender()`
modern_method_syntax:5:5: warning: Prefer method syntax: `id.delete()`
modern_method_syntax:6:14: warning: Prefer method syntax: `paid.into_balance()`
"###
    );
}

#[test]
fn modern_method_syntax_negative_receiver_not_ident() {
    let engine = create_default_engine();
    let src = include_str!("fixtures/modern_method_syntax/negative_receiver_not_ident.move");

    let diags = engine.lint_source(src).expect("linting should succeed");
    assert_snapshot!(format_diags(&diags), @r###""###);
}

#[test]
fn modern_method_syntax_negative_extra_args() {
    let engine = create_default_engine();
    let src = include_str!("fixtures/modern_method_syntax/negative_extra_args.move");

    let diags = engine.lint_source(src).expect("linting should succeed");
    assert_snapshot!(format_diags(&diags), @r###""###);
}
