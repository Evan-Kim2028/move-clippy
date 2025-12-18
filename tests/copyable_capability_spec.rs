//! Exhaustive spec-driven tests for `copyable_capability` lint.
//!
//! # Formal Specification
//!
//! ```text
//! INVARIANT: WARN if has_key(S) ∧ has_store(S) ∧ has_copy(S)
//! ```
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
//! Warning fires when: key AND store AND copy are ALL present.
//! Only 4 cases should trigger (cases with key+store+copy):
//! - key+store+copy
//! - key+store+copy+drop

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
        rationale: "Drop only - not a capability",
    },
    // =========================================================================
    // Two abilities (6 cases)
    // =========================================================================
    TestCase {
        index: 5,
        abilities: &["key", "store"],
        expected_warn: false,
        rationale: "Key+store = proper capability pattern (no copy)",
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
        rationale: "Key+drop - missing store, not transferable",
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
        expected_warn: true, // <-- SHOULD WARN
        rationale: "Key+store+copy = COPYABLE CAPABILITY BUG",
    },
    TestCase {
        index: 12,
        abilities: &["key", "store", "drop"],
        expected_warn: false,
        rationale: "Key+store+drop = droppable capability (different lint)",
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
        expected_warn: true, // <-- SHOULD WARN
        rationale: "All abilities = still a COPYABLE CAPABILITY BUG",
    },
];

/// Verify the test matrix is complete (16 cases).
#[test]
fn spec_copyable_capability_matrix_is_complete() {
    assert_eq!(
        TEST_MATRIX.len(),
        16,
        "Expected 16 test cases (2^4 ability combos), got {}",
        TEST_MATRIX.len()
    );

    // Count expected warnings (should be 2: key+store+copy variants)
    let warn_count = TEST_MATRIX.iter().filter(|tc| tc.expected_warn).count();
    assert_eq!(
        warn_count, 2,
        "Expected 2 warning cases (key+store+copy variants), got {}",
        warn_count
    );

    // Verify all warn cases have key+store+copy
    for tc in TEST_MATRIX.iter().filter(|tc| tc.expected_warn) {
        assert!(
            tc.abilities.contains(&"key")
                && tc.abilities.contains(&"store")
                && tc.abilities.contains(&"copy"),
            "Warning case {} should have key+store+copy: {:?}",
            tc.index,
            tc.abilities
        );
    }

    // Verify no case without key+store+copy warns
    for tc in TEST_MATRIX.iter().filter(|tc| !tc.expected_warn) {
        let has_all_three = tc.abilities.contains(&"key")
            && tc.abilities.contains(&"store")
            && tc.abilities.contains(&"copy");
        assert!(
            !has_all_three,
            "Non-warning case {} should not have key+store+copy: {:?}",
            tc.index, tc.abilities
        );
    }
}

/// Verify the formal invariant matches our expectations.
#[test]
fn spec_copyable_capability_invariant() {
    // The invariant: WARN if has_key(S) ∧ has_store(S) ∧ has_copy(S)

    for tc in TEST_MATRIX {
        let has_key = tc.abilities.contains(&"key");
        let has_store = tc.abilities.contains(&"store");
        let has_copy = tc.abilities.contains(&"copy");

        // The invariant says: warn iff (key AND store AND copy)
        let should_warn = has_key && has_store && has_copy;

        assert_eq!(
            tc.expected_warn, should_warn,
            "Case {} mismatch: expected_warn={} but invariant says {}. Abilities: {:?}",
            tc.index, tc.expected_warn, should_warn, tc.abilities
        );
    }
}

#[test]
fn spec_copyable_capability_exhaustive() {
    // Build a single package containing all 16 structs, so we compile once.
    let mut src = String::new();

    // Test-only UID shim so we can exercise ability combinations under Sui flavor.
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
        .filter(|d| d.lint.name == "copyable_capability")
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
        "copyable_capability should fire exactly for the key+store+copy cases"
    );
}
