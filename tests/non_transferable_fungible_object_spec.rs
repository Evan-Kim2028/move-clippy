//! Exhaustive spec-driven tests for `non_transferable_fungible_object` lint.
//!
//! # Formal Specification
//!
//! ```text
//! INVARIANT: WARN if has_key(S) ∧ ¬has_store(S) ∧ (has_copy(S) ∨ has_drop(S))
//! ```
//!
//! # First Principles Reasoning
//!
//! An object with `key` but without `store` is **non-transferable** (soulbound).
//! This is a legitimate pattern for badges, achievements, or identity tokens.
//!
//! However, adding `copy` or `drop` to a non-transferable object is incoherent:
//! - `{key, drop}`: Can be dropped but not transferred - why make it an object?
//! - `{key, copy}`: Can be copied but not transferred - defeats soulbound semantics
//! - `{key, copy, drop}`: Fully fungible but not transferable - contradictory design
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
//! # Expected Results
//!
//! Warning fires when: key AND NOT store AND (copy OR drop)
//!
//! Cases that should warn:
//! - `{key, drop}` - case 7
//! - `{key, copy}` - case 6
//! - `{key, copy, drop}` - case 13

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
        rationale: "No abilities - hot potato pattern",
    },
    // =========================================================================
    // Single ability (4 cases)
    // =========================================================================
    TestCase {
        index: 1,
        abilities: &["key"],
        expected_warn: false,
        rationale: "Key only - LEGITIMATE soulbound object",
    },
    TestCase {
        index: 2,
        abilities: &["store"],
        expected_warn: false,
        rationale: "Store only - embeddable struct, not an object",
    },
    TestCase {
        index: 3,
        abilities: &["copy"],
        expected_warn: false,
        rationale: "Copy only - not an object (no key)",
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
        rationale: "Key+store - LEGITIMATE transferable object/capability",
    },
    TestCase {
        index: 6,
        abilities: &["key", "copy"],
        expected_warn: true, // <-- SHOULD WARN
        rationale: "Key+copy without store - INCOHERENT (copyable but not transferable)",
    },
    TestCase {
        index: 7,
        abilities: &["key", "drop"],
        expected_warn: true, // <-- SHOULD WARN
        rationale: "Key+drop without store - INCOHERENT (droppable but not transferable)",
    },
    TestCase {
        index: 8,
        abilities: &["store", "copy"],
        expected_warn: false,
        rationale: "Store+copy - embeddable copyable, not an object",
    },
    TestCase {
        index: 9,
        abilities: &["store", "drop"],
        expected_warn: false,
        rationale: "Store+drop - embeddable droppable, not an object",
    },
    TestCase {
        index: 10,
        abilities: &["copy", "drop"],
        expected_warn: false,
        rationale: "Copy+drop - event/DTO pattern, not an object",
    },
    // =========================================================================
    // Three abilities (4 cases)
    // =========================================================================
    TestCase {
        index: 11,
        abilities: &["key", "store", "copy"],
        expected_warn: false,
        rationale: "Key+store+copy - has store, handled by copyable_capability",
    },
    TestCase {
        index: 12,
        abilities: &["key", "store", "drop"],
        expected_warn: false,
        rationale: "Key+store+drop - has store, handled by droppable_capability",
    },
    TestCase {
        index: 13,
        abilities: &["key", "copy", "drop"],
        expected_warn: true, // <-- SHOULD WARN
        rationale: "Key+copy+drop without store - INCOHERENT (fully fungible but not transferable)",
    },
    TestCase {
        index: 14,
        abilities: &["store", "copy", "drop"],
        expected_warn: false,
        rationale: "Store+copy+drop - pure value struct, not an object",
    },
    // =========================================================================
    // All four abilities (1 case)
    // =========================================================================
    TestCase {
        index: 15,
        abilities: &["key", "store", "copy", "drop"],
        expected_warn: false,
        rationale: "All abilities - has store, handled by copyable_capability",
    },
];

/// Verify the test matrix is complete (16 cases).
#[test]
fn spec_non_transferable_fungible_object_matrix_is_complete() {
    assert_eq!(
        TEST_MATRIX.len(),
        16,
        "Expected 16 test cases (2^4 ability combos), got {}",
        TEST_MATRIX.len()
    );

    // Count expected warnings (should be 3: key without store but with copy or drop)
    let warn_count = TEST_MATRIX.iter().filter(|tc| tc.expected_warn).count();
    assert_eq!(
        warn_count, 3,
        "Expected 3 warning cases (key without store, with copy or drop), got {}",
        warn_count
    );

    // Verify all warn cases have key, no store, and (copy or drop)
    for tc in TEST_MATRIX.iter().filter(|tc| tc.expected_warn) {
        let has_key = tc.abilities.contains(&"key");
        let has_store = tc.abilities.contains(&"store");
        let has_copy = tc.abilities.contains(&"copy");
        let has_drop = tc.abilities.contains(&"drop");

        assert!(
            has_key && !has_store && (has_copy || has_drop),
            "Warning case {} should have key, no store, and (copy or drop): {:?}",
            tc.index,
            tc.abilities
        );
    }
}

/// Verify the formal invariant matches our expectations.
#[test]
fn spec_non_transferable_fungible_object_invariant() {
    // The invariant: WARN if has_key(S) ∧ ¬has_store(S) ∧ (has_copy(S) ∨ has_drop(S))

    for tc in TEST_MATRIX {
        let has_key = tc.abilities.contains(&"key");
        let has_store = tc.abilities.contains(&"store");
        let has_copy = tc.abilities.contains(&"copy");
        let has_drop = tc.abilities.contains(&"drop");

        // The invariant says: warn iff (key AND NOT store AND (copy OR drop))
        let should_warn = has_key && !has_store && (has_copy || has_drop);

        assert_eq!(
            tc.expected_warn, should_warn,
            "Case {} mismatch: expected_warn={} but invariant says {}. Abilities: {:?}",
            tc.index, tc.expected_warn, should_warn, tc.abilities
        );
    }
}

/// Verify this lint is disjoint from copyable_capability and droppable_capability.
#[test]
fn spec_non_transferable_disjoint_from_capability_lints() {
    // non_transferable_fungible_object: key ∧ ¬store ∧ (copy ∨ drop)
    // copyable_capability: key ∧ store ∧ copy
    // droppable_capability: key ∧ store ∧ drop ∧ ¬copy
    //
    // These are disjoint because:
    // - non_transferable requires ¬store
    // - copyable/droppable_capability require store

    for tc in TEST_MATRIX {
        let has_key = tc.abilities.contains(&"key");
        let has_store = tc.abilities.contains(&"store");
        let has_copy = tc.abilities.contains(&"copy");
        let has_drop = tc.abilities.contains(&"drop");

        let triggers_non_transferable = has_key && !has_store && (has_copy || has_drop);
        let triggers_copyable_cap = has_key && has_store && has_copy;
        let triggers_droppable_cap = has_key && has_store && has_drop && !has_copy;

        // non_transferable should never overlap with capability lints
        if triggers_non_transferable {
            assert!(
                !triggers_copyable_cap && !triggers_droppable_cap,
                "Case {} triggers both non_transferable and a capability lint! Abilities: {:?}",
                tc.index,
                tc.abilities
            );
        }
    }
}

/// Verify that all "key without store" cases are either legitimate or caught.
#[test]
fn spec_all_key_without_store_cases_analyzed() {
    // Cases with key but no store:
    // - {key} alone = legitimate soulbound
    // - {key, copy} = WARN (incoherent)
    // - {key, drop} = WARN (incoherent)
    // - {key, copy, drop} = WARN (incoherent)

    let key_without_store: Vec<_> = TEST_MATRIX
        .iter()
        .filter(|tc| tc.abilities.contains(&"key") && !tc.abilities.contains(&"store"))
        .collect();

    assert_eq!(
        key_without_store.len(),
        4,
        "Should have 4 cases with key but no store"
    );

    // Exactly one should NOT warn (the legitimate soulbound case)
    let legitimate_count = key_without_store
        .iter()
        .filter(|tc| !tc.expected_warn)
        .count();
    assert_eq!(
        legitimate_count, 1,
        "Exactly one key-without-store case should be legitimate (pure soulbound)"
    );

    // That case should be {key} alone
    let legitimate = key_without_store
        .iter()
        .find(|tc| !tc.expected_warn)
        .unwrap();
    assert_eq!(
        legitimate.abilities,
        &["key"],
        "The legitimate soulbound case should be {{key}} alone"
    );
}

#[test]
fn spec_non_transferable_fungible_object_exhaustive() {
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
        .filter(|d| d.lint.name == "non_transferable_fungible_object")
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
        "non_transferable_fungible_object should fire exactly for the key-without-store but copy/drop cases"
    );
}
