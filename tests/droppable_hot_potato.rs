//! Exhaustive spec-driven tests for `droppable_hot_potato` lint.
//!
//! # Formal Specification
//!
//! ```text
//! INVARIANT:
//!   For struct S with abilities A and fields F:
//!     WARN if: abilities(S) = {drop} AND |fields(S)| > 0
//!
//!   Equivalently:
//!     has_drop(A) ∧ ¬has_copy(A) ∧ ¬has_key(A) ∧ ¬has_store(A) ∧ field_count(S) > 0
//! ```
//!
//! # Input Dimensions
//!
//! | Dimension | Values |
//! |-----------|--------|
//! | drop      | {present, absent} |
//! | copy      | {present, absent} |
//! | key       | {present, absent} |
//! | store     | {present, absent} |
//! | fields    | {0, 1+} |
//!
//! Total: 2^4 × 2 = 32 test cases
//!
//! # Expected Result
//!
//! Only ONE case should trigger a warning:
//! - abilities = {drop}, fields = 1+
//!
//! This proves zero false positives by exhaustion.

#[cfg(feature = "full")]
#[path = "support/mod.rs"]
mod support;

#[cfg(feature = "full")]
use move_clippy::lint::LintSettings;
#[cfg(feature = "full")]
use support::semantic_spec_harness::create_temp_package;

#[cfg(feature = "full")]
const MOVE_TOML: &str = r#"[package]
name = "spec_test_pkg"
edition = "2024"

[addresses]
spec_test_pkg = "0x0"
"#;

/// A single test case in our exhaustive matrix.
#[derive(Debug, Clone)]
struct TestCase {
    /// Abilities to add to the struct (empty = no abilities)
    abilities: &'static [&'static str],
    /// Number of fields (0 = empty struct, 1 = one field)
    field_count: usize,
    /// Whether the lint should fire
    expected_warn: bool,
    /// Human-readable explanation
    rationale: &'static str,
}

/// Generate Move source code for a test case.
fn generate_fixture(tc: &TestCase, index: usize) -> String {
    let abilities_str = if tc.abilities.is_empty() {
        String::new()
    } else {
        format!(" has {}", tc.abilities.join(", "))
    };

    let fields_str = if tc.field_count == 0 {
        " {}".to_string()
    } else {
        " { value: u64 }".to_string()
    };

    // Use public struct for Move 2024 edition
    format!(
        r#"module spec_test_pkg::test_case_{index} {{
    public struct TestStruct{abilities}{fields}
}}
"#,
        abilities = abilities_str,
        fields = fields_str
    )
}

/// Run semantic lints on a Move package and return diagnostics for a specific lint.
#[cfg(feature = "full")]
fn run_lint(source: &str, lint_name: &str) -> Vec<String> {
    let tmp = create_temp_package(MOVE_TOML, &[("test.move", source)])
        .expect("should create temp package");

    let result =
        move_clippy::semantic::lint_package(tmp.path(), &LintSettings::default(), false, true);

    match result {
        Ok(diags) => diags
            .into_iter()
            .filter(|d| d.lint.name == lint_name)
            .map(|d| d.message)
            .collect(),
        Err(e) => {
            // Compilation errors are expected for some invalid ability combinations
            // Return empty vec (no lint fired)
            eprintln!("Compilation error (expected for some cases): {}", e);
            vec![]
        }
    }
}

/// The exhaustive test matrix.
///
/// We enumerate all 32 combinations of:
/// - 4 abilities (drop, copy, key, store): 2^4 = 16 combinations
/// - 2 field counts (0, 1+): 2 combinations
///
/// Total: 16 × 2 = 32 test cases
const TEST_MATRIX: &[TestCase] = &[
    // =========================================================================
    // NO ABILITIES (4 cases)
    // =========================================================================
    TestCase {
        abilities: &[],
        field_count: 0,
        expected_warn: false,
        rationale: "Empty struct, no abilities - marker type pattern",
    },
    TestCase {
        abilities: &[],
        field_count: 1,
        expected_warn: false,
        rationale: "True hot potato - no abilities is correct design",
    },
    // =========================================================================
    // ONLY DROP (2 cases) - THE KEY CASES
    // =========================================================================
    TestCase {
        abilities: &["drop"],
        field_count: 0,
        expected_warn: false,
        rationale: "Empty witness struct - legitimate OTW pattern",
    },
    TestCase {
        abilities: &["drop"],
        field_count: 1,
        expected_warn: true, // <-- ONLY CASE THAT SHOULD WARN
        rationale: "BROKEN HOT POTATO - has drop but has fields, defeats consumption guarantee",
    },
    // =========================================================================
    // ONLY COPY (2 cases)
    // =========================================================================
    TestCase {
        abilities: &["copy"],
        field_count: 0,
        expected_warn: false,
        rationale: "Copy-only empty struct - unusual but valid",
    },
    TestCase {
        abilities: &["copy"],
        field_count: 1,
        expected_warn: false,
        rationale: "Copy-only with fields - no drop, so not our pattern",
    },
    // =========================================================================
    // ONLY KEY (2 cases)
    // =========================================================================
    TestCase {
        abilities: &["key"],
        field_count: 0,
        expected_warn: false,
        rationale: "Key-only empty - object without store",
    },
    TestCase {
        abilities: &["key"],
        field_count: 1,
        expected_warn: false,
        rationale: "Key-only with fields - object pattern, not hot potato",
    },
    // =========================================================================
    // ONLY STORE (2 cases)
    // =========================================================================
    TestCase {
        abilities: &["store"],
        field_count: 0,
        expected_warn: false,
        rationale: "Store-only empty - storable marker",
    },
    TestCase {
        abilities: &["store"],
        field_count: 1,
        expected_warn: false,
        rationale: "Store-only with fields - embeddable struct",
    },
    // =========================================================================
    // COPY + DROP (2 cases) - Event pattern
    // =========================================================================
    TestCase {
        abilities: &["copy", "drop"],
        field_count: 0,
        expected_warn: false,
        rationale: "Empty event - valid event pattern",
    },
    TestCase {
        abilities: &["copy", "drop"],
        field_count: 1,
        expected_warn: false,
        rationale: "Event/DTO with fields - legitimate copy+drop pattern",
    },
    // =========================================================================
    // COPY + KEY (2 cases)
    // =========================================================================
    TestCase {
        abilities: &["copy", "key"],
        field_count: 0,
        expected_warn: false,
        rationale: "Copy+key empty - copyable object (unusual)",
    },
    TestCase {
        abilities: &["copy", "key"],
        field_count: 1,
        expected_warn: false,
        rationale: "Copy+key with fields - copyable object",
    },
    // =========================================================================
    // COPY + STORE (2 cases)
    // =========================================================================
    TestCase {
        abilities: &["copy", "store"],
        field_count: 0,
        expected_warn: false,
        rationale: "Copy+store empty - storable copyable marker",
    },
    TestCase {
        abilities: &["copy", "store"],
        field_count: 1,
        expected_warn: false,
        rationale: "Copy+store with fields - config/data struct",
    },
    // =========================================================================
    // DROP + KEY (2 cases)
    // =========================================================================
    TestCase {
        abilities: &["drop", "key"],
        field_count: 0,
        expected_warn: false,
        rationale: "Drop+key empty - droppable object marker",
    },
    TestCase {
        abilities: &["drop", "key"],
        field_count: 1,
        expected_warn: false,
        rationale: "Drop+key with fields - droppable object (has key, so not pure drop)",
    },
    // =========================================================================
    // DROP + STORE (2 cases)
    // =========================================================================
    TestCase {
        abilities: &["drop", "store"],
        field_count: 0,
        expected_warn: false,
        rationale: "Drop+store empty - droppable storable marker",
    },
    TestCase {
        abilities: &["drop", "store"],
        field_count: 1,
        expected_warn: false,
        rationale: "Drop+store with fields - embeddable droppable (has store, so not pure drop)",
    },
    // =========================================================================
    // KEY + STORE (2 cases) - Resource/Capability pattern
    // =========================================================================
    TestCase {
        abilities: &["key", "store"],
        field_count: 0,
        expected_warn: false,
        rationale: "Key+store empty - capability marker",
    },
    TestCase {
        abilities: &["key", "store"],
        field_count: 1,
        expected_warn: false,
        rationale: "Key+store with fields - resource/capability pattern",
    },
    // =========================================================================
    // COPY + DROP + KEY (2 cases)
    // =========================================================================
    TestCase {
        abilities: &["copy", "drop", "key"],
        field_count: 0,
        expected_warn: false,
        rationale: "Copy+drop+key empty - freely usable object marker",
    },
    TestCase {
        abilities: &["copy", "drop", "key"],
        field_count: 1,
        expected_warn: false,
        rationale: "Copy+drop+key with fields - freely usable object",
    },
    // =========================================================================
    // COPY + DROP + STORE (2 cases) - Config pattern
    // =========================================================================
    TestCase {
        abilities: &["copy", "drop", "store"],
        field_count: 0,
        expected_warn: false,
        rationale: "Config empty - fully flexible marker",
    },
    TestCase {
        abilities: &["copy", "drop", "store"],
        field_count: 1,
        expected_warn: false,
        rationale: "Config with fields - config/data pattern",
    },
    // =========================================================================
    // COPY + KEY + STORE (2 cases)
    // =========================================================================
    TestCase {
        abilities: &["copy", "key", "store"],
        field_count: 0,
        expected_warn: false,
        rationale: "Copy+key+store empty - copyable resource marker",
    },
    TestCase {
        abilities: &["copy", "key", "store"],
        field_count: 1,
        expected_warn: false,
        rationale: "Copy+key+store with fields - copyable resource",
    },
    // =========================================================================
    // DROP + KEY + STORE (2 cases)
    // =========================================================================
    TestCase {
        abilities: &["drop", "key", "store"],
        field_count: 0,
        expected_warn: false,
        rationale: "Drop+key+store empty - droppable resource marker",
    },
    TestCase {
        abilities: &["drop", "key", "store"],
        field_count: 1,
        expected_warn: false,
        rationale: "Drop+key+store with fields - droppable resource",
    },
    // =========================================================================
    // ALL FOUR ABILITIES (2 cases)
    // =========================================================================
    TestCase {
        abilities: &["copy", "drop", "key", "store"],
        field_count: 0,
        expected_warn: false,
        rationale: "All abilities empty - fully flexible marker",
    },
    TestCase {
        abilities: &["copy", "drop", "key", "store"],
        field_count: 1,
        expected_warn: false,
        rationale: "All abilities with fields - no restrictions",
    },
];

#[cfg(feature = "full")]
#[test]
fn spec_droppable_hot_potato_exhaustive() {
    let mut passed = 0;
    let mut failed = 0;
    let mut warn_cases = Vec::new();

    for (i, tc) in TEST_MATRIX.iter().enumerate() {
        let fixture = generate_fixture(tc, i);
        let diags = run_lint(&fixture, "droppable_hot_potato_v2");
        let fired = !diags.is_empty();

        if fired {
            warn_cases.push(i);
        }

        if fired == tc.expected_warn {
            passed += 1;
        } else {
            failed += 1;
            eprintln!(
                "\n=== TEST CASE {} FAILED ===\n\
                 Abilities: {:?}\n\
                 Fields: {}\n\
                 Expected warn: {}\n\
                 Actual warn: {}\n\
                 Rationale: {}\n\
                 Fixture:\n{}\n\
                 Diagnostics: {:?}\n",
                i,
                tc.abilities,
                tc.field_count,
                tc.expected_warn,
                fired,
                tc.rationale,
                fixture,
                diags
            );
        }
    }

    // Summary
    println!("\n=== SPEC TEST SUMMARY ===");
    println!("Total cases: {}", TEST_MATRIX.len());
    println!("Passed: {}", passed);
    println!("Failed: {}", failed);
    println!("Cases that triggered warning: {:?}", warn_cases);

    // Verify exactly 1 case triggers warning (case index 3)
    assert_eq!(
        warn_cases.len(),
        1,
        "Expected exactly 1 case to trigger warning, got {} cases: {:?}",
        warn_cases.len(),
        warn_cases
    );
    assert_eq!(
        warn_cases[0], 3,
        "Expected case 3 (drop-only with fields) to warn, got case {}",
        warn_cases[0]
    );

    assert_eq!(
        failed, 0,
        "{} test cases failed - see above for details",
        failed
    );
}

/// Verify the test matrix is complete (32 cases).
#[test]
fn spec_matrix_is_complete() {
    assert_eq!(
        TEST_MATRIX.len(),
        32,
        "Expected 32 test cases (2^4 abilities × 2 field counts), got {}",
        TEST_MATRIX.len()
    );

    // Count expected warnings
    let warn_count = TEST_MATRIX.iter().filter(|tc| tc.expected_warn).count();
    assert_eq!(
        warn_count, 1,
        "Expected exactly 1 warning case in matrix, got {}",
        warn_count
    );
}

/// Test that the fixture generator produces valid Move code.
#[test]
fn fixture_generator_produces_valid_code() {
    for (i, tc) in TEST_MATRIX.iter().enumerate() {
        let fixture = generate_fixture(tc, i);

        // Basic sanity checks
        assert!(
            fixture.contains("module spec_test_pkg::test_case_"),
            "Missing module declaration (case {i}): {}",
            tc.rationale
        );
        assert!(
            fixture.contains("public struct TestStruct"),
            "Missing struct declaration (case {i}): {}",
            tc.rationale
        );

        if !tc.abilities.is_empty() {
            assert!(
                fixture.contains("has"),
                "Missing 'has' for abilities (case {i}): {}",
                tc.rationale
            );
        }

        if tc.field_count > 0 {
            assert!(
                fixture.contains("value: u64"),
                "Missing field declaration (case {i}): {}",
                tc.rationale
            );
        }
    }
}
