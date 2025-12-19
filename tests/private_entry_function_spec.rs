//! Exhaustive spec tests for `private_entry_function` lint.
//!
//! This test verifies the lint catches all 6 problematic cases in the
//! visibility × entry state space (3 visibilities × 2 entry states = 6 combinations).
//!
//! The lint fires when: visibility = private AND is_entry = true
//!
//! Test matrix:
//! | # | Visibility      | Entry | Expected |
//! |---|-----------------|-------|----------|
//! | 0 | private         | no    | NO       |
//! | 1 | private         | yes   | WARN     | <- only this case triggers
//! | 2 | public(package) | no    | NO       |
//! | 3 | public(package) | yes   | NO       |
//! | 4 | public          | no    | NO       |
//! | 5 | public          | yes   | NO       |

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

    /// Generate Move source that exercises all visibility × entry combinations.
    fn generate_move_source() -> String {
        r#"
module spec_test::test_module {
    // Case 0: private, not entry - NO WARN
    fun private_no_entry() {}

    // Case 1: private, entry - WARN (private entry is unreachable)
    entry fun private_with_entry() {}

    // Case 2: public(package), not entry - NO WARN
    public(package) fun package_no_entry() {}

    // Case 3: public(package), entry - NO WARN (callable from same-package transactions)
    public(package) entry fun package_with_entry() {}

    // Case 4: public, not entry - NO WARN
    public fun public_no_entry() {}

    // Case 5: public, entry - NO WARN (standard entry point)
    public entry fun public_with_entry() {}
}
"#
        .to_string()
    }

    #[test]
    fn spec_private_entry_function_exhaustive() {
        let source = generate_move_source();
        let temp_dir = create_temp_package(MOVE_TOML, &[("main.move", &source)])
            .expect("setup should succeed");
        let settings = LintSettings::default();

        let diags = semantic::lint_package(temp_dir.path(), &settings, true, false)
            .expect("lint_package should succeed");

        // Filter to only private_entry_function diagnostics
        let private_entry_diags: Vec<_> = diags
            .iter()
            .filter(|d| d.lint.name == "private_entry_function")
            .collect();

        // Should have exactly 1 diagnostic (Case 1: private + entry)
        assert_eq!(
            private_entry_diags.len(),
            1,
            "Expected exactly 1 private_entry_function diagnostic, got {}:\n{:#?}",
            private_entry_diags.len(),
            private_entry_diags
        );

        // Verify it's for the correct function
        let diag = &private_entry_diags[0];
        assert!(
            diag.message.contains("private_with_entry"),
            "Expected diagnostic for 'private_with_entry', got: {}",
            diag.message
        );
    }

    /// Verify that the lint message is actionable.
    #[test]
    fn spec_private_entry_function_message_quality() {
        let source = r#"
module spec_test::test {
    entry fun unreachable_fn() {}
}
"#;
        let temp_dir =
            create_temp_package(MOVE_TOML, &[("main.move", source)]).expect("setup should succeed");
        let settings = LintSettings::default();

        let diags = semantic::lint_package(temp_dir.path(), &settings, true, false)
            .expect("lint_package should succeed");

        let private_entry_diags: Vec<_> = diags
            .iter()
            .filter(|d| d.lint.name == "private_entry_function")
            .collect();

        assert_eq!(private_entry_diags.len(), 1);

        let msg = &private_entry_diags[0].message;

        // Message should explain the problem
        assert!(
            msg.contains("unreachable") || msg.contains("dead code"),
            "Message should explain the bug"
        );

        // Message should suggest fixes
        assert!(
            msg.contains("public entry") || msg.contains("remove"),
            "Message should suggest fixes"
        );
    }

    /// Verify no false positives on edge cases.
    #[test]
    fn spec_private_entry_function_no_false_positives() {
        // All these should NOT trigger the lint
        let source = r#"
module spec_test::edge_cases {
    // Regular private function
    fun helper() {}

    // Public entry with complex signature
    public entry fun complex_entry(x: u64, y: bool) {}

    // Package-level entry
    public(package) entry fun package_entry() {}

    // Friend function (not entry)
    public(package) fun friend_helper() {}
}
"#;
        let temp_dir =
            create_temp_package(MOVE_TOML, &[("main.move", source)]).expect("setup should succeed");
        let settings = LintSettings::default();

        let diags = semantic::lint_package(temp_dir.path(), &settings, true, false)
            .expect("lint_package should succeed");

        let private_entry_diags: Vec<_> = diags
            .iter()
            .filter(|d| d.lint.name == "private_entry_function")
            .collect();

        assert!(
            private_entry_diags.is_empty(),
            "Expected no private_entry_function diagnostics, got: {:#?}",
            private_entry_diags
        );
    }
}
