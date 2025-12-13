//! Ecosystem Snapshot Tests
//!
//! These tests run move-clippy against real-world Move codebases and snapshot
//! the results. This ensures:
//!
//! 1. **No regressions:** Changes to lints don't introduce new false positives
//! 2. **Real-world validation:** Lints work correctly on production code
//! 3. **FP detection:** New FPs surface as snapshot changes in CI
//!
//! ## How It Works
//!
//! 1. Clone or reference ecosystem repos (e.g., OpenZeppelin Sui)
//! 2. Run all stable lints against each repo
//! 3. Snapshot the findings using insta
//! 4. Review any changes in CI before merging
//!
//! ## Updating Snapshots
//!
//! When lints legitimately change behavior:
//! ```bash
//! cargo insta test --test ecosystem_snapshots
//! cargo insta review
//! ```

use move_clippy::LintEngine;
use move_clippy::lint::{LintRegistry, LintSettings};
use std::collections::BTreeMap;
use std::path::Path;
use walkdir::WalkDir;

/// Helper to lint source and return diagnostic messages
fn lint_source(source: &str) -> Vec<String> {
    let registry = LintRegistry::default_rules();
    let engine = LintEngine::new_with_settings(registry, LintSettings::default());
    engine
        .lint_source(source)
        .unwrap()
        .into_iter()
        .map(|d| format!("{}: {}", d.lint.name, d.message))
        .collect()
}

/// Run lints on a directory and return a sorted, deterministic summary
fn lint_directory(path: &Path) -> BTreeMap<String, Vec<String>> {
    let mut results: BTreeMap<String, Vec<String>> = BTreeMap::new();

    if !path.exists() {
        return results;
    }

    let registry = LintRegistry::default_rules();
    let engine = LintEngine::new_with_settings(registry, LintSettings::default());

    for entry in WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "move"))
    {
        let file_path = entry.path();
        let relative_path = file_path
            .strip_prefix(path)
            .unwrap_or(file_path)
            .to_string_lossy()
            .to_string();

        // Read the file
        let source = match std::fs::read_to_string(file_path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        // Run lints
        let messages: Vec<String> = match engine.lint_source(&source) {
            Ok(diags) => diags
                .iter()
                .map(|d| {
                    format!(
                        "{}:{}: {} - {}",
                        d.span.start.row, d.span.start.column, d.lint.name, d.message
                    )
                })
                .collect(),
            Err(_) => continue,
        };

        // Collect messages
        if !messages.is_empty() {
            results.insert(relative_path, messages);
        }
    }

    results
}

/// Format results for snapshot comparison
fn format_snapshot(results: &BTreeMap<String, Vec<String>>) -> String {
    if results.is_empty() {
        return "No findings.".to_string();
    }

    let mut output = String::new();

    for (file, messages) in results {
        output.push_str(&format!("=== {} ===\n", file));
        for msg in messages {
            output.push_str(&format!("  {}\n", msg));
        }
        output.push('\n');
    }

    output
}

// ============================================================================
// Embedded Test Fixtures
// ============================================================================
//
// Instead of requiring external repos (which creates CI complexity), we embed
// representative Move code samples that exercise our lints.

mod embedded_fixtures {
    use super::*;

    /// OpenZeppelin-style Ownable pattern
    const OWNABLE_SAMPLE: &str = r#"
        module example::ownable {
            use sui::object::{Self, UID};
            use sui::tx_context::TxContext;

            /// Ownership capability - transfer to change owner
            public struct AdminCap has key, store {
                id: UID,
            }

            /// Initialize with admin cap
            public fun init(ctx: &mut TxContext) {
                let admin_cap = AdminCap {
                    id: object::new(ctx),
                };
                transfer::transfer(admin_cap, tx_context::sender(ctx));
            }
        }
    "#;

    /// Token with correct abilities (no copy+drop)
    const SAFE_TOKEN: &str = r#"
        module example::safe_token {
            use sui::object::UID;
            use sui::coin::{Self, Coin};
            use sui::balance::Balance;

            /// Treasury capability
            public struct TreasuryCap has key, store {
                id: UID,
            }

            /// Safe token - no copy or drop
            public struct SAFE_TOKEN has key {
                id: UID,
            }

            /// Token balance holder
            public struct TokenStore has key {
                id: UID,
                balance: Balance<SAFE_TOKEN>,
            }
        }
    "#;

    /// Event struct with copy+drop (legitimate use case)
    const EVENT_STRUCTS: &str = r#"
        module example::events {
            /// Transfer event - copy+drop is fine for events
            public struct TransferEvent has copy, drop {
                from: address,
                to: address,
                amount: u64,
            }

            /// Swap event
            public struct AssetSwapped has copy, drop {
                pool_id: address,
                amount_in: u64,
                amount_out: u64,
            }

            /// Deposit completed
            public struct DepositCreated has copy, drop {
                user: address,
                amount: u64,
            }
        }
    "#;

    /// Two-step ownership transfer (correct pattern)
    const TWO_STEP_TRANSFER: &str = r#"
        module example::two_step {
            use sui::object::UID;
            use std::option::{Self, Option};

            public struct Exchange has key {
                id: UID,
                admin: address,
                pending_admin: Option<address>,
            }

            /// Propose new admin - first step
            public fun propose_admin(exchange: &mut Exchange, new_admin: address, ctx: &TxContext) {
                assert!(exchange.admin == tx_context::sender(ctx), 1);
                exchange.pending_admin = option::some(new_admin);
            }

            /// Accept admin role - second step
            public fun accept_admin(exchange: &mut Exchange, ctx: &TxContext) {
                assert!(
                    option::is_some(&exchange.pending_admin) &&
                    *option::borrow(&exchange.pending_admin) == tx_context::sender(ctx),
                    2
                );
                exchange.admin = tx_context::sender(ctx);
                exchange.pending_admin = option::none();
            }
        }
    "#;

    /// OTW with drop ability (correct)
    const OTW_CORRECT: &str = r#"
        module example::token {
            use sui::coin;
            use sui::transfer;
            use sui::tx_context::TxContext;

            /// One-time witness with drop ability
            public struct MY_TOKEN has drop {}

            fun init(witness: MY_TOKEN, ctx: &mut TxContext) {
                let (treasury_cap, metadata) = coin::create_currency(
                    witness,
                    9,
                    b"MYT",
                    b"My Token",
                    b"A test token",
                    option::none(),
                    ctx
                );
                transfer::public_freeze_object(metadata);
                transfer::public_transfer(treasury_cap, tx_context::sender(ctx));
            }
        }
    "#;

    /// Modern Move patterns (should pass all style lints)
    const MODERN_PATTERNS: &str = r#"
        module example::modern {
            use sui::object::UID;
            use sui::vec_map::VecMap;
            use sui::event;

            const E_NOT_FOUND: u64 = 1;

            public struct Registry has key {
                id: UID,
                items: VecMap<address, u64>,
            }

            /// Get item count
            public fun count(registry: &Registry): u64 {
                registry.items.size()
            }

            /// Check if empty using modern method
            public fun is_empty(registry: &Registry): bool {
                registry.items.is_empty()
            }

            /// Loop pattern (correct)
            public fun process_all(registry: &mut Registry) {
                loop {
                    if registry.items.is_empty() {
                        break
                    };
                    // Process...
                }
            }
        }
    "#;

    #[test]
    fn snapshot_ownable_pattern() {
        let messages = lint_source(OWNABLE_SAMPLE);
        insta::assert_debug_snapshot!("ownable_pattern", messages);
    }

    #[test]
    fn snapshot_safe_token() {
        let messages = lint_source(SAFE_TOKEN);
        insta::assert_debug_snapshot!("safe_token", messages);
    }

    #[test]
    fn snapshot_event_structs() {
        let messages = lint_source(EVENT_STRUCTS);
        insta::assert_debug_snapshot!("event_structs", messages);
    }

    #[test]
    fn snapshot_two_step_transfer() {
        let messages = lint_source(TWO_STEP_TRANSFER);
        insta::assert_debug_snapshot!("two_step_transfer", messages);
    }

    #[test]
    fn snapshot_otw_correct() {
        let messages = lint_source(OTW_CORRECT);
        insta::assert_debug_snapshot!("otw_correct", messages);
    }

    #[test]
    fn snapshot_modern_patterns() {
        let messages = lint_source(MODERN_PATTERNS);
        insta::assert_debug_snapshot!("modern_patterns", messages);
    }
}

// ============================================================================
// Local Repository Tests
// ============================================================================
//
// These tests run against local repos if available. They're marked #[ignore]
// by default but can be run with --ignored flag.

mod local_repos {
    use super::*;

    /// Test against OpenZeppelin Sui if available locally
    #[test]
    #[ignore = "requires local openzeppelin-sui clone"]
    fn snapshot_openzeppelin_sui() {
        let path = Path::new("../../openzeppelin-sui");
        if !path.exists() {
            eprintln!("Skipping: openzeppelin-sui not found at {}", path.display());
            return;
        }

        let results = lint_directory(path);
        let snapshot = format_snapshot(&results);
        insta::assert_snapshot!("openzeppelin_sui", snapshot);
    }

    /// Test against DeepBook v3 if available
    #[test]
    #[ignore = "requires local deepbookv3 clone"]
    fn snapshot_deepbook_v3() {
        let path = Path::new("../../deepbookv3");
        if !path.exists() {
            eprintln!("Skipping: deepbookv3 not found at {}", path.display());
            return;
        }

        let results = lint_directory(path);
        let snapshot = format_snapshot(&results);
        insta::assert_snapshot!("deepbook_v3", snapshot);
    }

    /// Test against Sui framework if available
    #[test]
    #[ignore = "requires local sui clone"]
    fn snapshot_sui_framework() {
        let path = Path::new("../../sui/crates/sui-framework/packages");
        if !path.exists() {
            eprintln!("Skipping: sui-framework not found at {}", path.display());
            return;
        }

        let results = lint_directory(path);
        let snapshot = format_snapshot(&results);
        insta::assert_snapshot!("sui_framework", snapshot);
    }
}

// ============================================================================
// Regression Tests
// ============================================================================
//
// Tests for specific historical FP issues that were fixed

mod regression_tests {
    use super::lint_source;

    /// Regression: "recap" was incorrectly flagged as capability
    #[test]
    fn regression_recap_not_capability() {
        let source = r#"
            module example::summary {
                public fun daily_recap(data: &Data) {
                    transfer::share_object(Summary { recap_id: 1 });
                }
            }
        "#;

        let messages = lint_source(source);

        // Should NOT flag "recap" as capability
        for msg in &messages {
            assert!(
                !msg.to_lowercase().contains("capability"),
                "Regression: 'recap' incorrectly flagged as capability"
            );
        }
    }

    /// Regression: "AssetSwap" event was incorrectly flagged for token abilities
    #[test]
    fn regression_asset_swap_event() {
        let source = r#"
            module example::dex {
                public struct AssetSwapped has copy, drop {
                    amount_in: u64,
                    amount_out: u64,
                }
            }
        "#;

        let messages = lint_source(source);

        // Should NOT flag event structs for token abilities
        for msg in &messages {
            assert!(
                !msg.contains("excessive") && !msg.contains("abilities"),
                "Regression: Event struct incorrectly flagged for abilities"
            );
        }
    }

    /// Regression: "capacity" was incorrectly flagged as capability
    #[test]
    fn regression_capacity_not_capability() {
        let source = r#"
            module example::storage {
                public struct Pool has key {
                    id: UID,
                    capacity: u64,
                }

                public fun check_capacity(pool: &Pool) {
                    transfer::share_object(Config { max_capacity: 100 });
                }
            }
        "#;

        let messages = lint_source(source);

        // Should NOT flag "capacity" as capability
        for msg in &messages {
            assert!(
                !msg.to_lowercase().contains("capability"),
                "Regression: 'capacity' incorrectly flagged as capability"
            );
        }
    }
}
