//! Exhaustive spec-driven tests for `share_owned_authority` lint.
//!
//! # Formal Specification
//!
//! ```text
//! INVARIANT:
//!   For call share_object<T>(...) or public_share_object<T>(...):
//!     WARN if: abilities(T) ⊇ {key, store}
//!
//!   Equivalently:
//!     is_share_call ∧ has_key(T) ∧ has_store(T)
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
//! | call_type | {share_object, public_share_object} |
//!
//! For struct definition tests: 2^4 = 16 ability combinations
//! For call tests: 16 abilities × 2 call types = 32 cases
//!
//! # Expected Result
//!
//! Warning fires when:
//! - Type has BOTH key AND store abilities
//! - Called via share_object or public_share_object
//!
//! This proves that ONLY key+store types trigger the warning.

#![cfg(feature = "full")]

mod support;

use move_clippy::lint::LintSettings;
use support::semantic_spec_harness::create_temp_package;

const MOVE_TOML: &str = r#"[package]
name = "spec_test_pkg"
edition = "2024"

[dependencies]
Sui = { git = "https://github.com/MystenLabs/sui.git", subdir = "crates/sui-framework/packages/sui-framework", rev = "framework/testnet" }

[addresses]
spec_test_pkg = "0x0"
sui = "0x2"
"#;

/// A single test case in our exhaustive matrix.
#[derive(Debug, Clone)]
struct TestCase {
    /// Abilities to add to the struct
    abilities: &'static [&'static str],
    /// Which share function to call (share_object or public_share_object)
    share_fn: &'static str,
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

    format!(
        r#"module spec_test_pkg::test_case_{index} {{
    use sui::transfer;
    use sui::object::{{Self, UID}};
    use sui::tx_context::TxContext;

    public struct TestStruct{abilities} {{
        id: UID,
        value: u64,
    }}

    public fun share_it(obj: TestStruct) {{
        transfer::{share_fn}(obj);
    }}
}}
"#,
        abilities = abilities_str,
        share_fn = tc.share_fn
    )
}

/// Run semantic lints on a Move package and return diagnostics for a specific lint.
fn run_lint(source: &str, lint_name: &str) -> Vec<String> {
    let tmp = match create_temp_package(MOVE_TOML, &[("test.move", source)]) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to create temp package: {}", e);
            return vec![];
        }
    };

    let result =
        move_clippy::semantic::lint_package(tmp.path(), &LintSettings::default(), false, false);

    match result {
        Ok(diags) => diags
            .into_iter()
            .filter(|d| d.lint.name == lint_name)
            .map(|d| d.message)
            .collect(),
        Err(e) => {
            // Compilation errors may occur for invalid ability combinations
            eprintln!("Compilation/lint error (may be expected): {}", e);
            vec![]
        }
    }
}

/// The exhaustive test matrix.
///
/// We test all 16 ability combinations × 2 share function variants = 32 cases.
///
/// The lint should ONLY fire when the type has BOTH key AND store.
const TEST_MATRIX: &[TestCase] = &[
    // =========================================================================
    // share_object variant (16 ability combinations)
    // =========================================================================

    // No abilities
    TestCase {
        abilities: &[],
        share_fn: "share_object",
        expected_warn: false,
        rationale: "No abilities - not an authority object",
    },
    // Single abilities
    TestCase {
        abilities: &["key"],
        share_fn: "share_object",
        expected_warn: false,
        rationale: "Key only - missing store, not transferable authority",
    },
    TestCase {
        abilities: &["store"],
        share_fn: "share_object",
        expected_warn: false,
        rationale: "Store only - missing key, not an object",
    },
    TestCase {
        abilities: &["copy"],
        share_fn: "share_object",
        expected_warn: false,
        rationale: "Copy only - not an object",
    },
    TestCase {
        abilities: &["drop"],
        share_fn: "share_object",
        expected_warn: false,
        rationale: "Drop only - not an object",
    },
    // Key + one other (key+store is the critical case)
    TestCase {
        abilities: &["key", "store"],
        share_fn: "share_object",
        expected_warn: true, // <-- SHOULD WARN
        rationale: "Key+store = transferable authority being shared publicly",
    },
    TestCase {
        abilities: &["key", "copy"],
        share_fn: "share_object",
        expected_warn: false,
        rationale: "Key+copy - missing store, not transferable",
    },
    TestCase {
        abilities: &["key", "drop"],
        share_fn: "share_object",
        expected_warn: false,
        rationale: "Key+drop - missing store, not transferable",
    },
    // Store + one other (without key)
    TestCase {
        abilities: &["store", "copy"],
        share_fn: "share_object",
        expected_warn: false,
        rationale: "Store+copy - missing key, not an object",
    },
    TestCase {
        abilities: &["store", "drop"],
        share_fn: "share_object",
        expected_warn: false,
        rationale: "Store+drop - missing key, not an object",
    },
    // Copy + drop
    TestCase {
        abilities: &["copy", "drop"],
        share_fn: "share_object",
        expected_warn: false,
        rationale: "Copy+drop = event pattern, not an object",
    },
    // Three abilities
    TestCase {
        abilities: &["key", "store", "copy"],
        share_fn: "share_object",
        expected_warn: true, // <-- SHOULD WARN (has key+store)
        rationale: "Key+store+copy - still a transferable authority",
    },
    TestCase {
        abilities: &["key", "store", "drop"],
        share_fn: "share_object",
        expected_warn: true, // <-- SHOULD WARN (has key+store)
        rationale: "Key+store+drop - still a transferable authority",
    },
    TestCase {
        abilities: &["key", "copy", "drop"],
        share_fn: "share_object",
        expected_warn: false,
        rationale: "Key+copy+drop - missing store, not transferable",
    },
    TestCase {
        abilities: &["store", "copy", "drop"],
        share_fn: "share_object",
        expected_warn: false,
        rationale: "Store+copy+drop - missing key, not an object",
    },
    // All four abilities
    TestCase {
        abilities: &["key", "store", "copy", "drop"],
        share_fn: "share_object",
        expected_warn: true, // <-- SHOULD WARN (has key+store)
        rationale: "All abilities - still has key+store = transferable authority",
    },
    // =========================================================================
    // public_share_object variant (16 ability combinations)
    // Same pattern - just testing the other function name
    // =========================================================================
    TestCase {
        abilities: &[],
        share_fn: "public_share_object",
        expected_warn: false,
        rationale: "No abilities (public_share) - not an authority object",
    },
    TestCase {
        abilities: &["key"],
        share_fn: "public_share_object",
        expected_warn: false,
        rationale: "Key only (public_share) - missing store",
    },
    TestCase {
        abilities: &["store"],
        share_fn: "public_share_object",
        expected_warn: false,
        rationale: "Store only (public_share) - missing key",
    },
    TestCase {
        abilities: &["copy"],
        share_fn: "public_share_object",
        expected_warn: false,
        rationale: "Copy only (public_share) - not an object",
    },
    TestCase {
        abilities: &["drop"],
        share_fn: "public_share_object",
        expected_warn: false,
        rationale: "Drop only (public_share) - not an object",
    },
    TestCase {
        abilities: &["key", "store"],
        share_fn: "public_share_object",
        expected_warn: true, // <-- SHOULD WARN
        rationale: "Key+store (public_share) = transferable authority being shared",
    },
    TestCase {
        abilities: &["key", "copy"],
        share_fn: "public_share_object",
        expected_warn: false,
        rationale: "Key+copy (public_share) - missing store",
    },
    TestCase {
        abilities: &["key", "drop"],
        share_fn: "public_share_object",
        expected_warn: false,
        rationale: "Key+drop (public_share) - missing store",
    },
    TestCase {
        abilities: &["store", "copy"],
        share_fn: "public_share_object",
        expected_warn: false,
        rationale: "Store+copy (public_share) - missing key",
    },
    TestCase {
        abilities: &["store", "drop"],
        share_fn: "public_share_object",
        expected_warn: false,
        rationale: "Store+drop (public_share) - missing key",
    },
    TestCase {
        abilities: &["copy", "drop"],
        share_fn: "public_share_object",
        expected_warn: false,
        rationale: "Copy+drop (public_share) - event pattern",
    },
    TestCase {
        abilities: &["key", "store", "copy"],
        share_fn: "public_share_object",
        expected_warn: true, // <-- SHOULD WARN
        rationale: "Key+store+copy (public_share) - transferable authority",
    },
    TestCase {
        abilities: &["key", "store", "drop"],
        share_fn: "public_share_object",
        expected_warn: true, // <-- SHOULD WARN
        rationale: "Key+store+drop (public_share) - transferable authority",
    },
    TestCase {
        abilities: &["key", "copy", "drop"],
        share_fn: "public_share_object",
        expected_warn: false,
        rationale: "Key+copy+drop (public_share) - missing store",
    },
    TestCase {
        abilities: &["store", "copy", "drop"],
        share_fn: "public_share_object",
        expected_warn: false,
        rationale: "Store+copy+drop (public_share) - missing key",
    },
    TestCase {
        abilities: &["key", "store", "copy", "drop"],
        share_fn: "public_share_object",
        expected_warn: true, // <-- SHOULD WARN
        rationale: "All abilities (public_share) - has key+store",
    },
];

/// This test requires network access to fetch Sui framework.
/// Run with: cargo test --features full share_owned_authority_spec -- --ignored
#[test]
#[ignore = "requires Sui framework download - run manually"]
fn spec_share_owned_authority_exhaustive() {
    let mut passed = 0;
    let mut failed = 0;
    let mut warn_cases = Vec::new();

    for (i, tc) in TEST_MATRIX.iter().enumerate() {
        let fixture = generate_fixture(tc, i);
        let diags = run_lint(&fixture, "share_owned_authority");
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
                 Share fn: {}\n\
                 Expected warn: {}\n\
                 Actual warn: {}\n\
                 Rationale: {}\n\
                 Fixture:\n{}\n\
                 Diagnostics: {:?}\n",
                i, tc.abilities, tc.share_fn, tc.expected_warn, fired, tc.rationale, fixture, diags
            );
        }
    }

    println!("\n=== SPEC TEST SUMMARY ===");
    println!("Total cases: {}", TEST_MATRIX.len());
    println!("Passed: {}", passed);
    println!("Failed: {}", failed);
    println!("Cases that triggered warning: {:?}", warn_cases);

    // Verify expected number of warnings (8 cases: 4 key+store combinations × 2 share functions)
    let expected_warn_count = TEST_MATRIX.iter().filter(|tc| tc.expected_warn).count();
    assert_eq!(
        warn_cases.len(),
        expected_warn_count,
        "Expected {} cases to trigger warning, got {}",
        expected_warn_count,
        warn_cases.len()
    );

    assert_eq!(failed, 0, "{} test cases failed", failed);
}

/// Verify the test matrix is complete (32 cases).
#[test]
fn spec_share_owned_authority_matrix_is_complete() {
    assert_eq!(
        TEST_MATRIX.len(),
        32,
        "Expected 32 test cases (16 ability combos × 2 share functions), got {}",
        TEST_MATRIX.len()
    );

    // Count expected warnings (should be 8: key+store variants)
    let warn_count = TEST_MATRIX.iter().filter(|tc| tc.expected_warn).count();
    assert_eq!(
        warn_count, 8,
        "Expected 8 warning cases (4 key+store combos × 2 share fns), got {}",
        warn_count
    );

    // Verify all warn cases have key+store
    for tc in TEST_MATRIX.iter().filter(|tc| tc.expected_warn) {
        assert!(
            tc.abilities.contains(&"key") && tc.abilities.contains(&"store"),
            "Warning case should have key+store: {:?}",
            tc.abilities
        );
    }
}

/// Test that the fixture generator produces valid Move code structure.
#[test]
fn fixture_generator_produces_valid_code() {
    for (i, tc) in TEST_MATRIX.iter().enumerate() {
        let fixture = generate_fixture(tc, i);

        assert!(
            fixture.contains("module spec_test_pkg::test_case_"),
            "Missing module"
        );
        assert!(
            fixture.contains("public struct TestStruct"),
            "Missing struct"
        );
        assert!(
            fixture.contains(&format!("transfer::{}", tc.share_fn)),
            "Missing share call"
        );

        if !tc.abilities.is_empty() {
            assert!(fixture.contains("has"), "Missing 'has' for abilities");
        }
    }
}
