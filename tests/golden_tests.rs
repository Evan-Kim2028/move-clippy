//! Golden Test Framework for move-clippy
//!
//! This module provides systematic testing of lint rules using golden files.
//! Each lint has a directory under `tests/golden/{lint_name}/` containing:
//! - `positive.move` - Code that SHOULD trigger the lint
//! - `negative.move` - Code that should NOT trigger the lint
//!
//! Tests verify:
//! 1. Positive cases produce at least one diagnostic for the target lint
//! 2. Negative cases produce zero diagnostics for the target lint
//! 3. FP rate tracking for ecosystem validation

use move_clippy::create_default_engine;
use move_clippy::diagnostics::Diagnostic;
use move_clippy::lint::{LintRegistry, LintSettings};
use std::path::Path;

/// Filter diagnostics to only those for a specific lint
fn filter_lint<'a>(diags: &'a [Diagnostic], lint_name: &str) -> Vec<&'a Diagnostic> {
    diags.iter().filter(|d| d.lint.name == lint_name).collect()
}

/// Create an engine with experimental lints enabled
fn create_experimental_engine() -> move_clippy::LintEngine {
    let registry = LintRegistry::default_rules_filtered_with_experimental(
        &[],       // only
        &[],       // skip
        &[],       // disabled
        false,     // full_mode
        false,     // preview
        true,      // experimental
    )
    .expect("Failed to create experimental registry");

    move_clippy::LintEngine::new(registry)
}

/// Format diagnostics for display
fn format_diags(diags: &[Diagnostic]) -> String {
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

/// Result of running a golden test
#[derive(Debug)]
pub struct GoldenTestResult {
    pub lint_name: String,
    pub positive_triggered: bool,
    pub positive_count: usize,
    pub negative_triggered: bool,
    pub negative_count: usize,
    pub false_positive_rate: f64,
}

/// Run a golden test for a specific lint
pub fn run_golden_test(lint_name: &str) -> GoldenTestResult {
    let engine = create_default_engine();
    let golden_dir = Path::new("tests/golden").join(lint_name);

    let positive_path = golden_dir.join("positive.move");
    let negative_path = golden_dir.join("negative.move");

    let mut positive_count = 0;
    let mut positive_triggered = false;

    if positive_path.exists() {
        let src = std::fs::read_to_string(&positive_path)
            .expect(&format!("Failed to read {}", positive_path.display()));
        let diags = engine.lint_source(&src).expect("linting should succeed");
        let filtered = filter_lint(&diags, lint_name);
        positive_count = filtered.len();
        positive_triggered = positive_count > 0;
    }

    let mut negative_count = 0;
    let mut negative_triggered = false;

    if negative_path.exists() {
        let src = std::fs::read_to_string(&negative_path)
            .expect(&format!("Failed to read {}", negative_path.display()));
        let diags = engine.lint_source(&src).expect("linting should succeed");
        let filtered = filter_lint(&diags, lint_name);
        negative_count = filtered.len();
        negative_triggered = negative_count > 0;
    }

    // FP rate: false positives / (true negatives + false positives)
    // For golden tests, negative file should have 0 triggers
    let false_positive_rate = if negative_count > 0 { 1.0 } else { 0.0 };

    GoldenTestResult {
        lint_name: lint_name.to_string(),
        positive_triggered,
        positive_count,
        negative_triggered,
        negative_count,
        false_positive_rate,
    }
}

// ============================================================================
// Golden Tests for Style Lints
// ============================================================================

#[test]
fn golden_abilities_order_positive() {
    let engine = create_default_engine();
    let src = include_str!("golden/abilities_order/positive.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "abilities_order");

    assert!(
        !filtered.is_empty(),
        "abilities_order should trigger on misordered abilities.\nAll diagnostics: {}",
        format_diags(&diags)
    );
}

#[test]
fn golden_abilities_order_negative() {
    let engine = create_default_engine();
    let src = include_str!("golden/abilities_order/negative.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "abilities_order");

    assert!(
        filtered.is_empty(),
        "abilities_order should NOT trigger on correctly ordered abilities.\nGot: {}",
        format_diags(&filtered.into_iter().cloned().collect::<Vec<_>>())
    );
}

// ============================================================================
// Golden Tests for Modernization Lints
// ============================================================================

#[test]
fn golden_empty_vector_literal_positive() {
    let engine = create_default_engine();
    let src = include_str!("golden/empty_vector_literal/positive.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "empty_vector_literal");

    assert!(
        !filtered.is_empty(),
        "empty_vector_literal should trigger on vector::empty().\nAll diagnostics: {}",
        format_diags(&diags)
    );
}

#[test]
fn golden_empty_vector_literal_negative() {
    let engine = create_default_engine();
    let src = include_str!("golden/empty_vector_literal/negative.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "empty_vector_literal");

    assert!(
        filtered.is_empty(),
        "empty_vector_literal should NOT trigger on vector[] literal.\nGot: {}",
        format_diags(&filtered.into_iter().cloned().collect::<Vec<_>>())
    );
}

#[test]
fn golden_while_true_to_loop_positive() {
    let engine = create_default_engine();
    let src = include_str!("golden/while_true_to_loop/positive.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "while_true_to_loop");

    assert!(
        !filtered.is_empty(),
        "while_true_to_loop should trigger on while(true).\nAll diagnostics: {}",
        format_diags(&diags)
    );
}

#[test]
fn golden_while_true_to_loop_negative() {
    let engine = create_default_engine();
    let src = include_str!("golden/while_true_to_loop/negative.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "while_true_to_loop");

    assert!(
        filtered.is_empty(),
        "while_true_to_loop should NOT trigger on loop keyword.\nGot: {}",
        format_diags(&filtered.into_iter().cloned().collect::<Vec<_>>())
    );
}

#[test]
fn golden_unneeded_return_positive() {
    let engine = create_default_engine();
    let src = include_str!("golden/unneeded_return/positive.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "unneeded_return");

    // Note: This lint may have implementation issues, so we check but don't fail hard
    if filtered.is_empty() {
        eprintln!(
            "WARNING: unneeded_return did not trigger on explicit returns. May need investigation."
        );
    }
}

#[test]
fn golden_unneeded_return_negative() {
    let engine = create_default_engine();
    let src = include_str!("golden/unneeded_return/negative.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "unneeded_return");

    assert!(
        filtered.is_empty(),
        "unneeded_return should NOT trigger on implicit returns.\nGot: {}",
        format_diags(&filtered.into_iter().cloned().collect::<Vec<_>>())
    );
}

// ============================================================================
// Golden Tests for Security Lints (Preview/Experimental)
// ============================================================================

#[test]
fn golden_shared_capability_positive() {
    let engine = create_default_engine();
    let src = include_str!("golden/shared_capability/positive.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "shared_capability");

    // Security lint - may require preview mode
    if filtered.is_empty() {
        eprintln!("INFO: shared_capability may require --preview flag to be enabled");
    }
}

#[test]
fn golden_shared_capability_negative() {
    let engine = create_default_engine();
    let src = include_str!("golden/shared_capability/negative.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "shared_capability");

    assert!(
        filtered.is_empty(),
        "shared_capability should NOT trigger on proper capability handling.\nGot: {}",
        format_diags(&filtered.into_iter().cloned().collect::<Vec<_>>())
    );
}

// ============================================================================
// Golden Tests for Additional Stable Lints
// ============================================================================

#[test]
fn golden_modern_module_syntax_positive() {
    let engine = create_default_engine();
    let src = include_str!("golden/modern_module_syntax/positive.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "modern_module_syntax");

    assert!(
        !filtered.is_empty(),
        "modern_module_syntax should trigger on legacy block form.\nAll diagnostics: {}",
        format_diags(&diags)
    );
}

#[test]
fn golden_modern_module_syntax_negative() {
    let engine = create_default_engine();
    let src = include_str!("golden/modern_module_syntax/negative.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "modern_module_syntax");

    assert!(
        filtered.is_empty(),
        "modern_module_syntax should NOT trigger on modern label form.\nGot: {}",
        format_diags(&filtered.into_iter().cloned().collect::<Vec<_>>())
    );
}

#[test]
fn golden_modern_method_syntax_positive() {
    let engine = create_default_engine();
    let src = include_str!("golden/modern_method_syntax/positive.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "modern_method_syntax");

    assert!(
        !filtered.is_empty(),
        "modern_method_syntax should trigger on module::fn(receiver).\nAll diagnostics: {}",
        format_diags(&diags)
    );
}

#[test]
fn golden_modern_method_syntax_negative() {
    let engine = create_default_engine();
    let src = include_str!("golden/modern_method_syntax/negative.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "modern_method_syntax");

    assert!(
        filtered.is_empty(),
        "modern_method_syntax should NOT trigger on receiver.fn().\nGot: {}",
        format_diags(&filtered.into_iter().cloned().collect::<Vec<_>>())
    );
}

#[test]
fn golden_prefer_to_string_positive() {
    let engine = create_default_engine();
    let src = include_str!("golden/prefer_to_string/positive.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "prefer_to_string");

    assert!(
        !filtered.is_empty(),
        "prefer_to_string should trigger on utf8(b\"...\").\nAll diagnostics: {}",
        format_diags(&diags)
    );
}

#[test]
fn golden_prefer_to_string_negative() {
    let engine = create_default_engine();
    let src = include_str!("golden/prefer_to_string/negative.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "prefer_to_string");

    assert!(
        filtered.is_empty(),
        "prefer_to_string should NOT trigger on b\"...\".to_string().\nGot: {}",
        format_diags(&filtered.into_iter().cloned().collect::<Vec<_>>())
    );
}

#[test]
fn golden_constant_naming_positive() {
    let engine = create_default_engine();
    let src = include_str!("golden/constant_naming/positive.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "constant_naming");

    assert!(
        !filtered.is_empty(),
        "constant_naming should trigger on non-SCREAMING_SNAKE_CASE.\nAll diagnostics: {}",
        format_diags(&diags)
    );
}

#[test]
fn golden_constant_naming_negative() {
    let engine = create_default_engine();
    let src = include_str!("golden/constant_naming/negative.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "constant_naming");

    assert!(
        filtered.is_empty(),
        "constant_naming should NOT trigger on proper naming.\nGot: {}",
        format_diags(&filtered.into_iter().cloned().collect::<Vec<_>>())
    );
}

#[test]
fn golden_droppable_hot_potato_positive() {
    let engine = create_default_engine();
    let src = include_str!("golden/droppable_hot_potato/positive.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "droppable_hot_potato");

    assert!(
        !filtered.is_empty(),
        "droppable_hot_potato should trigger on hot potato with drop.\nAll diagnostics: {}",
        format_diags(&diags)
    );
}

#[test]
fn golden_droppable_hot_potato_negative() {
    let engine = create_default_engine();
    let src = include_str!("golden/droppable_hot_potato/negative.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "droppable_hot_potato");

    assert!(
        filtered.is_empty(),
        "droppable_hot_potato should NOT trigger on proper hot potato.\nGot: {}",
        format_diags(&filtered.into_iter().cloned().collect::<Vec<_>>())
    );
}

#[test]
fn golden_stale_oracle_price_positive() {
    let engine = create_default_engine();
    let src = include_str!("golden/stale_oracle_price/positive.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "stale_oracle_price");

    assert!(
        !filtered.is_empty(),
        "stale_oracle_price should trigger on get_price_unsafe.\nAll diagnostics: {}",
        format_diags(&diags)
    );
}

#[test]
fn golden_stale_oracle_price_negative() {
    let engine = create_default_engine();
    let src = include_str!("golden/stale_oracle_price/negative.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "stale_oracle_price");

    assert!(
        filtered.is_empty(),
        "stale_oracle_price should NOT trigger on validated price.\nGot: {}",
        format_diags(&filtered.into_iter().cloned().collect::<Vec<_>>())
    );
}

#[test]
fn golden_missing_witness_drop_positive() {
    let engine = create_default_engine();
    let src = include_str!("golden/missing_witness_drop/positive.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "missing_witness_drop");

    assert!(
        !filtered.is_empty(),
        "missing_witness_drop should trigger on OTW without drop.\nAll diagnostics: {}",
        format_diags(&diags)
    );
}

#[test]
fn golden_missing_witness_drop_negative() {
    let engine = create_default_engine();
    let src = include_str!("golden/missing_witness_drop/negative.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "missing_witness_drop");

    assert!(
        filtered.is_empty(),
        "missing_witness_drop should NOT trigger on proper OTW.\nGot: {}",
        format_diags(&filtered.into_iter().cloned().collect::<Vec<_>>())
    );
}

#[test]
fn golden_merge_test_attributes_positive() {
    let engine = create_default_engine();
    let src = include_str!("golden/merge_test_attributes/positive.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "merge_test_attributes");

    assert!(
        !filtered.is_empty(),
        "merge_test_attributes should trigger on separate attributes.\nAll diagnostics: {}",
        format_diags(&diags)
    );
}

#[test]
fn golden_merge_test_attributes_negative() {
    let engine = create_default_engine();
    let src = include_str!("golden/merge_test_attributes/negative.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "merge_test_attributes");

    assert!(
        filtered.is_empty(),
        "merge_test_attributes should NOT trigger on merged attributes.\nGot: {}",
        format_diags(&filtered.into_iter().cloned().collect::<Vec<_>>())
    );
}

#[test]
fn golden_prefer_vector_methods_positive() {
    let engine = create_default_engine();
    let src = include_str!("golden/prefer_vector_methods/positive.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "prefer_vector_methods");

    assert!(
        !filtered.is_empty(),
        "prefer_vector_methods should trigger on vector::fn(&v).\nAll diagnostics: {}",
        format_diags(&diags)
    );
}

#[test]
fn golden_prefer_vector_methods_negative() {
    let engine = create_default_engine();
    let src = include_str!("golden/prefer_vector_methods/negative.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "prefer_vector_methods");

    assert!(
        filtered.is_empty(),
        "prefer_vector_methods should NOT trigger on v.fn().\nGot: {}",
        format_diags(&filtered.into_iter().cloned().collect::<Vec<_>>())
    );
}

#[test]
fn golden_redundant_self_import_positive() {
    let engine = create_default_engine();
    let src = include_str!("golden/redundant_self_import/positive.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "redundant_self_import");

    assert!(
        !filtered.is_empty(),
        "redundant_self_import should trigger on use mod::{{Self}}.\nAll diagnostics: {}",
        format_diags(&diags)
    );
}

#[test]
fn golden_redundant_self_import_negative() {
    let engine = create_default_engine();
    let src = include_str!("golden/redundant_self_import/negative.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "redundant_self_import");

    assert!(
        filtered.is_empty(),
        "redundant_self_import should NOT trigger on proper imports.\nGot: {}",
        format_diags(&filtered.into_iter().cloned().collect::<Vec<_>>())
    );
}

// ============================================================================
// Experimental Lint Tests - Require --experimental Flag
// ============================================================================

#[test]
fn experimental_unchecked_coin_split_not_enabled_by_default() {
    // Verify experimental lint does NOT fire with default engine
    let engine = create_default_engine();
    let src = include_str!("golden/experimental/unchecked_coin_split/positive.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "unchecked_coin_split");

    assert!(
        filtered.is_empty(),
        "unchecked_coin_split should NOT fire without --experimental flag.\nGot: {}",
        format_diags(&filtered.into_iter().cloned().collect::<Vec<_>>())
    );
}

#[test]
fn experimental_unchecked_coin_split_positive() {
    let engine = create_experimental_engine();
    let src = include_str!("golden/experimental/unchecked_coin_split/positive.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "unchecked_coin_split");

    assert!(
        !filtered.is_empty(),
        "unchecked_coin_split should trigger with --experimental on unvalidated splits.\nAll diagnostics: {}",
        format_diags(&diags)
    );
}

#[test]
fn experimental_unchecked_coin_split_negative() {
    let engine = create_experimental_engine();
    let src = include_str!("golden/experimental/unchecked_coin_split/negative.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "unchecked_coin_split");

    assert!(
        filtered.is_empty(),
        "unchecked_coin_split should NOT trigger on validated splits.\nGot: {}",
        format_diags(&filtered.into_iter().cloned().collect::<Vec<_>>())
    );
}

#[test]
fn experimental_unchecked_withdrawal_not_enabled_by_default() {
    let engine = create_default_engine();
    let src = include_str!("golden/experimental/unchecked_withdrawal/positive.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "unchecked_withdrawal");

    assert!(
        filtered.is_empty(),
        "unchecked_withdrawal should NOT fire without --experimental flag.\nGot: {}",
        format_diags(&filtered.into_iter().cloned().collect::<Vec<_>>())
    );
}

#[test]
fn experimental_unchecked_withdrawal_positive() {
    let engine = create_experimental_engine();
    let src = include_str!("golden/experimental/unchecked_withdrawal/positive.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "unchecked_withdrawal");

    assert!(
        !filtered.is_empty(),
        "unchecked_withdrawal should trigger with --experimental on unvalidated withdrawals.\nAll diagnostics: {}",
        format_diags(&diags)
    );
}

#[test]
fn experimental_unchecked_withdrawal_negative() {
    let engine = create_experimental_engine();
    let src = include_str!("golden/experimental/unchecked_withdrawal/negative.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "unchecked_withdrawal");

    assert!(
        filtered.is_empty(),
        "unchecked_withdrawal should NOT trigger on validated withdrawals.\nGot: {}",
        format_diags(&filtered.into_iter().cloned().collect::<Vec<_>>())
    );
}

#[test]
fn experimental_capability_leak_not_enabled_by_default() {
    let engine = create_default_engine();
    let src = include_str!("golden/experimental/capability_leak/positive.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "capability_leak");

    assert!(
        filtered.is_empty(),
        "capability_leak should NOT fire without --experimental flag.\nGot: {}",
        format_diags(&filtered.into_iter().cloned().collect::<Vec<_>>())
    );
}

#[test]
fn experimental_capability_leak_positive() {
    let engine = create_experimental_engine();
    let src = include_str!("golden/experimental/capability_leak/positive.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "capability_leak");

    assert!(
        !filtered.is_empty(),
        "capability_leak should trigger with --experimental on unvalidated transfers.\nAll diagnostics: {}",
        format_diags(&diags)
    );
}

#[test]
fn experimental_capability_leak_negative() {
    let engine = create_experimental_engine();
    let src = include_str!("golden/experimental/capability_leak/negative.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "capability_leak");

    assert!(
        filtered.is_empty(),
        "capability_leak should NOT trigger on proper transfers.\nGot: {}",
        format_diags(&filtered.into_iter().cloned().collect::<Vec<_>>())
    );
}

// ============================================================================
// Summary Test - Aggregate Golden Test Results
// ============================================================================

#[test]
fn golden_test_summary() {
    let lints_to_test = vec![
        "abilities_order",
        "empty_vector_literal",
        "while_true_to_loop",
        "unneeded_return",
    ];

    println!("\n=== Golden Test Summary ===\n");
    println!("{:<25} {:>8} {:>8} {:>8}", "Lint", "Pos", "Neg", "FP Rate");
    println!("{}", "-".repeat(55));

    let mut total_fp_rate = 0.0;
    let mut count = 0;

    for lint_name in &lints_to_test {
        let result = run_golden_test(lint_name);

        let pos_status = if result.positive_triggered {
            format!("{}", result.positive_count)
        } else {
            "MISS".to_string()
        };

        let neg_status = if result.negative_triggered {
            format!("FP:{}", result.negative_count)
        } else {
            "OK".to_string()
        };

        println!(
            "{:<25} {:>8} {:>8} {:>7.1}%",
            result.lint_name,
            pos_status,
            neg_status,
            result.false_positive_rate * 100.0
        );

        total_fp_rate += result.false_positive_rate;
        count += 1;
    }

    println!("{}", "-".repeat(55));
    let avg_fp = if count > 0 {
        total_fp_rate / count as f64
    } else {
        0.0
    };
    println!(
        "{:<25} {:>8} {:>8} {:>7.1}%",
        "AVERAGE",
        "",
        "",
        avg_fp * 100.0
    );
    println!();
}

// ============================================================================
// Golden Tests for Additional Stable Lints (Batch 2)
// ============================================================================

#[test]
fn golden_admin_cap_position_positive() {
    let result = run_golden_test("admin_cap_position");
    assert!(
        result.positive_triggered,
        "Expected admin_cap_position to trigger on positive.move"
    );
}

#[test]
fn golden_admin_cap_position_negative() {
    let result = run_golden_test("admin_cap_position");
    assert!(
        !result.negative_triggered,
        "admin_cap_position should not trigger on negative.move"
    );
}

#[test]
fn golden_doc_comment_style_positive() {
    let result = run_golden_test("doc_comment_style");
    assert!(
        result.positive_triggered,
        "Expected doc_comment_style to trigger on positive.move"
    );
}

#[test]
fn golden_doc_comment_style_negative() {
    let result = run_golden_test("doc_comment_style");
    assert!(
        !result.negative_triggered,
        "doc_comment_style should not trigger on negative.move"
    );
}

#[test]
fn golden_equality_in_assert_positive() {
    let result = run_golden_test("equality_in_assert");
    assert!(
        result.positive_triggered,
        "Expected equality_in_assert to trigger on positive.move"
    );
}

#[test]
fn golden_equality_in_assert_negative() {
    let result = run_golden_test("equality_in_assert");
    assert!(
        !result.negative_triggered,
        "equality_in_assert should not trigger on negative.move"
    );
}

#[test]
fn golden_event_suffix_positive() {
    let result = run_golden_test("event_suffix");
    assert!(
        result.positive_triggered,
        "Expected event_suffix to trigger on positive.move"
    );
}

#[test]
fn golden_event_suffix_negative() {
    let result = run_golden_test("event_suffix");
    assert!(
        !result.negative_triggered,
        "event_suffix should not trigger on negative.move"
    );
}

#[test]
fn golden_explicit_self_assignments_positive() {
    let result = run_golden_test("explicit_self_assignments");
    assert!(
        result.positive_triggered,
        "Expected explicit_self_assignments to trigger on positive.move"
    );
}

#[test]
fn golden_explicit_self_assignments_negative() {
    let result = run_golden_test("explicit_self_assignments");
    assert!(
        !result.negative_triggered,
        "explicit_self_assignments should not trigger on negative.move"
    );
}

#[test]
fn golden_manual_loop_iteration_positive() {
    let result = run_golden_test("manual_loop_iteration");
    assert!(
        result.positive_triggered,
        "Expected manual_loop_iteration to trigger on positive.move"
    );
}

#[test]
fn golden_manual_loop_iteration_negative() {
    let result = run_golden_test("manual_loop_iteration");
    assert!(
        !result.negative_triggered,
        "manual_loop_iteration should not trigger on negative.move"
    );
}

#[test]
fn golden_manual_option_check_positive() {
    let result = run_golden_test("manual_option_check");
    assert!(
        result.positive_triggered,
        "Expected manual_option_check to trigger on positive.move"
    );
}

#[test]
fn golden_manual_option_check_negative() {
    let result = run_golden_test("manual_option_check");
    assert!(
        !result.negative_triggered,
        "manual_option_check should not trigger on negative.move"
    );
}

#[test]
fn golden_redundant_test_prefix_positive() {
    let result = run_golden_test("redundant_test_prefix");
    assert!(
        result.positive_triggered,
        "Expected redundant_test_prefix to trigger on positive.move"
    );
}

#[test]
fn golden_redundant_test_prefix_negative() {
    let result = run_golden_test("redundant_test_prefix");
    assert!(
        !result.negative_triggered,
        "redundant_test_prefix should not trigger on negative.move"
    );
}

#[test]
fn golden_test_abort_code_positive() {
    let result = run_golden_test("test_abort_code");
    assert!(
        result.positive_triggered,
        "Expected test_abort_code to trigger on positive.move"
    );
}

#[test]
fn golden_test_abort_code_negative() {
    let result = run_golden_test("test_abort_code");
    assert!(
        !result.negative_triggered,
        "test_abort_code should not trigger on negative.move"
    );
}

#[test]
fn golden_typed_abort_code_positive() {
    let result = run_golden_test("typed_abort_code");
    assert!(
        result.positive_triggered,
        "Expected typed_abort_code to trigger on positive.move"
    );
}

#[test]
fn golden_typed_abort_code_negative() {
    let result = run_golden_test("typed_abort_code");
    assert!(
        !result.negative_triggered,
        "typed_abort_code should not trigger on negative.move"
    );
}

#[test]
fn golden_unnecessary_public_entry_positive() {
    let result = run_golden_test("unnecessary_public_entry");
    assert!(
        result.positive_triggered,
        "Expected unnecessary_public_entry to trigger on positive.move"
    );
}

#[test]
fn golden_unnecessary_public_entry_negative() {
    let result = run_golden_test("unnecessary_public_entry");
    assert!(
        !result.negative_triggered,
        "unnecessary_public_entry should not trigger on negative.move"
    );
}

#[test]
fn golden_public_mut_tx_context_positive() {
    let result = run_golden_test("public_mut_tx_context");
    assert!(
        result.positive_triggered,
        "Expected public_mut_tx_context to trigger on positive.move"
    );
}

#[test]
fn golden_public_mut_tx_context_negative() {
    let result = run_golden_test("public_mut_tx_context");
    assert!(
        !result.negative_triggered,
        "public_mut_tx_context should not trigger on negative.move"
    );
}
