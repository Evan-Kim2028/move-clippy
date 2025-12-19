//! Exhaustive spec-driven tests for `droppable_capability` lint.
//!
//! # Formal Specification
//!
//! ```text
//! INVARIANT: WARN if has_key(S) ∧ has_store(S) ∧ has_drop(S) ∧ ¬has_copy(S)
//! ```
//!
//! # Security Reference
//!
//! - Mirage Audits (2025): "The Ability Mistakes That Will Drain Your Sui Move Protocol"
//!   <https://www.mirageaudits.com/blog/sui-move-ability-security-mistakes>
//!
//! # Input Dimensions
//!
//! | Dimension | Values |
//! |-----------|--------|
//! | key       | {present, absent} |
//! | store     | {present, absent} |
//! | copy      | {present, absent} |
//! | drop      | {present, absent} |
//!
//! Total: 2^4 = 16 ability combinations
//!
//! # Expected Result
//!
//! Warning fires when: key AND store AND drop are present, but NOT copy.
//! Only 1 case should trigger: key+store+drop (without copy).
//!
//! Note: The case with all 4 abilities (key+store+copy+drop) is handled by
//! `copyable_capability` lint instead, so we exclude `copy` from this lint.

#![cfg(feature = "full")]

mod support;

use move_clippy::lint::LintSettings;
use support::semantic_spec_harness::{create_temp_sui_package, extract_struct_name};

/// A single test case in our exhaustive matrix.
#[derive(Debug, Clone)]
#[allow(dead_code)] // rationale field is for documentation
struct TestCase {
    /// Index for identification
    index: usize,
    /// Abilities to add to the struct
    abilities: &'static [&'static str],
    /// Whether the lint should fire
    expected_warn: bool,
    /// Human-readable explanation
    rationale: &'static str,
}

/// The exhaustive test matrix - all 16 ability combinations.
const TEST_MATRIX: &[TestCase] = &[
    // =========================================================================
    // No abilities (1 case)
    // =========================================================================
    TestCase {
        index: 0,
        abilities: &[],
        expected_warn: false,
        rationale: "No abilities - not a capability",
    },
    // =========================================================================
    // Single ability (4 cases)
    // =========================================================================
    TestCase {
        index: 1,
        abilities: &["key"],
        expected_warn: false,
        rationale: "Key only - not a transferable capability (missing store)",
    },
    TestCase {
        index: 2,
        abilities: &["store"],
        expected_warn: false,
        rationale: "Store only - not an object (missing key)",
    },
    TestCase {
        index: 3,
        abilities: &["copy"],
        expected_warn: false,
        rationale: "Copy only - not a capability",
    },
    TestCase {
        index: 4,
        abilities: &["drop"],
        expected_warn: false,
        rationale: "Drop only - droppable_hot_potato handles this",
    },
    // =========================================================================
    // Two abilities (6 cases)
    // =========================================================================
    TestCase {
        index: 5,
        abilities: &["key", "store"],
        expected_warn: false,
        rationale: "Key+store = PROPER capability pattern (no drop, no copy)",
    },
    TestCase {
        index: 6,
        abilities: &["key", "copy"],
        expected_warn: false,
        rationale: "Key+copy - missing store, not transferable",
    },
    TestCase {
        index: 7,
        abilities: &["key", "drop"],
        expected_warn: false,
        rationale: "Key+drop - missing store, not transferable capability",
    },
    TestCase {
        index: 8,
        abilities: &["store", "copy"],
        expected_warn: false,
        rationale: "Store+copy - missing key, not an object",
    },
    TestCase {
        index: 9,
        abilities: &["store", "drop"],
        expected_warn: false,
        rationale: "Store+drop - missing key, not an object",
    },
    TestCase {
        index: 10,
        abilities: &["copy", "drop"],
        expected_warn: false,
        rationale: "Copy+drop = event/DTO pattern, not a capability",
    },
    // =========================================================================
    // Three abilities (4 cases)
    // =========================================================================
    TestCase {
        index: 11,
        abilities: &["key", "store", "copy"],
        expected_warn: false,
        rationale: "Key+store+copy - handled by copyable_capability lint",
    },
    TestCase {
        index: 12,
        abilities: &["key", "store", "drop"],
        expected_warn: true, // <-- SHOULD WARN
        rationale: "Key+store+drop = DROPPABLE CAPABILITY BUG",
    },
    TestCase {
        index: 13,
        abilities: &["key", "copy", "drop"],
        expected_warn: false,
        rationale: "Key+copy+drop - missing store, not transferable",
    },
    TestCase {
        index: 14,
        abilities: &["store", "copy", "drop"],
        expected_warn: false,
        rationale: "Store+copy+drop - missing key, not an object",
    },
    // =========================================================================
    // All four abilities (1 case)
    // =========================================================================
    TestCase {
        index: 15,
        abilities: &["key", "store", "copy", "drop"],
        expected_warn: false,
        rationale: "All abilities - handled by copyable_capability (copy takes priority)",
    },
];

/// Verify the test matrix is complete (16 cases).
#[test]
fn spec_droppable_capability_matrix_is_complete() {
    assert_eq!(
        TEST_MATRIX.len(),
        16,
        "Expected 16 test cases (2^4 ability combos), got {}",
        TEST_MATRIX.len()
    );

    // Count expected warnings (should be 1: key+store+drop without copy)
    let warn_count = TEST_MATRIX.iter().filter(|tc| tc.expected_warn).count();
    assert_eq!(
        warn_count, 1,
        "Expected 1 warning case (key+store+drop without copy), got {}",
        warn_count
    );

    // Verify the single warn case has exactly key+store+drop (no copy)
    for tc in TEST_MATRIX.iter().filter(|tc| tc.expected_warn) {
        assert!(
            tc.abilities.contains(&"key")
                && tc.abilities.contains(&"store")
                && tc.abilities.contains(&"drop")
                && !tc.abilities.contains(&"copy"),
            "Warning case {} should have key+store+drop but NOT copy: {:?}",
            tc.index,
            tc.abilities
        );
    }
}

/// Verify the formal invariant matches our expectations.
#[test]
fn spec_droppable_capability_invariant() {
    // The invariant: WARN if has_key(S) ∧ has_store(S) ∧ has_drop(S) ∧ ¬has_copy(S)

    for tc in TEST_MATRIX {
        let has_key = tc.abilities.contains(&"key");
        let has_store = tc.abilities.contains(&"store");
        let has_drop = tc.abilities.contains(&"drop");
        let has_copy = tc.abilities.contains(&"copy");

        // The invariant says: warn iff (key AND store AND drop AND NOT copy)
        let should_warn = has_key && has_store && has_drop && !has_copy;

        assert_eq!(
            tc.expected_warn, should_warn,
            "Case {} mismatch: expected_warn={} but invariant says {}. Abilities: {:?}",
            tc.index, tc.expected_warn, should_warn, tc.abilities
        );
    }
}

/// Verify that droppable_capability and copyable_capability are mutually exclusive.
#[test]
fn spec_droppable_vs_copyable_capability_disjoint() {
    // droppable_capability: key ∧ store ∧ drop ∧ ¬copy
    // copyable_capability: key ∧ store ∧ copy
    // These should never overlap (¬copy vs copy)

    for tc in TEST_MATRIX {
        let has_key = tc.abilities.contains(&"key");
        let has_store = tc.abilities.contains(&"store");
        let has_drop = tc.abilities.contains(&"drop");
        let has_copy = tc.abilities.contains(&"copy");

        let triggers_droppable = has_key && has_store && has_drop && !has_copy;
        let triggers_copyable = has_key && has_store && has_copy;

        // They should never both be true
        assert!(
            !(triggers_droppable && triggers_copyable),
            "Case {} triggers both lints! Abilities: {:?}",
            tc.index,
            tc.abilities
        );
    }
}

#[test]
fn spec_droppable_capability_exhaustive() {
    let mut src = String::new();
    src.push_str(
        r#"module sui::object {
    public struct UID has copy, drop, store { v: u64 }
}
module spec_test_pkg::m {
    use sui::object::UID;
"#,
    );

    for tc in TEST_MATRIX {
        let abilities_str = if tc.abilities.is_empty() {
            String::new()
        } else {
            format!(" has {}", tc.abilities.join(", "))
        };
        src.push_str(&format!(
            "    public struct TestStruct_{}{} {{ id: UID, v: u64 }}\n",
            tc.index, abilities_str
        ));
    }
    src.push_str("}\n");

    let tmp = create_temp_sui_package(&src).expect("should create temp package");
    let diags =
        move_clippy::semantic::lint_package(tmp.path(), &LintSettings::default(), false, false)
            .expect("semantic linting should succeed");

    let mut fired: Vec<String> = diags
        .into_iter()
        .filter(|d| d.lint.name == "droppable_capability")
        .filter_map(|d| extract_struct_name(&d.message))
        .collect();
    fired.sort();

    let mut expected: Vec<String> = TEST_MATRIX
        .iter()
        .filter(|tc| tc.expected_warn)
        .map(|tc| format!("TestStruct_{}", tc.index))
        .collect();
    expected.sort();

    assert_eq!(
        fired, expected,
        "droppable_capability should fire exactly for the key+store+drop (no copy) cases"
    );
}
