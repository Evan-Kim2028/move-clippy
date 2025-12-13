//! Security lints based on real audit findings and published security research.
//!
//! These lints detect vulnerabilities that the Move compiler does not catch because
//! they are semantic/intent issues rather than syntax errors. Each lint includes
//! explicit source citations with verification dates.
//!
//! # References
//!
//! All security lints are backed by published audit reports and security research.
//! See `docs/SECURITY_LINTS.md` for the complete reference list.

use crate::lint::{FixDescriptor, LintCategory, LintContext, LintDescriptor, LintRule, RuleGroup};
use tree_sitter::Node;

// ============================================================================
// droppable_hot_potato - Detects hot potato structs with drop ability
// ============================================================================

/// Detects flash loan receipts and hot potato structs with the `drop` ability.
///
/// # Security References
///
/// - **Trail of Bits (2025-09-10)**: "How Sui Move rethinks flash loan security"
///   URL: https://blog.trailofbits.com/2025/09/10/how-sui-move-rethinks-flash-loan-security/
///   Verified: 2025-12-13 (DeepBookV3 FlashLoan struct analysis)
///
/// - **Mirage Audits (2025-10-01)**: "The Ability Mistakes That Will Drain Your Sui Move Protocol"
///   URL: https://www.mirageaudits.com/blog/sui-move-ability-security-mistakes
///   Verified: 2025-12-13 (Production audit findings, "The Accidental Droppable Hot Potato")
///
/// - **Sui Official Documentation**: Flash Loans in DeepBookV3
///   URL: https://docs.sui.io/standards/deepbookv3/flash-loans
///   Verified: 2025-12-13 (Hot potato pattern specification)
///
/// # Why This Matters
///
/// Adding `drop` to a hot potato silently breaks the security model.
/// The compiler accepts it as valid syntax, but attackers can then
/// borrow assets and simply drop the receipt without repaying.
///
/// # Example
///
/// ```move
/// // CRITICAL BUG - enables theft
/// struct FlashLoanReceipt has drop {
///     pool_id: ID,
///     amount: u64,
/// }
///
/// // Attacker can do this:
/// public fun exploit(pool: &mut Pool) {
///     let (stolen_coins, receipt) = borrow(pool, 1_000_000);
///     // Don't call repay - receipt gets dropped automatically!
///     transfer::public_transfer(stolen_coins, @attacker);
/// }
/// ```
///
/// # Correct Pattern
///
/// ```move
/// // No abilities = hot potato, must be consumed
/// struct FlashLoanReceipt {
///     pool_id: ID,
///     amount: u64,
/// }
/// ```
pub static DROPPABLE_HOT_POTATO: LintDescriptor = LintDescriptor {
    name: "droppable_hot_potato",
    category: LintCategory::Security,
    description: "Hot potato struct has `drop` ability, enabling theft (see: Trail of Bits 2025, Mirage Audits 2025)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
};

/// Keywords that indicate a struct is likely intended to be a hot potato.
/// These patterns come from real-world DeFi protocols and audit reports.
///
/// Note: We require the name to contain both a "hot potato indicator" keyword
/// AND NOT be an event struct (which legitimately has copy+drop).
const HOT_POTATO_KEYWORDS: &[&str] = &[
    "receipt",   // FlashLoanReceipt
    "promise",   // RepaymentPromise
    "ticket",    // BorrowTicket
    "potato",    // HotPotato (explicit)
    "obligation", // RepaymentObligation
    "voucher",   // LoanVoucher
];

/// Keywords that indicate a struct is an event (and legitimately has copy+drop).
const EVENT_KEYWORDS: &[&str] = &["event", "emitted", "log"];

pub struct DroppableHotPotatoLint;

impl LintRule for DroppableHotPotatoLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &DROPPABLE_HOT_POTATO
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        check_droppable_hot_potato(root, source, ctx);
    }
}

fn check_droppable_hot_potato(node: Node, source: &str, ctx: &mut LintContext<'_>) {
    // Look for struct definitions
    if node.kind() == "struct_definition" {
        if let Some(name_node) = node.child_by_field_name("name") {
            let struct_name = name_node
                .utf8_text(source.as_bytes())
                .unwrap_or("")
                .to_lowercase();

            // Skip if this is an event struct (events legitimately have copy+drop)
            let is_event = EVENT_KEYWORDS.iter().any(|kw| struct_name.contains(kw));
            if is_event {
                // Recurse and return early
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    check_droppable_hot_potato(child, source, ctx);
                }
                return;
            }

            // Check if this looks like a hot potato struct
            let is_hot_potato = HOT_POTATO_KEYWORDS
                .iter()
                .any(|kw| struct_name.contains(kw));

            if is_hot_potato {
                // Check if it has the drop ability
                if has_drop_ability(node, source) {
                    let original_name = name_node.utf8_text(source.as_bytes()).unwrap_or("");
                    ctx.report_node(
                        &DROPPABLE_HOT_POTATO,
                        node,
                        format!(
                            "Struct `{}` appears to be a hot potato but has `drop` ability. \
                             Hot potatoes must have no abilities to enforce consumption. \
                             See: https://blog.trailofbits.com/2025/09/10/how-sui-move-rethinks-flash-loan-security/",
                            original_name
                        ),
                    );
                }
            }
        }
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        check_droppable_hot_potato(child, source, ctx);
    }
}

/// Check if a struct definition has the `drop` ability.
fn has_drop_ability(struct_node: Node, source: &str) -> bool {
    // Look for ability_decls child which contains the abilities
    let mut cursor = struct_node.walk();
    for child in struct_node.children(&mut cursor) {
        if child.kind() == "ability_decls" {
            let abilities_text = child.utf8_text(source.as_bytes()).unwrap_or("");
            // Check for "drop" keyword in the abilities
            // Abilities are in format: "has copy, drop, store" or "has drop"
            return abilities_text
                .split(|c: char| c == ',' || c.is_whitespace())
                .any(|ability| ability.trim().eq_ignore_ascii_case("drop"));
        }
    }
    false
}

// ============================================================================
// excessive_token_abilities - Detects tokens with copy+drop (infinite money)
// ============================================================================

/// Detects token/asset structs with both `copy` and `drop` abilities.
///
/// # Security References
///
/// - **Mirage Audits (2025-10-01)**: "The Ability Combination Nightmare"
///   URL: https://www.mirageaudits.com/blog/sui-move-ability-security-mistakes
///   Verified: 2025-12-13 (Documents the copy+drop vulnerability)
///
/// - **MoveBit (2023-07-07)**: "Avoid giving excessive abilities to structs"
///   URL: https://movebit.xyz/blog/post/Sui-Objects-Security-Principles-and-Best-Practices.html
///   Verified: 2025-12-13 (Still valid - fundamental Move security pattern)
///
/// # Why This Matters
///
/// A struct with both `copy` and `drop` can be:
/// 1. **Duplicated infinitely** (via `copy`)
/// 2. **Destroyed at will** (via `drop`)
/// 3. **Created from thin air** by copying and modifying
///
/// This is the "infinite money glitch" for token implementations.
///
/// # Example
///
/// ```move
/// // CRITICAL VULNERABILITY - DO NOT USE
/// struct TokenCoin has copy, drop, store {
///     amount: u64,
/// }
///
/// // Attacker can duplicate tokens:
/// let original = get_token();
/// let copy1 = original;  // copy happens
/// let copy2 = original;  // another copy
/// // Now attacker has 3x the tokens!
/// ```
///
/// # Correct Pattern
///
/// ```move
/// // Assets should ONLY have key + store
/// struct TokenCoin has key, store {
///     id: UID,
///     balance: Balance,
/// }
/// ```
pub static EXCESSIVE_TOKEN_ABILITIES: LintDescriptor = LintDescriptor {
    name: "excessive_token_abilities",
    category: LintCategory::Security,
    description: "Token struct has copy+drop abilities, enabling infinite duplication (see: Mirage Audits 2025, MoveBit 2023)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
};

/// Keywords that indicate a struct represents a valuable asset.
/// These patterns come from real-world token implementations.
///
/// Note: We exclude:
/// - Event structs (legitimately have copy+drop)
/// - Key structs used as map keys (legitimately have copy+drop)
const TOKEN_KEYWORDS: &[&str] = &[
    "token",   // MyToken
    "coin",    // GameCoin
    "asset",   // DigitalAsset
    "share",   // PoolShare
    "stake",   // StakePosition
];

/// Keywords that indicate a struct is a key/event, not a valuable asset.
const NON_ASSET_SUFFIXES: &[&str] = &["key", "event", "log", "data", "info", "params"];

pub struct ExcessiveTokenAbilitiesLint;

impl LintRule for ExcessiveTokenAbilitiesLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &EXCESSIVE_TOKEN_ABILITIES
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        check_excessive_token_abilities(root, source, ctx);
    }
}

fn check_excessive_token_abilities(node: Node, source: &str, ctx: &mut LintContext<'_>) {
    // Look for struct definitions
    if node.kind() == "struct_definition" {
        if let Some(name_node) = node.child_by_field_name("name") {
            let struct_name = name_node
                .utf8_text(source.as_bytes())
                .unwrap_or("")
                .to_lowercase();

            // Skip if this is a non-asset struct (events, keys, data, etc.)
            let is_non_asset = NON_ASSET_SUFFIXES
                .iter()
                .any(|suffix| struct_name.ends_with(suffix) || struct_name.contains(&format!("{}_", suffix)));
            if is_non_asset {
                // Recurse and return early
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    check_excessive_token_abilities(child, source, ctx);
                }
                return;
            }

            // Check if this looks like a token/asset struct
            let is_token = TOKEN_KEYWORDS.iter().any(|kw| struct_name.contains(kw));

            if is_token {
                // Check if it has both copy AND drop abilities
                let (has_copy, has_drop) = get_copy_drop_abilities(node, source);

                if has_copy && has_drop {
                    let original_name = name_node.utf8_text(source.as_bytes()).unwrap_or("");
                    ctx.report_node(
                        &EXCESSIVE_TOKEN_ABILITIES,
                        node,
                        format!(
                            "Struct `{}` appears to be a token/asset but has both `copy` and `drop` abilities. \
                             This enables infinite duplication. Assets should only have `key` and `store`. \
                             See: https://www.mirageaudits.com/blog/sui-move-ability-security-mistakes",
                            original_name
                        ),
                    );
                }
            }
        }
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        check_excessive_token_abilities(child, source, ctx);
    }
}

/// Check if a struct definition has copy and/or drop abilities.
/// Returns (has_copy, has_drop).
fn get_copy_drop_abilities(struct_node: Node, source: &str) -> (bool, bool) {
    let mut has_copy = false;
    let mut has_drop = false;

    // Look for ability_decls child which contains the abilities
    let mut cursor = struct_node.walk();
    for child in struct_node.children(&mut cursor) {
        if child.kind() == "ability_decls" {
            let abilities_text = child.utf8_text(source.as_bytes()).unwrap_or("");
            // Parse abilities from format: "has copy, drop, store"
            for ability in abilities_text.split(|c: char| c == ',' || c.is_whitespace()) {
                let ability = ability.trim().to_lowercase();
                if ability == "copy" {
                    has_copy = true;
                }
                if ability == "drop" {
                    has_drop = true;
                }
            }
        }
    }

    (has_copy, has_drop)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lint::{LintContext, LintSettings};
    use crate::parser::parse_source;

    fn lint_source(source: &str) -> Vec<String> {
        let tree = parse_source(source).unwrap();
        let mut ctx = LintContext::new(source, LintSettings::default());

        let lint = DroppableHotPotatoLint;
        lint.check(tree.root_node(), source, &mut ctx);

        let lint2 = ExcessiveTokenAbilitiesLint;
        lint2.check(tree.root_node(), source, &mut ctx);

        ctx.into_diagnostics()
            .into_iter()
            .map(|d| d.message)
            .collect()
    }

    #[test]
    fn test_droppable_hot_potato_detected() {
        let source = r#"
            module example::flash {
                struct FlashLoanReceipt has drop {
                    pool_id: ID,
                    amount: u64,
                }
            }
        "#;

        let messages = lint_source(source);
        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("FlashLoanReceipt"));
        assert!(messages[0].contains("hot potato"));
    }

    #[test]
    fn test_hot_potato_no_abilities_ok() {
        let source = r#"
            module example::flash {
                struct FlashLoanReceipt {
                    pool_id: ID,
                    amount: u64,
                }
            }
        "#;

        let messages = lint_source(source);
        assert!(messages.is_empty());
    }

    #[test]
    fn test_excessive_token_abilities_detected() {
        let source = r#"
            module example::token {
                struct TokenCoin has copy, drop, store {
                    amount: u64,
                }
            }
        "#;

        let messages = lint_source(source);
        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("TokenCoin"));
        assert!(messages[0].contains("copy"));
        assert!(messages[0].contains("drop"));
    }

    #[test]
    fn test_token_with_key_store_ok() {
        let source = r#"
            module example::token {
                struct TokenCoin has key, store {
                    id: UID,
                    balance: u64,
                }
            }
        "#;

        let messages = lint_source(source);
        assert!(messages.is_empty());
    }

    #[test]
    fn test_non_token_struct_with_copy_drop_ok() {
        // Regular data structs can have copy+drop
        let source = r#"
            module example::data {
                struct Point has copy, drop {
                    x: u64,
                    y: u64,
                }
            }
        "#;

        let messages = lint_source(source);
        assert!(messages.is_empty());
    }
}
