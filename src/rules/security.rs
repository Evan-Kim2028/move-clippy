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
                let has_drop = has_drop_ability(node, source);
                let has_copy = has_copy_ability(node, source);
                
                // Skip if it has BOTH copy AND drop - this is likely a data transfer object
                // Hot potatoes should have ONLY drop (or no abilities at all)
                // A struct with copy+drop is typically used for events or tracking, not enforcement
                if has_drop && !has_copy {
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

/// Check if a struct definition has the `copy` ability.
fn has_copy_ability(struct_node: Node, source: &str) -> bool {
    // Look for ability_decls child which contains the abilities
    let mut cursor = struct_node.walk();
    for child in struct_node.children(&mut cursor) {
        if child.kind() == "ability_decls" {
            let abilities_text = child.utf8_text(source.as_bytes()).unwrap_or("");
            // Check for "copy" keyword in the abilities
            return abilities_text
                .split(|c: char| c == ',' || c.is_whitespace())
                .any(|ability| ability.trim().eq_ignore_ascii_case("copy"));
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
/// Includes past-tense event naming patterns (e.g., "Created", "Updated").
const NON_ASSET_SUFFIXES: &[&str] = &[
    // Explicit non-asset suffixes
    "key", "event", "log", "data", "info", "params",
    // Past-tense event naming patterns (common event naming convention)
    "created", "updated", "deleted", "transferred", "minted", "burned",
    "deposited", "withdrawn", "swapped", "claimed", "staked", "unstaked",
    "swap",  // AssetSwap is a common event name
];

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

// ============================================================================
// shared_capability - Detects capability objects being shared publicly
// ============================================================================

/// Detects capability objects (AdminCap, OwnerCap, etc.) being shared via
/// `transfer::share_object` or `transfer::public_share_object`.
///
/// # Security References
///
/// - **Sui Official Documentation**: "Object Ownership"
///   URL: https://docs.sui.io/concepts/object-ownership
///   Verified: 2025-12-13 (Capability pattern best practices)
///
/// - **OtterSec Audits**: Multiple findings across DeFi protocols
///   Common finding: "AdminCap shared instead of transferred to admin"
///
/// - **MoveBit (2023-07-07)**: "Sui Objects Security Principles"
///   URL: https://movebit.xyz/blog/post/Sui-Objects-Security-Principles-and-Best-Practices.html
///   Verified: 2025-12-13 (Capability object patterns)
///
/// # Why This Matters
///
/// Capabilities are meant to grant exclusive administrative rights to a single
/// owner. Sharing a capability makes it publicly accessible, meaning ANYONE
/// can call privileged functions. This is one of the most common security bugs
/// in Sui Move contracts.
///
/// # Example
///
/// ```move
/// // CRITICAL BUG - Anyone can use the AdminCap!
/// public fun init(ctx: &mut TxContext) {
///     let admin_cap = AdminCap { id: object::new(ctx) };
///     transfer::share_object(admin_cap);  // BUG!
/// }
///
/// // Now any attacker can call admin-only functions:
/// public fun drain_treasury(cap: &AdminCap, treasury: &mut Treasury) {
///     // Cap check passes because it's shared!
///     transfer_all_funds(treasury, @attacker);
/// }
/// ```
///
/// # Correct Pattern
///
/// ```move
/// public fun init(ctx: &mut TxContext) {
///     let admin_cap = AdminCap { id: object::new(ctx) };
///     // Transfer to the deployer (tx sender)
///     transfer::transfer(admin_cap, tx_context::sender(ctx));
/// }
/// ```
pub static SHARED_CAPABILITY: LintDescriptor = LintDescriptor {
    name: "shared_capability",
    category: LintCategory::Security,
    description: "Capability object is being shared, making it publicly accessible (see: Sui security best practices)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
};

/// Keywords that indicate a struct/variable is a capability object.
/// These MUST be at the end of the name (word boundary) to avoid false positives
/// like "Capacity" or "Capital".
const CAPABILITY_SUFFIXES: &[&str] = &[
    "Cap",          // AdminCap, OwnerCap, MintCap
    "Capability",   // AdminCapability (verbose form)
];

/// Full capability names to match exactly (case-insensitive after first char).
const CAPABILITY_NAMES: &[&str] = &[
    "AdminCap",
    "OwnerCap",
    "TreasuryCap",
    "MintCap",
    "BurnCap",
    "PauseCap",
    "UpgradeCap",
    "TransferCap",
    "FreezerCap",
    "DenyCap",
    "GovernorCap",
    "ManagerCap",
    "OperatorCap",
];

/// Functions that share objects publicly.
const SHARE_FUNCTIONS: &[&str] = &[
    "share_object",
    "public_share_object",
];

pub struct SharedCapabilityLint;

impl LintRule for SharedCapabilityLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &SHARED_CAPABILITY
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        check_shared_capability(root, source, ctx);
    }
}

fn check_shared_capability(node: Node, source: &str, ctx: &mut LintContext<'_>) {
    // Look for function calls
    if node.kind() == "call_expression" || node.kind() == "macro_call" {
        let node_text = node.utf8_text(source.as_bytes()).unwrap_or("");
        
        // Check if this is a share_object or public_share_object call
        let is_share_call = SHARE_FUNCTIONS.iter().any(|func| {
            node_text.contains(&format!("{}(", func)) || 
            node_text.contains(&format!("{}::", func))
        });
        
        if is_share_call {
            // Check if the argument looks like a capability
            if is_capability_argument(node_text) {
                ctx.report_node(
                    &SHARED_CAPABILITY,
                    node,
                    format!(
                        "Capability object appears to be shared via `share_object`. \
                         Capabilities should be transferred to a specific owner, not shared publicly. \
                         Use `transfer::transfer(cap, owner_address)` instead."
                    ),
                );
            }
        }
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        check_shared_capability(child, source, ctx);
    }
}

/// Check if the argument to share_object looks like a capability object.
fn is_capability_argument(call_text: &str) -> bool {
    // Check for common capability suffixes/names in the call
    let call_lower = call_text.to_lowercase();
    
    // Check exact capability names (case-insensitive)
    for name in CAPABILITY_NAMES {
        if call_lower.contains(&name.to_lowercase()) {
            return true;
        }
    }
    
    // Check for "Cap" suffix but NOT "Capacity", "Capital", "Recap", etc.
    // We need to check for word boundaries on BOTH sides of "cap"
    for suffix in CAPABILITY_SUFFIXES {
        let suffix_lower = suffix.to_lowercase();
        let mut search_pos = 0;
        
        while let Some(pos) = call_lower[search_pos..].find(&suffix_lower) {
            let actual_pos = search_pos + pos;
            
            // Check char BEFORE "cap" - must be word boundary (not alphabetic)
            // This prevents matching "recap", "escape", etc.
            let char_before = if actual_pos > 0 {
                call_lower.chars().nth(actual_pos - 1)
            } else {
                None
            };
            
            let valid_prefix = match char_before {
                None => true, // Start of string is valid
                Some(c) => !c.is_alphabetic(), // Non-alpha before is valid (e.g., "_cap", " cap")
            };
            
            if !valid_prefix {
                // Move past this match and continue searching
                search_pos = actual_pos + suffix_lower.len();
                continue;
            }
            
            // Check char AFTER "cap" - must be word boundary (not alphabetic)
            // This prevents matching "capacity", "capital", etc.
            let after_pos = actual_pos + suffix_lower.len();
            if after_pos >= call_lower.len() {
                // Suffix at end of string is valid
                return true;
            }
            
            let next_char = call_lower.chars().nth(after_pos);
            if let Some(c) = next_char {
                if !c.is_alphabetic() {
                    return true;
                }
            }
            
            // Move past this match and continue searching
            search_pos = actual_pos + suffix_lower.len();
        }
    }
    
    false
}

// ============================================================================
// suspicious_overflow_check - Detects potentially incorrect manual overflow checks
// ============================================================================

/// Detects suspicious patterns in manual overflow checking code.
///
/// # Security References
///
/// - **Cetus $223M Hack (2025-05-22)**: Integer overflow in integer_mate library
///   URL: https://x.com/paborji/status/1925573106270621989
///   Verified: 2025-12-13 (Analysis of vulnerable code vs fixed code)
///
/// # Why This Matters
///
/// The Cetus hack involved TWO bugs in a manual overflow check:
/// 1. Wrong mask: `0xffffffffffffffff << 192` (wrong value) vs `1 << 192` (correct)
/// 2. Wrong comparison: `>` (wrong) vs `>=` (correct)
///
/// Manual overflow checks are notoriously error-prone. When we see:
/// - Functions named "checked_*", "safe_*", or similar
/// - Combined with bit shifts and manual comparisons
///
/// We flag it as a code smell that warrants careful review.
///
/// # Example (from Cetus hack)
///
/// ```move
/// // VULNERABLE - $223M lost!
/// public fun checked_shlw(n: u256): (u256, bool) {
///     let mask = 0xffffffffffffffff << 192;  // Wrong mask!
///     if (n > mask) { ... }                   // Wrong operator (> vs >=)
///     (n << 64, false)
/// }
///
/// // FIXED
/// public fun checked_shlw(n: u256): (u256, bool) {
///     let mask = 1 << 192;                   // Correct mask
///     if (n >= mask) { ... }                 // Correct operator
///     (n << 64, false)
/// }
/// ```
///
/// # Note
///
/// This lint is in PREVIEW mode because it has moderate false positive risk.
/// It's meant as an advisory to flag code that needs extra scrutiny.
pub static SUSPICIOUS_OVERFLOW_CHECK: LintDescriptor = LintDescriptor {
    name: "suspicious_overflow_check",
    category: LintCategory::Security,
    description: "Manual overflow check detected - these are error-prone. Consider using built-in checked arithmetic (see: Cetus $223M hack)",
    group: RuleGroup::Preview,  // Preview due to FP risk
    fix: FixDescriptor::none(),
};

/// Function name patterns that indicate overflow/bounds checking.
const OVERFLOW_CHECK_PATTERNS: &[&str] = &[
    "checked_",
    "safe_",
    "_checked",
    "_safe",
    "overflow",
    "underflow",
];

pub struct SuspiciousOverflowCheckLint;

impl LintRule for SuspiciousOverflowCheckLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &SUSPICIOUS_OVERFLOW_CHECK
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        check_suspicious_overflow(root, source, ctx);
    }
}

fn check_suspicious_overflow(node: Node, source: &str, ctx: &mut LintContext<'_>) {
    // Look for function definitions
    if node.kind() == "function_definition" {
        if let Some(name_node) = node.child_by_field_name("name") {
            let func_name = name_node.utf8_text(source.as_bytes()).unwrap_or("");
            let func_name_lower = func_name.to_lowercase();
            
            // Check if function name suggests overflow checking
            let is_overflow_function = OVERFLOW_CHECK_PATTERNS
                .iter()
                .any(|pat| func_name_lower.contains(pat));
            
            if is_overflow_function {
                // Check if the function body contains bit shifts AND comparisons
                let func_text = node.utf8_text(source.as_bytes()).unwrap_or("");
                let has_bit_shift = func_text.contains("<<") || func_text.contains(">>");
                let has_comparison = func_text.contains(" > ") || 
                                    func_text.contains(" >= ") ||
                                    func_text.contains(" < ") ||
                                    func_text.contains(" <= ");
                let has_hex_constant = func_text.contains("0x");
                
                // If we have bit shifts AND comparisons AND hex constants,
                // this is very likely a manual overflow check
                if has_bit_shift && has_comparison && has_hex_constant {
                    ctx.report_node(
                        &SUSPICIOUS_OVERFLOW_CHECK,
                        node,
                        format!(
                            "Function `{}` appears to implement manual overflow checking with bit shifts. \
                             Manual overflow checks are error-prone (see Cetus $223M hack). \
                             Consider: 1) Using built-in checked arithmetic, 2) Adding extensive tests, \
                             3) Getting this code audited.",
                            func_name
                        ),
                    );
                }
            }
        }
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        check_suspicious_overflow(child, source, ctx);
    }
}

// ============================================================================
// stale_oracle_price - Detects usage of unsafe oracle price functions
// ============================================================================

/// Detects usage of `get_price_unsafe` which may return stale/outdated prices.
///
/// # Security References
///
/// - **Bluefin MoveBit Audit (2024-05)**: "Oracle does not check outdated prices"
///   Finding: Protocol used `pyth::get_price_unsafe` which doesn't guarantee freshness
///   URL: https://www.movebit.xyz/blog/post/Bluefin-vulnerabilities-explanation-1.html
///   Verified: 2025-12-13
///
/// - **Pyth Network Documentation**: Explicitly warns about `get_price_unsafe`
///   URL: https://docs.pyth.network/
///
/// # Why This Matters
///
/// `get_price_unsafe` is explicitly named "unsafe" because it may return stale prices.
/// In DeFi, stale prices can lead to:
/// - Incorrect liquidations (liquidating healthy positions)
/// - Arbitrage opportunities against the protocol
/// - Loss of user funds
///
/// # Example
///
/// ```move
/// // VULNERABLE - may return stale price
/// let price = pyth::get_price_unsafe(price_info);
/// let value = amount * price;  // Could be using hours-old price!
///
/// // CORRECT - ensures price freshness
/// let price = pyth::get_price_no_older_than(price_info, MAX_PRICE_AGE);
/// ```
///
/// # Note
///
/// This lint has near-zero false positives because the function is explicitly
/// named "unsafe" in the Pyth API.
pub static STALE_ORACLE_PRICE: LintDescriptor = LintDescriptor {
    name: "stale_oracle_price",
    category: LintCategory::Security,
    description: "Using get_price_unsafe may return stale prices (see: Bluefin Audit 2024, Pyth docs)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
};

/// Function names that indicate potentially stale oracle price retrieval.
const UNSAFE_PRICE_FUNCTIONS: &[&str] = &[
    "get_price_unsafe",
    "price_unsafe",
];

pub struct StaleOraclePriceLint;

impl LintRule for StaleOraclePriceLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &STALE_ORACLE_PRICE
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        check_stale_oracle_price(root, source, ctx);
    }
}

fn check_stale_oracle_price(node: Node, source: &str, ctx: &mut LintContext<'_>) {
    // Look for function calls
    if node.kind() == "call_expression" {
        let node_text = node.utf8_text(source.as_bytes()).unwrap_or("");
        
        // Check if this calls an unsafe price function
        let is_unsafe_price_call = UNSAFE_PRICE_FUNCTIONS.iter().any(|func| {
            node_text.contains(func)
        });
        
        if is_unsafe_price_call {
            ctx.report_node(
                &STALE_ORACLE_PRICE,
                node,
                "Using `get_price_unsafe` may return stale/outdated oracle prices. \
                 Consider using `get_price_no_older_than` with an appropriate max age \
                 to ensure price freshness. See: Pyth documentation."
                    .to_string(),
            );
        }
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        check_stale_oracle_price(child, source, ctx);
    }
}

// ============================================================================
// single_step_ownership_transfer - Detects dangerous single-step admin transfers
// ============================================================================

/// Detects single-step ownership/admin transfer patterns that are dangerous.
///
/// # Security References
///
/// - **Bluefin MoveBit Audit (2024-05)**: "Dangerous Single-Step Ownership Transfer"
///   Finding: Admin transfer functions had no confirmation step
///   URL: https://www.movebit.xyz/blog/post/Bluefin-vulnerabilities-explanation-1.html
///   Verified: 2025-12-13
///
/// - **OpenZeppelin (Ethereum)**: Two-step ownership transfer is best practice
///   The Ownable2Step pattern is widely adopted in production contracts.
///
/// # Why This Matters
///
/// Single-step ownership transfer is dangerous because:
/// 1. A typo in the new admin address permanently locks out admin access
/// 2. No confirmation that the new admin can actually receive/use the role
/// 3. No way to cancel an accidental transfer
///
/// # Example
///
/// ```move
/// // VULNERABLE - single step, no confirmation
/// public fun transfer_admin(exchange: &mut Exchange, new_admin: address) {
///     exchange.admin = new_admin;  // Typo = permanent loss!
/// }
///
/// // CORRECT - two-step with confirmation
/// public fun propose_admin(exchange: &mut Exchange, new_admin: address) {
///     exchange.pending_admin = option::some(new_admin);
/// }
///
/// public fun accept_admin(exchange: &mut Exchange, ctx: &TxContext) {
///     assert!(exchange.pending_admin == option::some(sender(ctx)), E_NOT_PENDING);
///     exchange.admin = sender(ctx);
///     exchange.pending_admin = option::none();
/// }
/// ```
pub static SINGLE_STEP_OWNERSHIP_TRANSFER: LintDescriptor = LintDescriptor {
    name: "single_step_ownership_transfer",
    category: LintCategory::Security,
    description: "Single-step ownership transfer is dangerous - use two-step pattern (see: OpenZeppelin Ownable2Step)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
};

/// Function name patterns that suggest admin/ownership transfer.
const OWNERSHIP_TRANSFER_PATTERNS: &[&str] = &[
    "transfer_admin",
    "set_admin",
    "change_admin",
    "update_admin",
    "transfer_owner",
    "set_owner", 
    "change_owner",
    "update_owner",
    "transfer_authority",
    "set_authority",
];

/// Patterns that indicate a two-step transfer (safe).
const TWO_STEP_PATTERNS: &[&str] = &[
    "pending",
    "propose",
    "accept",
    "claim",
];

pub struct SingleStepOwnershipTransferLint;

impl LintRule for SingleStepOwnershipTransferLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &SINGLE_STEP_OWNERSHIP_TRANSFER
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        check_single_step_ownership(root, source, ctx);
    }
}

fn check_single_step_ownership(node: Node, source: &str, ctx: &mut LintContext<'_>) {
    // Look for function definitions
    if node.kind() == "function_definition" {
        if let Some(name_node) = node.child_by_field_name("name") {
            let func_name = name_node.utf8_text(source.as_bytes()).unwrap_or("");
            let func_name_lower = func_name.to_lowercase();
            
            // Check if function name suggests ownership transfer
            let is_ownership_transfer = OWNERSHIP_TRANSFER_PATTERNS
                .iter()
                .any(|pat| func_name_lower.contains(pat));
            
            if is_ownership_transfer {
                let func_text = node.utf8_text(source.as_bytes()).unwrap_or("");
                let func_text_lower = func_text.to_lowercase();
                
                // Check if this appears to be a two-step pattern (safe)
                let is_two_step = TWO_STEP_PATTERNS
                    .iter()
                    .any(|pat| func_text_lower.contains(pat));
                
                // If it's a transfer function but doesn't use two-step pattern, flag it
                if !is_two_step {
                    // Additional check: make sure there's an assignment to admin/owner field
                    let has_admin_assignment = func_text_lower.contains(".admin =") ||
                                               func_text_lower.contains(".owner =") ||
                                               func_text_lower.contains(".authority =");
                    
                    if has_admin_assignment {
                        ctx.report_node(
                            &SINGLE_STEP_OWNERSHIP_TRANSFER,
                            node,
                            format!(
                                "Function `{}` appears to implement single-step ownership transfer. \
                                 This is dangerous - a typo in the new address permanently locks out admin access. \
                                 Consider implementing a two-step pattern: 1) propose_admin sets pending_admin, \
                                 2) accept_admin requires the new admin to confirm.",
                                func_name
                            ),
                        );
                    }
                }
            }
        }
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        check_single_step_ownership(child, source, ctx);
    }
}

// ============================================================================
// unchecked_coin_split - Detects coin::split without balance validation
// ============================================================================

/// Detects `coin::split` calls that may split more than the coin's balance.
///
/// # Security References
///
/// - **MoveBit (2023-07-07)**: "Sui Objects Security Principles and Best Practices"
///   URL: https://movebit.xyz/blog/post/Sui-Objects-Security-Principles-and-Best-Practices.html
///   Verified: 2024-12-13 (Balance validation best practices)
///
/// - **Sui Standard Library**: coin::split panics if amount > balance
///   URL: https://docs.sui.io/references/framework/sui/coin
///
/// # Why This Matters
///
/// `coin::split` will abort if you try to split more than the coin's balance.
/// This can cause transaction failures if the amount isn't validated first.
/// While not a security vulnerability per se, it can cause DoS or UX issues.
///
/// # Example
///
/// ```move
/// // RISKY - Will abort if user_coin.value < amount
/// public fun withdraw(user_coin: &mut Coin<SUI>, amount: u64, ctx: &mut TxContext): Coin<SUI> {
///     coin::split(user_coin, amount, ctx)
/// }
/// ```
///
/// # Correct Pattern
///
/// ```move
/// public fun withdraw(user_coin: &mut Coin<SUI>, amount: u64, ctx: &mut TxContext): Coin<SUI> {
///     assert!(coin::value(user_coin) >= amount, E_INSUFFICIENT_BALANCE);
///     coin::split(user_coin, amount, ctx)
/// }
/// ```
pub static UNCHECKED_COIN_SPLIT: LintDescriptor = LintDescriptor {
    name: "unchecked_coin_split",
    category: LintCategory::Security,
    description: "coin::split without prior balance check may abort unexpectedly",
    group: RuleGroup::Preview, // Preview since it may have FPs
    fix: FixDescriptor::none(),
};

pub struct UncheckedCoinSplitLint;

impl LintRule for UncheckedCoinSplitLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &UNCHECKED_COIN_SPLIT
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        check_unchecked_coin_split(root, source, ctx);
    }
}

fn check_unchecked_coin_split(node: Node, source: &str, ctx: &mut LintContext<'_>) {
    // Look for function definitions
    if node.kind() == "function_definition" {
        let func_text = node.utf8_text(source.as_bytes()).unwrap_or("");
        let func_text_lower = func_text.to_lowercase();
        
        // Check if function contains coin::split
        if func_text_lower.contains("coin::split") || func_text_lower.contains("split(") {
            // Check if there's a balance check before the split
            // Look for patterns like: coin::value, .value(), >= amount, > amount
            let has_balance_check = func_text_lower.contains("coin::value") ||
                                    func_text_lower.contains(".value()") ||
                                    func_text_lower.contains("balance::value") ||
                                    func_text_lower.contains(">= amount") ||
                                    func_text_lower.contains("> amount") ||
                                    func_text_lower.contains("assert!");
            
            if !has_balance_check {
                // Get function name for reporting
                let func_name = node.child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                    .unwrap_or("unknown");
                
                ctx.report_node(
                    &UNCHECKED_COIN_SPLIT,
                    node,
                    format!(
                        "Function `{}` uses coin::split without an apparent balance check. \
                         Consider adding `assert!(coin::value(&coin) >= amount, E_INSUFFICIENT)` \
                         to provide a clearer error message than the default abort.",
                        func_name
                    ),
                );
            }
        }
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        check_unchecked_coin_split(child, source, ctx);
    }
}

// ============================================================================
// missing_witness_drop - Detects OTW witness without drop ability
// ============================================================================

/// Detects one-time witness (OTW) structs that are missing the `drop` ability.
///
/// # Security References
///
/// - **Sui Documentation**: "One-Time Witness"
///   URL: https://docs.sui.io/concepts/sui-move-concepts/one-time-witness
///   Verified: 2024-12-13 (OTW must have drop)
///
/// - **MoveBit (2023)**: "Sui Move Security Best Practices"
///   URL: https://movebit.xyz/blog/post/Sui-Objects-Security-Principles-and-Best-Practices.html
///
/// # Why This Matters
///
/// A one-time witness (OTW) is used to prove that code is being run in the
/// module's `init` function. The OTW struct MUST have `drop` so it can be
/// consumed after use. Without `drop`, the witness cannot be properly destroyed.
///
/// # Example
///
/// ```move
/// // BAD - OTW without drop cannot be consumed
/// struct MY_TOKEN {}
/// 
/// fun init(witness: MY_TOKEN, ctx: &mut TxContext) {
///     // witness cannot be dropped after use!
/// }
/// ```
///
/// # Correct Pattern
///
/// ```move
/// // GOOD - OTW with drop
/// struct MY_TOKEN has drop {}
/// 
/// fun init(witness: MY_TOKEN, ctx: &mut TxContext) {
///     // witness is dropped automatically
/// }
/// ```
pub static MISSING_WITNESS_DROP: LintDescriptor = LintDescriptor {
    name: "missing_witness_drop",
    category: LintCategory::Security,
    description: "One-time witness struct missing `drop` ability (see: Sui OTW pattern docs)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
};

pub struct MissingWitnessDropLint;

impl LintRule for MissingWitnessDropLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &MISSING_WITNESS_DROP
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        check_missing_witness_drop(root, source, ctx);
    }
}

fn check_missing_witness_drop(node: Node, source: &str, ctx: &mut LintContext<'_>) {
    // Look for struct definitions
    if node.kind() == "struct_definition" {
        if let Some(name_node) = node.child_by_field_name("name") {
            let struct_name = name_node
                .utf8_text(source.as_bytes())
                .unwrap_or("");
            
            // OTW pattern: SCREAMING_SNAKE_CASE, same as module name, empty body
            // Check if it looks like an OTW (all uppercase with underscores)
            let is_screaming_case = struct_name.chars().all(|c| c.is_uppercase() || c == '_');
            
            if is_screaming_case && !struct_name.is_empty() {
                // Check if it has empty body (no fields)
                let struct_text = node.utf8_text(source.as_bytes()).unwrap_or("");
                let has_empty_body = struct_text.contains("{}") || 
                                     struct_text.contains("{ }");
                
                if has_empty_body {
                    // Check if it has drop ability
                    let has_drop = has_drop_ability(node, source);
                    
                    if !has_drop {
                        ctx.report_node(
                            &MISSING_WITNESS_DROP,
                            node,
                            format!(
                                "Struct `{}` appears to be a one-time witness (OTW) but is missing \
                                 the `drop` ability. OTW structs must have `drop` so they can be \
                                 consumed after use in the init function. Add `has drop` to the struct.",
                                struct_name
                            ),
                        );
                    }
                }
            }
        }
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        check_missing_witness_drop(child, source, ctx);
    }
}

// ============================================================================
// public_random_access - Detects public functions returning Random
// ============================================================================

/// Detects public functions that expose or return `Random` objects.
///
/// # Security References
///
/// - **Sui Documentation**: "Randomness"
///   URL: https://docs.sui.io/guides/developer/advanced/randomness
///   Verified: 2024-12-13 (Random must be private)
///
/// - **Sui Linter**: `public_random` built-in lint
///   The Move compiler warns about this, but we provide additional context.
///
/// # Why This Matters
///
/// `Random` objects should never be exposed publicly because:
/// 1. Validators can see the random value before including the transaction
/// 2. This enables front-running and manipulation of random outcomes
/// 3. Random values should only be consumed within the same PTB
///
/// # Example
///
/// ```move
/// // BAD - Exposes random value
/// public fun get_random(r: &Random): u64 {
///     random::new_generator(r).generate_u64()
/// }
/// ```
///
/// # Correct Pattern
///
/// ```move
/// // GOOD - Random consumed internally
/// entry fun flip_coin(r: &Random, ctx: &mut TxContext) {
///     let gen = random::new_generator(r, ctx);
///     let result = gen.generate_bool();
///     // Use result internally, don't return it
/// }
/// ```
pub static PUBLIC_RANDOM_ACCESS: LintDescriptor = LintDescriptor {
    name: "public_random_access",
    category: LintCategory::Security,
    description: "Public function exposes Random object, enabling front-running (see: Sui randomness docs)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
};

pub struct PublicRandomAccessLint;

impl LintRule for PublicRandomAccessLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &PUBLIC_RANDOM_ACCESS
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        check_public_random_access(root, source, ctx);
    }
}

fn check_public_random_access(node: Node, source: &str, ctx: &mut LintContext<'_>) {
    // Look for function definitions
    if node.kind() == "function_definition" {
        let func_text = node.utf8_text(source.as_bytes()).unwrap_or("");
        
        // Check if function is public (not entry)
        let is_public = func_text.starts_with("public fun") || 
                        func_text.contains("public fun ");
        let is_entry = func_text.contains("entry ");
        
        // Entry functions are OK because they can't be composed
        if is_public && !is_entry {
            // Check if function takes Random as parameter or returns random value
            let func_lower = func_text.to_lowercase();
            let has_random_param = func_lower.contains("&random") || 
                                   func_lower.contains(": random");
            let returns_random = func_lower.contains("-> u64") && 
                                 (func_lower.contains("generate") || func_lower.contains("random"));
            
            if has_random_param || returns_random {
                let func_name = node.child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                    .unwrap_or("unknown");
                
                ctx.report_node(
                    &PUBLIC_RANDOM_ACCESS,
                    node,
                    format!(
                        "Function `{}` is public and exposes Random. This enables front-running \
                         attacks where validators can see random values before inclusion. \
                         Use `entry` functions or consume random values internally.",
                        func_name
                    ),
                );
            }
        }
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        check_public_random_access(child, source, ctx);
    }
}

// ============================================================================
// unbounded_vector_growth - Detects vector operations without size limits
// ============================================================================

/// Detects vector push operations that could lead to unbounded growth.
///
/// # Security References
///
/// - **General Smart Contract Security**: Unbounded data structures are a DoS vector
/// - **Sui Gas Model**: Large vectors consume more gas for storage and iteration
///
/// # Why This Matters
///
/// Vectors that grow without bounds can:
/// 1. **DoS**: Make operations too expensive to execute
/// 2. **Gas exhaustion**: Iteration over large vectors exceeds gas limits
/// 3. **Storage bloat**: Permanently increase on-chain storage costs
///
/// # Example
///
/// ```move
/// // BAD - No limit on vector size
/// public fun add_member(group: &mut Group, member: address) {
///     vector::push_back(&mut group.members, member);
/// }
/// ```
///
/// # Correct Pattern
///
/// ```move
/// const MAX_MEMBERS: u64 = 1000;
///
/// public fun add_member(group: &mut Group, member: address) {
///     assert!(vector::length(&group.members) < MAX_SIZE, E_TOO_MANY);
///     vector::push_back(&mut group.members, member);
/// }
/// ```
pub static UNBOUNDED_VECTOR_GROWTH: LintDescriptor = LintDescriptor {
    name: "unbounded_vector_growth",
    category: LintCategory::Security,
    description: "Vector growth without size limit may cause DoS",
    group: RuleGroup::Preview, // Preview due to potential FPs
    fix: FixDescriptor::none(),
};

pub struct UnboundedVectorGrowthLint;

impl LintRule for UnboundedVectorGrowthLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &UNBOUNDED_VECTOR_GROWTH
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        check_unbounded_vector_growth(root, source, ctx);
    }
}

fn check_unbounded_vector_growth(node: Node, source: &str, ctx: &mut LintContext<'_>) {
    // Look for function definitions
    if node.kind() == "function_definition" {
        let func_text = node.utf8_text(source.as_bytes()).unwrap_or("");
        let func_lower = func_text.to_lowercase();
        
        // Check if function has vector push operations
        let has_push = func_lower.contains("vector::push_back") || 
                       func_lower.contains(".push_back(") ||
                       func_lower.contains("vec_set::insert") ||
                       func_lower.contains("vec_map::insert");
        
        if has_push {
            // Check if there's a length check before the push
            let has_length_check = func_lower.contains("vector::length") ||
                                   func_lower.contains(".length()") ||
                                   func_lower.contains("< max") ||
                                   func_lower.contains("< MAX") ||
                                   func_lower.contains("<= max") ||
                                   func_lower.contains("<= MAX") ||
                                   func_lower.contains("e_too_many") ||
                                   func_lower.contains("e_max_") ||
                                   func_lower.contains("e_limit");
            
            if !has_length_check {
                let func_name = node.child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                    .unwrap_or("unknown");
                
                ctx.report_node(
                    &UNBOUNDED_VECTOR_GROWTH,
                    node,
                    format!(
                        "Function `{}` adds to a vector without an apparent size limit. \
                         Consider adding a maximum size check like \
                         `assert!(vector::length(&v) < MAX_SIZE, E_TOO_MANY)` to prevent DoS.",
                        func_name
                    ),
                );
            }
        }
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        check_unbounded_vector_growth(child, source, ctx);
    }
}

// ============================================================================
// hardcoded_address - Detects literal addresses in code
// ============================================================================

/// Detects hardcoded addresses that should be constants or parameters.
///
/// # Security References
///
/// - **General Best Practice**: Hardcoded addresses make code inflexible
/// - **Upgrade Safety**: Addresses may need to change across deployments
///
/// # Why This Matters
///
/// Hardcoded addresses:
/// 1. **Inflexibility**: Can't change without code modification
/// 2. **Test/Prod confusion**: Wrong address in wrong environment
/// 3. **Audit difficulty**: Hard to verify all addresses are correct
///
/// # Example
///
/// ```move
/// // BAD - Hardcoded address
/// public fun send_fee(coin: Coin<SUI>) {
///     transfer::public_transfer(coin, @0x1234abcd...);
/// }
/// ```
///
/// # Correct Pattern
///
/// ```move
/// const FEE_RECIPIENT: address = @fee_recipient;
/// // Or use a config object
/// 
/// public fun send_fee(config: &Config, coin: Coin<SUI>) {
///     transfer::public_transfer(coin, config.fee_recipient);
/// }
/// ```
pub static HARDCODED_ADDRESS: LintDescriptor = LintDescriptor {
    name: "hardcoded_address",
    category: LintCategory::Security,
    description: "Hardcoded address should be a constant or parameter",
    group: RuleGroup::Preview, // Preview due to many legitimate uses
    fix: FixDescriptor::none(),
};

pub struct HardcodedAddressLint;

impl LintRule for HardcodedAddressLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &HARDCODED_ADDRESS
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        check_hardcoded_address(root, source, ctx);
    }
}

/// Addresses that are commonly legitimate and shouldn't be flagged
const KNOWN_SAFE_ADDRESSES: &[&str] = &[
    "@0x0",      // Zero address (null)
    "@0x1",      // System address
    "@0x2",      // Sui framework
    "@0x3",      // Sui system
    "@0x5",      // Sui system
    "@0x6",      // Clock object
    "@0x8",      // Random object
    "@0xdee9",   // DeepBook
];

fn check_hardcoded_address(node: Node, source: &str, ctx: &mut LintContext<'_>) {
    // Look for address literals in function bodies (not at module/const level)
    if node.kind() == "function_definition" {
        let func_text = node.utf8_text(source.as_bytes()).unwrap_or("");
        
        // Find @0x... patterns that aren't in known safe list
        let mut i = 0;
        let bytes = func_text.as_bytes();
        while i < bytes.len() {
            if bytes[i] == b'@' && i + 3 < bytes.len() && bytes[i+1] == b'0' && bytes[i+2] == b'x' {
                // Found @0x, extract the address
                let start = i;
                let mut end = i + 3;
                while end < bytes.len() && (bytes[end].is_ascii_hexdigit() || bytes[end] == b'_') {
                    end += 1;
                }
                
                let addr = &func_text[start..end];
                let addr_lower = addr.to_lowercase();
                
                // Skip known safe addresses - must be exact match or followed by non-hex
                let is_safe = KNOWN_SAFE_ADDRESSES.iter().any(|safe| {
                    let safe_lower = safe.to_lowercase();
                    if addr_lower == safe_lower {
                        return true;
                    }
                    // Check if the address starts with a safe prefix followed by non-hex
                    // e.g., @0x2 should match @0x2 but not @0x234
                    if addr_lower.starts_with(&safe_lower) {
                        // Check what comes after
                        let remainder = &addr_lower[safe_lower.len()..];
                        // If there's more hex digits, it's not a match
                        !remainder.chars().next().map_or(true, |c| c.is_ascii_hexdigit())
                    } else {
                        false
                    }
                });
                
                // Skip short addresses (likely constants)
                let is_short = addr.len() < 10;
                
                if !is_safe && !is_short {
                    let func_name = node.child_by_field_name("name")
                        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                        .unwrap_or("unknown");
                    
                    ctx.report_node(
                        &HARDCODED_ADDRESS,
                        node,
                        format!(
                            "Function `{}` contains hardcoded address `{}`. \
                             Consider using a constant or configuration parameter for flexibility.",
                            func_name,
                            addr
                        ),
                    );
                    // Only report once per function
                    break;
                }
                
                i = end;
            } else {
                i += 1;
            }
        }
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        check_hardcoded_address(child, source, ctx);
    }
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

        let lint3 = SharedCapabilityLint;
        lint3.check(tree.root_node(), source, &mut ctx);

        let lint4 = SuspiciousOverflowCheckLint;
        lint4.check(tree.root_node(), source, &mut ctx);

        let lint5 = StaleOraclePriceLint;
        lint5.check(tree.root_node(), source, &mut ctx);

        let lint6 = SingleStepOwnershipTransferLint;
        lint6.check(tree.root_node(), source, &mut ctx);

        let lint7 = UncheckedCoinSplitLint;
        lint7.check(tree.root_node(), source, &mut ctx);

        let lint8 = MissingWitnessDropLint;
        lint8.check(tree.root_node(), source, &mut ctx);

        let lint9 = PublicRandomAccessLint;
        lint9.check(tree.root_node(), source, &mut ctx);

        let lint10 = UnboundedVectorGrowthLint;
        lint10.check(tree.root_node(), source, &mut ctx);

        let lint11 = HardcodedAddressLint;
        lint11.check(tree.root_node(), source, &mut ctx);

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

    // =========================================================================
    // SharedCapabilityLint tests
    // =========================================================================

    #[test]
    fn test_shared_admin_cap_detected() {
        let source = r#"
            module example::admin {
                use sui::transfer;
                
                public fun init(ctx: &mut TxContext) {
                    let admin_cap = AdminCap { id: object::new(ctx) };
                    transfer::share_object(admin_cap);
                }
            }
        "#;

        let messages = lint_source(source);
        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Capability"));
        assert!(messages[0].contains("share_object"));
    }

    #[test]
    fn test_shared_mint_cap_detected() {
        let source = r#"
            module example::token {
                use sui::transfer;
                
                public fun init(ctx: &mut TxContext) {
                    let mint_cap = MintCap { id: object::new(ctx) };
                    transfer::public_share_object(mint_cap);
                }
            }
        "#;

        let messages = lint_source(source);
        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Capability"));
    }

    #[test]
    fn test_shared_treasury_cap_detected() {
        let source = r#"
            module example::coin {
                use sui::transfer;
                
                public fun init(ctx: &mut TxContext) {
                    transfer::share_object(TreasuryCap { id: object::new(ctx) });
                }
            }
        "#;

        let messages = lint_source(source);
        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn test_transferred_cap_ok() {
        // Transferring to owner is the correct pattern
        let source = r#"
            module example::admin {
                use sui::transfer;
                
                public fun init(ctx: &mut TxContext) {
                    let admin_cap = AdminCap { id: object::new(ctx) };
                    transfer::transfer(admin_cap, tx_context::sender(ctx));
                }
            }
        "#;

        let messages = lint_source(source);
        assert!(messages.is_empty());
    }

    #[test]
    fn test_shared_pool_not_cap_ok() {
        // Pool is not a capability - should not fire
        let source = r#"
            module example::dex {
                use sui::transfer;
                
                public fun create_pool(ctx: &mut TxContext) {
                    let pool = Pool { id: object::new(ctx) };
                    transfer::share_object(pool);
                }
            }
        "#;

        let messages = lint_source(source);
        assert!(messages.is_empty());
    }

    #[test]
    fn test_capacity_not_capability_ok() {
        // "Capacity" should NOT trigger - it's not a capability
        let source = r#"
            module example::storage {
                use sui::transfer;
                
                public fun init(ctx: &mut TxContext) {
                    let capacity = Capacity { max: 1000 };
                    transfer::share_object(capacity);
                }
            }
        "#;

        let messages = lint_source(source);
        assert!(messages.is_empty());
    }

    #[test]
    fn test_custom_cap_suffix_detected() {
        // Custom capability with Cap suffix should be detected
        let source = r#"
            module example::gov {
                use sui::transfer;
                
                public fun init(ctx: &mut TxContext) {
                    let governance_cap = GovernanceCap { id: object::new(ctx) };
                    transfer::share_object(governance_cap);
                }
            }
        "#;

        let messages = lint_source(source);
        assert_eq!(messages.len(), 1);
    }

    // =========================================================================
    // SuspiciousOverflowCheckLint tests
    // =========================================================================

    #[test]
    fn test_suspicious_overflow_check_detected() {
        let source = r#"
            module example::math {
                public fun checked_shlw(n: u256): (u256, bool) {
                    let mask = 0xffffffffffffffff << 192;
                    if (n > mask) {
                        return (0, true)
                    };
                    (n << 64, false)
                }
            }
        "#;

        let messages = lint_source(source);
        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("checked_shlw"));
        assert!(messages[0].contains("overflow"));
    }

    #[test]
    fn test_safe_function_with_shifts_detected() {
        let source = r#"
            module example::math {
                public fun safe_mul(a: u256, b: u256): (u256, bool) {
                    let max = 0xffffffffffffffffffffffffffffffff;
                    if (a > max / b) {
                        return (0, true)
                    };
                    (a << 1, false)
                }
            }
        "#;

        let messages = lint_source(source);
        // Should detect due to "safe_" prefix + bit shift + hex + comparison
        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn test_normal_arithmetic_function_ok() {
        // Regular function without overflow check patterns
        let source = r#"
            module example::math {
                public fun add(a: u64, b: u64): u64 {
                    a + b
                }
            }
        "#;

        let messages = lint_source(source);
        assert!(messages.is_empty());
    }

    #[test]
    fn test_checked_function_without_shifts_ok() {
        // "checked_" function but no bit shifts or hex - different kind of check
        let source = r#"
            module example::validate {
                public fun checked_balance(balance: u64, amount: u64): bool {
                    balance >= amount
                }
            }
        "#;

        let messages = lint_source(source);
        assert!(messages.is_empty());
    }

    // =========================================================================
    // StaleOraclePriceLint tests
    // =========================================================================

    #[test]
    fn test_stale_oracle_price_detected() {
        let source = r#"
            module example::oracle {
                use pyth::pyth;
                
                public fun get_value(price_info: &PriceInfoObject) {
                    let price = pyth::get_price_unsafe(price_info);
                    price.price
                }
            }
        "#;

        let messages = lint_source(source);
        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("get_price_unsafe"));
        assert!(messages[0].contains("stale"));
    }

    #[test]
    fn test_safe_oracle_price_ok() {
        let source = r#"
            module example::oracle {
                use pyth::pyth;
                
                public fun get_value(price_info: &PriceInfoObject, clock: &Clock) {
                    let price = pyth::get_price_no_older_than(price_info, clock, 60);
                    price.price
                }
            }
        "#;

        let messages = lint_source(source);
        assert!(messages.is_empty());
    }

    #[test]
    fn test_generic_price_function_ok() {
        // Regular price function without "unsafe" - should not fire
        let source = r#"
            module example::oracle {
                public fun get_price(asset: &Asset) {
                    asset.price
                }
            }
        "#;

        let messages = lint_source(source);
        assert!(messages.is_empty());
    }

    // =========================================================================
    // SingleStepOwnershipTransferLint tests
    // =========================================================================

    #[test]
    fn test_single_step_transfer_admin_detected() {
        let source = r#"
            module example::admin {
                public fun transfer_admin(exchange: &mut Exchange, new_admin: address) {
                    exchange.admin = new_admin;
                }
            }
        "#;

        let messages = lint_source(source);
        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("transfer_admin"));
        assert!(messages[0].contains("single-step"));
    }

    #[test]
    fn test_single_step_set_owner_detected() {
        let source = r#"
            module example::owner {
                public fun set_owner(config: &mut Config, new_owner: address) {
                    config.owner = new_owner;
                }
            }
        "#;

        let messages = lint_source(source);
        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("set_owner"));
    }

    #[test]
    fn test_two_step_propose_admin_ok() {
        // Two-step pattern with "pending" - should not fire
        let source = r#"
            module example::admin {
                public fun propose_admin(exchange: &mut Exchange, new_admin: address) {
                    exchange.pending_admin = option::some(new_admin);
                }
            }
        "#;

        let messages = lint_source(source);
        assert!(messages.is_empty());
    }

    #[test]
    fn test_two_step_accept_admin_ok() {
        // Accept function - should not fire
        let source = r#"
            module example::admin {
                public fun accept_admin(exchange: &mut Exchange, ctx: &TxContext) {
                    let pending = exchange.pending_admin;
                    exchange.admin = sender(ctx);
                }
            }
        "#;

        let messages = lint_source(source);
        assert!(messages.is_empty());
    }

    #[test]
    fn test_transfer_tokens_not_admin_ok() {
        // "transfer" but not admin transfer - should not fire
        let source = r#"
            module example::token {
                public fun transfer_tokens(from: &mut Account, to: address, amount: u64) {
                    from.balance = from.balance - amount;
                }
            }
        "#;

        let messages = lint_source(source);
        assert!(messages.is_empty());
    }

    #[test]
    fn test_set_admin_fee_not_ownership_ok() {
        // "set_admin" in name but actually setting a fee, not ownership
        let source = r#"
            module example::config {
                public fun set_admin_fee(config: &mut Config, fee: u64) {
                    config.admin_fee = fee;
                }
            }
        "#;

        let messages = lint_source(source);
        // Should NOT fire because there's no ".admin =" or ".owner =" assignment
        assert!(messages.is_empty());
    }

    // =========================================================================
    // UncheckedCoinSplitLint tests
    // =========================================================================

    #[test]
    fn test_unchecked_coin_split_detected() {
        let source = r#"
            module example::withdraw {
                public fun withdraw(coin: &mut Coin<SUI>, amount: u64, ctx: &mut TxContext): Coin<SUI> {
                    coin::split(coin, amount, ctx)
                }
            }
        "#;

        let messages = lint_source(source);
        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("coin::split"));
        assert!(messages[0].contains("balance check"));
    }

    #[test]
    fn test_coin_split_with_balance_check_ok() {
        let source = r#"
            module example::withdraw {
                public fun withdraw(coin: &mut Coin<SUI>, amount: u64, ctx: &mut TxContext): Coin<SUI> {
                    assert!(coin::value(coin) >= amount, E_INSUFFICIENT);
                    coin::split(coin, amount, ctx)
                }
            }
        "#;

        let messages = lint_source(source);
        assert!(messages.is_empty());
    }

    // =========================================================================
    // MissingWitnessDropLint tests
    // =========================================================================

    #[test]
    fn test_missing_witness_drop_detected() {
        let source = r#"
            module example::token {
                struct MY_TOKEN {}
            }
        "#;

        let messages = lint_source(source);
        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("MY_TOKEN"));
        assert!(messages[0].contains("one-time witness"));
    }

    #[test]
    fn test_witness_with_drop_ok() {
        let source = r#"
            module example::token {
                struct MY_TOKEN has drop {}
            }
        "#;

        let messages = lint_source(source);
        assert!(messages.is_empty());
    }

    #[test]
    fn test_regular_struct_not_witness_ok() {
        // Not all caps, so not detected as OTW
        let source = r#"
            module example::data {
                struct UserData {}
            }
        "#;

        let messages = lint_source(source);
        assert!(messages.is_empty());
    }

    // =========================================================================
    // PublicRandomAccessLint tests
    // =========================================================================

    #[test]
    fn test_public_random_access_detected() {
        let source = r#"
            module example::game {
                public fun get_random_number(r: &Random) {
                    random::new_generator(r).generate_u64()
                }
            }
        "#;

        let messages = lint_source(source);
        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Random"));
        assert!(messages[0].contains("front-running"));
    }

    #[test]
    fn test_entry_random_ok() {
        // entry functions are OK for Random
        let source = r#"
            module example::game {
                entry fun roll_dice(r: &Random, ctx: &mut TxContext) {
                    let result = random::new_generator(r, ctx).generate_u64();
                }
            }
        "#;

        let messages = lint_source(source);
        assert!(messages.is_empty());
    }

    // =========================================================================
    // UnboundedVectorGrowthLint tests
    // =========================================================================

    #[test]
    fn test_unbounded_vector_growth_detected() {
        let source = r#"
            module example::group {
                public fun add_member(group: &mut Group, member: address) {
                    vector::push_back(&mut group.members, member);
                }
            }
        "#;

        let messages = lint_source(source);
        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("vector"));
        assert!(messages[0].contains("size limit"));
    }

    #[test]
    fn test_bounded_vector_ok() {
        let source = r#"
            module example::group {
                const MAX_MEMBERS: u64 = 100;
                
                public fun add_member(group: &mut Group, member: address) {
                    assert!(vector::length(&group.members) < MAX_SIZE, E_TOO_MANY);
                    vector::push_back(&mut group.members, member);
                }
            }
        "#;

        let messages = lint_source(source);
        assert!(messages.is_empty());
    }

    // =========================================================================
    // HardcodedAddressLint tests
    // =========================================================================

    #[test]
    fn test_hardcoded_address_detected() {
        let source = r#"
module example::fee {
    public fun send_fee(coin: Coin<SUI>) {
        transfer::public_transfer(coin, @0x1234567890abcdef);
    }
}
        "#;

        let messages = lint_source(source);
        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("hardcoded address"));
    }

    #[test]
    fn test_system_address_ok() {
        // System addresses like @0x2 (sui framework) are OK
        let source = r#"
            module example::call {
                public fun call_sui() {
                    let _ = @0x2;
                }
            }
        "#;

        let messages = lint_source(source);
        assert!(messages.is_empty());
    }

    #[test]
    fn test_short_address_ok() {
        // Short addresses are likely named constants
        let source = r#"
            module example::call {
                public fun call_thing() {
                    let _ = @0xabc;
                }
            }
        "#;

        let messages = lint_source(source);
        assert!(messages.is_empty());
    }
}
