//! Exhaustive spec tests for `entry_function_returns_value` lint.
//!
//! This test verifies the lint catches all cases where entry functions
//! return non-unit values (which are discarded by the runtime).
//!
//! Test matrix (entry × return type):
//! | # | Entry | Return Type | Expected |
//! |---|-------|-------------|----------|
//! | 0 | no    | unit        | NO       |
//! | 1 | no    | non-unit    | NO       | (non-entry can return values)
//! | 2 | yes   | unit        | NO       | (standard entry point)
//! | 3 | yes   | non-unit    | WARN     | <- only this case triggers

#[cfg(feature = "full")]
mod support;

#[cfg(feature = "full")]
mod full {
    use crate::support::semantic_spec_harness::create_temp_package;
    use move_clippy::lint::LintSettings;
    use move_clippy::semantic;

    const MOVE_TOML: &str = r#"
[package]
name = "spec_test"
edition = "2024"

[addresses]
spec_test = "0x0"
"#;

    /// Test the 4 combinations of entry × return type.
    #[test]
    fn spec_entry_returns_value_exhaustive() {
        let source = r#"
module spec_test::test_module {
    // Case 0: not entry, returns unit - NO WARN
    public fun non_entry_unit() {}

    // Case 1: not entry, returns value - NO WARN (caller receives value)
    public fun non_entry_returns_value(): u64 { 42 }

    // Case 2: entry, returns unit - NO WARN (standard pattern)
    public entry fun entry_unit() {}

    // Case 3: entry, returns value - WARN (value is discarded!)
    public entry fun entry_returns_value(): u64 { 42 }
}
"#;
        let temp_dir =
            create_temp_package(MOVE_TOML, &[("main.move", source)]).expect("setup should succeed");
        let settings = LintSettings::default();

        let diags = semantic::lint_package(temp_dir.path(), &settings, true, false)
            .expect("linting should succeed");

        // Filter to only entry_function_returns_value diagnostics
        let entry_returns_diags: Vec<_> = diags
            .iter()
            .filter(|d| d.lint.name == "entry_function_returns_value")
            .collect();

        // Should have exactly 1 diagnostic (Case 3: entry + returns value)
        assert_eq!(
            entry_returns_diags.len(),
            1,
            "Expected exactly 1 entry_function_returns_value diagnostic, got {}:\n{:#?}",
            entry_returns_diags.len(),
            entry_returns_diags
        );

        // Verify it's for the correct function
        let diag = &entry_returns_diags[0];
        assert!(
            diag.message.contains("entry_returns_value"),
            "Expected diagnostic for 'entry_returns_value', got: {}",
            diag.message
        );
    }

    /// Test various return types that should trigger the lint.
    #[test]
    fn spec_entry_returns_value_various_types() {
        let source = r#"
module spec_test::various_types {
    // Primitive types
    public entry fun returns_u64(): u64 { 42 }
    public entry fun returns_bool(): bool { true }
    public entry fun returns_address(): address { @0x1 }

    // Tuple (multiple return values)
    public entry fun returns_tuple(): (u64, bool) { (42, true) }

    // Vector
    public entry fun returns_vector(): vector<u8> { vector[] }
}
"#;
        let temp_dir =
            create_temp_package(MOVE_TOML, &[("main.move", source)]).expect("setup should succeed");
        let settings = LintSettings::default();

        let diags = semantic::lint_package(temp_dir.path(), &settings, true, false)
            .expect("linting should succeed");

        let entry_returns_diags: Vec<_> = diags
            .iter()
            .filter(|d| d.lint.name == "entry_function_returns_value")
            .collect();

        // All 5 functions should trigger the lint
        assert_eq!(
            entry_returns_diags.len(),
            5,
            "Expected 5 entry_function_returns_value diagnostics (one per function), got {}:\n{:#?}",
            entry_returns_diags.len(),
            entry_returns_diags
        );
    }

    /// Verify no false positives on legitimate patterns.
    #[test]
    fn spec_entry_returns_value_no_false_positives() {
        let source = r#"
module spec_test::legit_patterns {
    // Standard entry point
    public entry fun do_something() {}

    // Factory function (not entry, returns value)
    public fun create_thing(): u64 { 42 }

    // Entry with parameters but unit return
    public entry fun with_params(x: u64, y: bool) {}

    // Private entry that returns unit (caught by other lint, not this one)
    entry fun private_entry_unit() {}
}
"#;
        let temp_dir =
            create_temp_package(MOVE_TOML, &[("main.move", source)]).expect("setup should succeed");
        let settings = LintSettings::default();

        let diags = semantic::lint_package(temp_dir.path(), &settings, true, false)
            .expect("linting should succeed");

        let entry_returns_diags: Vec<_> = diags
            .iter()
            .filter(|d| d.lint.name == "entry_function_returns_value")
            .collect();

        assert!(
            entry_returns_diags.is_empty(),
            "Expected no entry_function_returns_value diagnostics, got: {:#?}",
            entry_returns_diags
        );
    }

    /// Verify lint message quality.
    #[test]
    fn spec_entry_returns_value_message_quality() {
        let source = r#"
module spec_test::test {
    public entry fun bad_entry(): u64 { 42 }
}
"#;
        let temp_dir =
            create_temp_package(MOVE_TOML, &[("main.move", source)]).expect("setup should succeed");
        let settings = LintSettings::default();

        let diags = semantic::lint_package(temp_dir.path(), &settings, true, false)
            .expect("linting should succeed");

        let entry_returns_diags: Vec<_> = diags
            .iter()
            .filter(|d| d.lint.name == "entry_function_returns_value")
            .collect();

        assert_eq!(entry_returns_diags.len(), 1);

        let msg = &entry_returns_diags[0].message;

        // Message should explain the problem
        assert!(
            msg.contains("discarded") || msg.contains("lost"),
            "Message should explain the bug: {}",
            msg
        );

        // Message should mention entry function semantics
        assert!(
            msg.contains("runtime") || msg.contains("entry"),
            "Message should explain entry function semantics: {}",
            msg
        );
    }

    /// Test interaction with visibility modifiers.
    #[test]
    fn spec_entry_returns_value_all_visibilities() {
        let source = r#"
module spec_test::visibility_test {
    // Private entry returning value - triggers BOTH lints
    entry fun private_entry_returns(): u64 { 1 }

    // Package entry returning value - triggers this lint only
    public(package) entry fun package_entry_returns(): u64 { 2 }

    // Public entry returning value - triggers this lint only
    public entry fun public_entry_returns(): u64 { 3 }
}
"#;
        let temp_dir =
            create_temp_package(MOVE_TOML, &[("main.move", source)]).expect("setup should succeed");
        let settings = LintSettings::default();

        let diags = semantic::lint_package(temp_dir.path(), &settings, true, false)
            .expect("linting should succeed");

        let entry_returns_diags: Vec<_> = diags
            .iter()
            .filter(|d| d.lint.name == "entry_function_returns_value")
            .collect();

        // All 3 entry functions return non-unit values
        assert_eq!(
            entry_returns_diags.len(),
            3,
            "Expected 3 entry_function_returns_value diagnostics, got {}:\n{:#?}",
            entry_returns_diags.len(),
            entry_returns_diags
        );
    }
}
