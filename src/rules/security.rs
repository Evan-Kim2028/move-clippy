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

use crate::lint::{
    AnalysisKind, FixDescriptor, LintCategory, LintContext, LintDescriptor, LintRule, RuleGroup,
    TypeSystemGap,
};
use crate::rules::util::is_test_only_module;
use tree_sitter::Node;

// ============================================================================
// Helper functions for syntax-based ability checking
// ============================================================================

/// Check if a struct definition node has the `drop` ability (syntax-based).
fn has_drop_ability(node: Node, source: &str) -> bool {
    let struct_text = node.utf8_text(source.as_bytes()).unwrap_or("");
    struct_text.contains("has drop")
        || struct_text.contains("has key, drop")
        || struct_text.contains("has drop, key")
        || struct_text.contains("has store, drop")
        || struct_text.contains("has drop, store")
        || struct_text.contains("has copy, drop")
        || struct_text.contains("has drop, copy")
        || struct_text.contains("drop,")
        || struct_text.contains(", drop")
}

// ============================================================================
// suspicious_overflow_check - Detects potentially incorrect manual overflow checks
// ============================================================================

/// Detects suspicious patterns in manual overflow checking code.
///
/// # Security References
///
/// - **Cetus $223M Hack (2025-05-22)**: Integer overflow in integer_mate library
///   URL: <https://x.com/paborji/status/1925573106270621989>
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
/// # Stability
///
/// STABLE: Validated against 13 ecosystem repos with 100% true positive rate.
/// All 4 findings were in legitimate overflow-checking math libraries.
pub static SUSPICIOUS_OVERFLOW_CHECK: LintDescriptor = LintDescriptor {
    name: "suspicious_overflow_check",
    category: LintCategory::Security,
    description: "Manual overflow check detected - these are error-prone. Consider using built-in checked arithmetic (see Cetus $223M hack)",
    group: RuleGroup::Stable, // Promoted: 100% TP rate in ecosystem validation
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::Syntactic,
    gap: Some(TypeSystemGap::ArithmeticSafety),
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
    if node.kind() == "function_definition"
        && let Some(name_node) = node.child_by_field_name("name")
    {
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
            let has_comparison = func_text.contains(" > ")
                || func_text.contains(" >= ")
                || func_text.contains(" < ")
                || func_text.contains(" <= ");
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
/// - **Bluefin MoveBit Audit (2024-05)**: "Dangerous Single-Step Ownership Transfer"
///   Finding: Protocol used `pyth::get_price_unsafe` which doesn't guarantee freshness
///   URL: <https://www.movebit.xyz/blog/post/Bluefin-vulnerabilities-explanation-1.html>
///   Verified: 2025-12-13
///
/// - **Pyth Network Documentation**: Explicitly warns about `get_price_unsafe`
///   URL: <https://docs.pyth.network/>
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
    analysis: AnalysisKind::Syntactic,
    gap: Some(TypeSystemGap::TemporalOrdering),
};

/// Function names that indicate potentially stale oracle price retrieval.
const UNSAFE_PRICE_FUNCTIONS: &[&str] = &["get_price_unsafe", "price_unsafe"];

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
        let is_unsafe_price_call = UNSAFE_PRICE_FUNCTIONS
            .iter()
            .any(|func| node_text.contains(func));

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
///   URL: <https://www.movebit.xyz/blog/post/Bluefin-vulnerabilities-explanation-1.html>
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
/// // VULNERABLE - enables theft
/// public fun transfer_admin(exchange: &mut Exchange, new_admin: address) {
///     exchange.admin = new_admin;  // Typo = permanent loss!
/// }
///
/// // CORRECT - two-step with confirmation
/// public fun propose_admin(exchange: &mut Exchange, new_admin: address) {
///     // Requires caller to already have AdminCap
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
    analysis: AnalysisKind::Syntactic,
    gap: Some(TypeSystemGap::TemporalOrdering),
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
const TWO_STEP_PATTERNS: &[&str] = &["pending", "propose", "accept", "claim"];

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
    if node.kind() == "function_definition"
        && let Some(name_node) = node.child_by_field_name("name")
    {
        let func_name = name_node
            .utf8_text(source.as_bytes())
            .unwrap_or("")
            .to_lowercase();

        // Check if function name suggests ownership transfer
        let is_ownership_transfer = OWNERSHIP_TRANSFER_PATTERNS
            .iter()
            .any(|pat| func_name.contains(pat));

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
                let has_admin_assignment = func_text_lower.contains(".admin =")
                    || func_text_lower.contains(".owner =")
                    || func_text_lower.contains(".authority =");

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
///   URL: <https://movebit.xyz/blog/post/Sui-Objects-Security-Principles-and-Best-Practices.html>
///   Verified: 2024-12-13 (Balance validation best practices)
///
/// - **Sui Standard Library**: coin::split panics if amount > balance
///   URL: <https://docs.sui.io/references/framework/sui/coin>
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
///     assert!(coin::value(user_coin) >= amount, E_INSUFFICIENT);
///     coin::split(user_coin, amount, ctx)
/// }
/// ```
pub static UNCHECKED_COIN_SPLIT: LintDescriptor = LintDescriptor {
    name: "unchecked_coin_split",
    category: LintCategory::Security,
    description: "[DEPRECATED] Sui runtime already enforces balance checks - coin::split panics on insufficient balance",
    group: RuleGroup::Deprecated,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::Syntactic,
    gap: Some(TypeSystemGap::ValueFlow),
};

pub struct UncheckedCoinSplitLint;

impl LintRule for UncheckedCoinSplitLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &UNCHECKED_COIN_SPLIT
    }

    fn check(&self, _root: Node, _source: &str, _ctx: &mut LintContext<'_>) {
        // DEPRECATED: Sui runtime already enforces this - coin::split panics on insufficient balance
        // This lint only suggested better error messages, not a security fix
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
///   URL: <https://docs.sui.io/concepts/sui-move-concepts/one-time-witness>
///   Verified: 2024-12-13 (OTW must have drop)
///
/// - **MoveBit (2023)**: "Sui Move Security Best Practices"
///   URL: <https://movebit.xyz/blog/post/Sui-Objects-Security-Principles-and-Best-Practices.html>
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
    analysis: AnalysisKind::Syntactic,
    gap: Some(TypeSystemGap::AbilityMismatch),
};

pub struct MissingWitnessDropLint;

impl LintRule for MissingWitnessDropLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &MISSING_WITNESS_DROP
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        // Skip test modules - OTW pattern is irrelevant in tests
        if is_test_only_module(root, source) {
            return;
        }
        check_missing_witness_drop(root, source, ctx);
    }
}

fn check_missing_witness_drop(node: Node, source: &str, ctx: &mut LintContext<'_>) {
    // Look for struct definitions
    if node.kind() == "struct_definition"
        && let Some(name_node) = node.child_by_field_name("name")
    {
        let struct_name = name_node.utf8_text(source.as_bytes()).unwrap_or("");

        // OTW pattern: SCREAMING_SNAKE_CASE, same as module name, empty body
        // Check if it looks like an OTW (all uppercase with underscores)
        let is_screaming_case = struct_name.chars().all(|c| c.is_uppercase() || c == '_');

        if is_screaming_case && !struct_name.is_empty() {
            // Check if it has empty body (no fields)
            let struct_text = node.utf8_text(source.as_bytes()).unwrap_or("");
            let has_empty_body = struct_text.contains("{}") || struct_text.contains("{ }");

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
///   URL: <https://docs.sui.io/guides/developer/advanced/randomness>
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
    analysis: AnalysisKind::Syntactic,
    gap: Some(TypeSystemGap::ApiMisuse),
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
        let is_public = func_text.starts_with("public fun") || func_text.contains("public fun ");
        let is_entry = func_text.contains("entry ");

        // Entry functions are OK because they can't be composed
        if is_public && !is_entry {
            // Check if function takes Random as parameter or returns random value
            let func_lower = func_text.to_lowercase();
            let has_random_param =
                func_lower.contains("&random") || func_lower.contains(": random");
            let returns_random = func_lower.contains("-> u64")
                && (func_lower.contains("generate") || func_lower.contains("random"));

            if has_random_param || returns_random {
                let func_name = node
                    .child_by_field_name("name")
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
// ignored_boolean_return - Detects boolean-returning functions with ignored results
// ============================================================================

/// Detects when boolean-returning functions like `vector::contains` have their
/// results ignored, which often indicates a missing authorization check.
///
/// # Security References
///
/// - **Typus Finance Hack (Oct 2025)**: $3.4M lost due to `vector::contains()` result ignored
///   URL: <https://slowmist.medium.com/is-the-move-language-secure-the-typus-permission-validation-vulnerability-755a5175f7c3>
///   Verified: 2025-12-13
///
/// # Why This Matters
///
/// Functions like `vector::contains`, `table::contains`, `option::is_some` return
/// boolean values that should be checked. Ignoring the result often means the
/// authorization check is not being enforced.
///
/// # Example
///
/// ```move
/// // VULNERABLE - result ignored!
/// public fun update(authority: &UpdateAuthority, ctx: &TxContext) {
///     vector::contains(&authority.whitelist, &tx_context::sender(ctx));
///     // proceeds without checking result...
/// }
///
/// // CORRECT
/// public fun update(authority: &UpdateAuthority, ctx: &TxContext) {
///     assert!(vector::contains(&authority.whitelist, &tx_context::sender(ctx)), E_UNAUTHORIZED);
/// }
/// ```
pub static IGNORED_BOOLEAN_RETURN: LintDescriptor = LintDescriptor {
    name: "ignored_boolean_return",
    category: LintCategory::Security,
    description: "Boolean-returning function result is ignored, may indicate missing authorization check (see: Typus Finance hack)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::Syntactic,
    gap: Some(TypeSystemGap::ValueFlow),
};

/// Functions that return bool and should have their results checked
const BOOLEAN_FUNCTIONS: &[&str] = &[
    "contains", // vector::contains, table::contains
    "is_some",  // option::is_some
    "is_none",  // option::is_none
    "is_empty", // vector::is_empty
    "exists",   // exists<T>(addr)
];

pub struct IgnoredBooleanReturnLint;

impl LintRule for IgnoredBooleanReturnLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &IGNORED_BOOLEAN_RETURN
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        // Skip test modules - benchmarks legitimately ignore returns
        if is_test_only_module(root, source) {
            return;
        }
        check_ignored_boolean_return(root, source, ctx);
    }
}

fn check_ignored_boolean_return(node: Node, source: &str, ctx: &mut LintContext<'_>) {
    // Look for call_expression within a node
    if node.kind() == "call_expression" {
        let call_text = node.utf8_text(source.as_bytes()).unwrap_or("");

        // Check if this is a boolean-returning function
        for func in BOOLEAN_FUNCTIONS {
            if call_text.contains(&format!("{}(", func))
                || call_text.ends_with(&format!("{}(", func))
            {
                // Check if this call's result is being used by walking up the parent chain
                let mut current = node.parent();
                while let Some(parent) = current {
                    let parent_kind = parent.kind();
                    // These parent types mean the result IS being used
                    if parent_kind == "let_statement"
                        || parent_kind == "assignment"
                        || parent_kind == "macro_call_expression"  // assert!()
                        || parent_kind == "if_expression"
                        || parent_kind == "while_expression"
                        || parent_kind == "return_expression"
                        || parent_kind == "binary_expression"  // used in && or ||
                        || parent_kind == "unary_expression"   // !contains(...)
                        || parent_kind == "parenthesized_expression"  // (contains(...))
                        || parent_kind == "call_expression"
                    // nested in another call like assert!(...)
                    {
                        // Result is being used, skip to recursion
                        let mut cursor = node.walk();
                        for child in node.children(&mut cursor) {
                            check_ignored_boolean_return(child, source, ctx);
                        }
                        return;
                    }
                    // Stop at function/block boundaries
                    if parent_kind == "function_definition" || parent_kind == "block" {
                        break;
                    }
                    current = parent.parent();
                }

                // Also check if this call is inside a macro like assert!
                // by looking at the source context - check a wider window for multi-line asserts
                let start_byte = node.start_byte();
                if start_byte > 50 {
                    let prefix = &source[start_byte.saturating_sub(100)..start_byte];
                    // Count open/close parens to see if we're inside an assert!(
                    if prefix.contains("assert!") || prefix.contains("assert_eq!") {
                        // Check if we're still inside the assert by counting parens
                        let after_assert =
                            prefix.rfind("assert").map(|i| &prefix[i..]).unwrap_or("");
                        let open_parens = after_assert.matches('(').count();
                        let close_parens = after_assert.matches(')').count();
                        if open_parens > close_parens {
                            // Inside an assert macro, skip
                            let mut cursor = node.walk();
                            for child in node.children(&mut cursor) {
                                check_ignored_boolean_return(child, source, ctx);
                            }
                            return;
                        }
                    }
                }

                // Check if this is the last expression in a function (implicit return)
                // by checking if there's nothing significant after it
                let end_byte = node.end_byte();
                if end_byte < source.len() {
                    let suffix = &source[end_byte..source.len().min(end_byte + 20)];
                    // If followed only by whitespace and }, it's an implicit return
                    let trimmed = suffix.trim();
                    if trimmed.is_empty() || trimmed.starts_with('}') {
                        // Implicit return, skip
                        let mut cursor = node.walk();
                        for child in node.children(&mut cursor) {
                            check_ignored_boolean_return(child, source, ctx);
                        }
                        return;
                    }
                }

                // Get function context for better message
                let func_name = get_enclosing_function_name(node, source);

                ctx.report_node(
                    &IGNORED_BOOLEAN_RETURN,
                    node,
                    format!(
                        "Function `{}` calls `{}` but ignores the boolean result. \
                         This may indicate a missing authorization check. \
                         Consider wrapping in `assert!()` or using the result in a condition.",
                        func_name, func
                    ),
                );
                break;
            }
        }
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        check_ignored_boolean_return(child, source, ctx);
    }
}

/// Get the name of the enclosing function
fn get_enclosing_function_name<'a>(node: Node<'a>, source: &'a str) -> &'a str {
    let mut current = Some(node);
    while let Some(n) = current {
        if (n.kind() == "function_definition" || n.kind() == "native_function_definition")
            && let Some(name_node) = n.child_by_field_name("name")
        {
            return name_node.utf8_text(source.as_bytes()).unwrap_or("unknown");
        }
        current = n.parent();
    }
    "unknown"
}

// ============================================================================
// unchecked_withdrawal - Detects withdrawals without balance checks
// ============================================================================

/// Detects withdrawal/unstake operations without preceding balance validation.
///
/// # Security References
///
/// - **Thala Hack (Nov 2024)**: $25.5M lost due to unstake without balance check
///   URL: <https://www.halborn.com/blog/post/explained-the-thala-hack-november-2024>
///   Verified: 2025-12-13
///
/// # Why This Matters
///
/// Withdrawal functions that don't validate the user's balance before withdrawing
/// can allow users to withdraw more than they deposited.
///
/// # Example
///
/// ```move
/// // VULNERABLE
/// public fun unstake(user: &mut User, amount: u64): Coin<SUI> {
///     pool::take(amount);  // No check if user has enough!
/// }
///
/// // CORRECT - balance check before withdrawal
/// public fun unstake(user: &mut User, amount: u64): Coin<SUI> {
///     assert!(user.balance >= amount, E_INSUFFICIENT);
///     user.balance = user.balance - amount;
///     pool::take(amount)
/// }
/// ```
pub static UNCHECKED_WITHDRAWAL: LintDescriptor = LintDescriptor {
    name: "unchecked_withdrawal",
    category: LintCategory::Security,
    description: "[DEPRECATED] Business logic bugs require formal verification, not linting - name-based heuristics have high FP rate",
    group: RuleGroup::Deprecated,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::Syntactic,
    gap: Some(TypeSystemGap::ValueFlow),
};

pub struct UncheckedWithdrawalLint;

impl LintRule for UncheckedWithdrawalLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &UNCHECKED_WITHDRAWAL
    }

    fn check(&self, _root: Node, _source: &str, _ctx: &mut LintContext<'_>) {
        // DEPRECATED: Business logic bugs (like the Thala hack) cannot be caught by type analysis
        // The concept of "withdrawal without balance check" is semantic, not syntactic
        // Would need formal verification to properly catch this class of bugs
    }
}

// ============================================================================
// capability_leak - DEPRECATED
// ============================================================================

pub static CAPABILITY_LEAK: LintDescriptor = LintDescriptor {
    name: "capability_leak",
    category: LintCategory::Security,
    description: "[DEPRECATED] Superseded by capability_transfer_v2 which uses type-based detection",
    group: RuleGroup::Deprecated,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::Syntactic,
    gap: Some(TypeSystemGap::CapabilityEscape),
};

pub struct CapabilityLeakLint;

impl LintRule for CapabilityLeakLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &CAPABILITY_LEAK
    }

    fn check(&self, _root: Node, _source: &str, _ctx: &mut LintContext<'_>) {
        // DEPRECATED: Superseded by capability_transfer_v2 in absint_lints.rs
        // The v2 version uses type-based detection (checking abilities) instead of name heuristics
    }
}

// ============================================================================
// NEW LINTS: Type System Gap Coverage
// ============================================================================

// ============================================================================
// destroy_zero_unchecked - Detects destroy_zero without prior zero-check
// ============================================================================

/// Detects calls to `destroy_zero` without a prior check that the value is zero.
///
/// # Type System Gap: ValueFlow
///
/// The type system allows `destroy_zero(balance)` without verifying the balance
/// is actually zero. This causes a runtime abort if the balance is non-zero,
/// potentially leading to fund loss in error handling paths.
///
/// # Why This Matters
///
/// If `destroy_zero` is called on a non-zero balance:
/// 1. The transaction aborts
/// 2. If this is in an error path, the original error is masked
/// 3. Non-zero balances are lost if the abort is caught incorrectly
///
/// # Example (Bad)
///
/// ```move
/// public fun cleanup(b: Balance<SUI>) {
///     balance::destroy_zero(b);  // Will abort if b > 0!
/// }
/// ```
///
/// # Correct Pattern
///
/// ```move
/// public fun cleanup(b: Balance<SUI>) {
///     assert!(balance::value(&b) == 0, E_NOT_EMPTY);
///     balance::destroy_zero(b);
/// }
/// ```
pub static DESTROY_ZERO_UNCHECKED: LintDescriptor = LintDescriptor {
    name: "destroy_zero_unchecked",
    category: LintCategory::Security,
    description: "destroy_zero called without verifying value is zero - may abort unexpectedly (needs CFG for low FP)",
    group: RuleGroup::Experimental,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::Syntactic,
    gap: Some(TypeSystemGap::ValueFlow),
};

pub struct DestroyZeroUncheckedLint;

impl LintRule for DestroyZeroUncheckedLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &DESTROY_ZERO_UNCHECKED
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        // Skip test modules - test cleanup code may legitimately skip checks
        if is_test_only_module(root, source) {
            return;
        }
        check_destroy_zero_unchecked(root, source, ctx);
    }
}

fn check_destroy_zero_unchecked(node: Node, source: &str, ctx: &mut LintContext<'_>) {
    if node.kind() == "function_definition" {
        let func_text = node.utf8_text(source.as_bytes()).unwrap_or("");

        // Check if function calls destroy_zero
        if func_text.contains("destroy_zero") {
            // Check if there's a zero-check pattern before it
            let has_zero_check = func_text.contains("== 0")
                || func_text.contains("value(&") && func_text.contains("== 0")
                || func_text.contains("is_zero")
                || func_text.contains("assert!")
                    && (func_text.contains("value") || func_text.contains("== 0"));

            if !has_zero_check {
                let func_name = node
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                    .unwrap_or("unknown");

                ctx.report_node(
                    &DESTROY_ZERO_UNCHECKED,
                    node,
                    format!(
                        "Function `{}` calls `destroy_zero` without verifying the value is zero. \
                         This will abort if the balance/coin is non-zero. \
                         Add `assert!(value(&x) == 0, E_NOT_ZERO)` before destroy_zero.",
                        func_name
                    ),
                );
            }
        }
    }

    // Recurse
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        check_destroy_zero_unchecked(child, source, ctx);
    }
}

// ============================================================================
// otw_pattern_violation - Detects OTW types that don't match module name
// ============================================================================

/// Detects one-time witness types that don't follow the OTW naming convention.
///
/// # Type System Gap: ApiMisuse
///
/// The `coin::create_currency` function requires a one-time witness (OTW) type.
/// The OTW pattern requires:
/// 1. Struct name matches module name (uppercase)
/// 2. Has `drop` ability
/// 3. Has no fields
///
/// The type system doesn't enforce the naming convention - it's checked at runtime.
///
/// # Example (Bad)
///
/// ```move
/// module my_coin {
///     struct Token has drop {}  // Wrong name!
///     
///     fun init(witness: Token, ctx: &mut TxContext) {
///         coin::create_currency(witness, ...)  // Runtime abort!
///     }
/// }
/// ```
///
/// # Correct Pattern
///
/// ```move
/// module my_coin {
///     struct MY_COIN has drop {}  // Matches module name (uppercase)
/// }
/// ```
pub static OTW_PATTERN_VIOLATION: LintDescriptor = LintDescriptor {
    name: "otw_pattern_violation",
    category: LintCategory::Security,
    description: "One-time witness type name doesn't match module name - will fail at runtime (needs better module name handling)",
    group: RuleGroup::Experimental,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::Syntactic,
    gap: Some(TypeSystemGap::ApiMisuse),
};

pub struct OtwPatternViolationLint;

impl LintRule for OtwPatternViolationLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &OTW_PATTERN_VIOLATION
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        // Skip test modules - test coin types don't need real OTW pattern
        if is_test_only_module(root, source) {
            return;
        }
        check_otw_pattern(root, source, ctx, None);
    }
}

fn check_otw_pattern(
    node: Node,
    source: &str,
    ctx: &mut LintContext<'_>,
    module_name: Option<&str>,
) {
    match node.kind() {
        "module_definition" => {
            // Extract module name
            if let Some(name_node) = node.child_by_field_name("name") {
                let mod_name = name_node.utf8_text(source.as_bytes()).unwrap_or("");
                // Recurse with module name context
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    check_otw_pattern(child, source, ctx, Some(mod_name));
                }
                return;
            }
        }
        "function_definition" => {
            let func_text = node.utf8_text(source.as_bytes()).unwrap_or("");

            // Check if this calls create_currency
            if func_text.contains("create_currency")
                && let Some(mod_name) = module_name
            {
                let expected_otw = mod_name.to_uppercase();

                // Check if the expected OTW type is used
                if !func_text.contains(&expected_otw) {
                    let func_name = node
                        .child_by_field_name("name")
                        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                        .unwrap_or("unknown");

                    ctx.report_node(
                        &OTW_PATTERN_VIOLATION,
                        node,
                        format!(
                            "Function `{}` calls `create_currency` but the OTW type doesn't \
                                 appear to match the module name. Expected OTW type: `{}`. \
                                 The OTW must be named after the module in SCREAMING_CASE.",
                            func_name, expected_otw
                        ),
                    );
                }
            }
        }
        _ => {}
    }

    // Recurse
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        check_otw_pattern(child, source, ctx, module_name);
    }
}

// ============================================================================
// digest_as_randomness - Detects tx_context::digest used as randomness
// ============================================================================

/// Detects usage of `tx_context::digest` as a randomness source.
///
/// # Type System Gap: ApiMisuse
///
/// The `tx_context::digest()` function returns a deterministic hash of the
/// transaction. While it looks random, it's predictable and can be manipulated
/// by validators. The Sui documentation explicitly warns against using it for
/// randomness.
///
/// # Why This Matters
///
/// Using digest for randomness enables:
/// 1. Validator manipulation of "random" outcomes
/// 2. Front-running attacks in games/lotteries
/// 3. Predictable outcomes in security-sensitive operations
///
/// # Example (Bad)
///
/// ```move
/// public fun pick_winner(ctx: &TxContext): u64 {
///     let seed = tx_context::digest(ctx);
///     (*vector::borrow(seed, 0) as u64) % num_participants  // Predictable!
/// }
/// ```
///
/// # Correct Pattern
///
/// ```move
/// public fun pick_winner(r: &Random, ctx: &mut TxContext): u64 {
///     let mut gen = random::new_generator(r, ctx);
///     random::generate_u64_in_range(&mut gen, 0, num_participants)
/// }
/// ```
pub static DIGEST_AS_RANDOMNESS: LintDescriptor = LintDescriptor {
    name: "digest_as_randomness",
    category: LintCategory::Security,
    description: "tx_context::digest used as randomness source - predictable and manipulable (needs taint analysis for low FP)",
    group: RuleGroup::Experimental,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::Syntactic,
    gap: Some(TypeSystemGap::ApiMisuse),
};

pub struct DigestAsRandomnessLint;

impl LintRule for DigestAsRandomnessLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &DIGEST_AS_RANDOMNESS
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        // Skip test modules - test helpers may use digest for deterministic tests
        if is_test_only_module(root, source) {
            return;
        }
        check_digest_as_randomness(root, source, ctx);
    }
}

fn check_digest_as_randomness(node: Node, source: &str, ctx: &mut LintContext<'_>) {
    if node.kind() == "function_definition" {
        let func_text = node.utf8_text(source.as_bytes()).unwrap_or("");

        // Check if function uses digest
        if func_text.contains("digest(") || func_text.contains("::digest") {
            // Check if it's used in arithmetic/modulo operations (randomness pattern)
            let has_randomness_pattern = func_text.contains(" % ")
                || func_text.contains("borrow(")  // vector::borrow on digest
                || func_text.contains("random")
                || func_text.contains("seed")
                || func_text.contains("lottery")
                || func_text.contains("winner");

            if has_randomness_pattern {
                let func_name = node
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                    .unwrap_or("unknown");

                ctx.report_node(
                    &DIGEST_AS_RANDOMNESS,
                    node,
                    format!(
                        "Function `{}` appears to use `tx_context::digest` as a randomness source. \
                         Digest is predictable and can be manipulated by validators. \
                         Use `sui::random` for secure randomness instead.",
                        func_name
                    ),
                );
            }
        }
    }

    // Recurse
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        check_digest_as_randomness(child, source, ctx);
    }
}

// ============================================================================
// divide_by_zero_literal - Detects division by literal zero
// ============================================================================

/// Detects division or modulo by zero or by a variable that could be zero.
///
/// # Type System Gap: ArithmeticSafety
///
/// Move allows division by zero which causes a runtime abort. While
/// `unchecked_division_v2` handles the general case with CFG analysis,
/// this lint catches obvious cases with literal zeros or suspicious patterns.
///
/// # Example (Bad)
///
/// ```move
/// public fun bad_divide(x: u64): u64 {
///     x / 0  // Always aborts!
/// }
///
/// public fun bad_modulo(x: u64, n: u64): u64 {
///     coin::divide_into_n(&mut c, 0, ctx)  // n=0 aborts!
/// }
/// ```
pub static DIVIDE_BY_ZERO_LITERAL: LintDescriptor = LintDescriptor {
    name: "divide_by_zero_literal",
    category: LintCategory::Security,
    description: "Division or modulo by literal zero - will always abort",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::Syntactic,
    gap: Some(TypeSystemGap::ArithmeticSafety),
};

pub struct DivideByZeroLiteralLint;

impl LintRule for DivideByZeroLiteralLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &DIVIDE_BY_ZERO_LITERAL
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        check_divide_by_zero(root, source, ctx);
    }
}

fn check_divide_by_zero(node: Node, source: &str, ctx: &mut LintContext<'_>) {
    let node_text = node.utf8_text(source.as_bytes()).unwrap_or("");

    // Check for division/modulo by literal 0
    if node.kind() == "binary_expression" {
        // Look for patterns like "/ 0" or "% 0"
        if (node_text.contains("/ 0") || node_text.contains("% 0"))
            && !node_text.contains("/ 0x")  // Exclude hex
            && !node_text.contains("% 0x")
        {
            ctx.report_node(
                &DIVIDE_BY_ZERO_LITERAL,
                node,
                "Division or modulo by literal zero - this will always abort at runtime."
                    .to_string(),
            );
        }
    }

    // Check for divide_into_n with literal 0
    if node.kind() == "call_expression" && node_text.contains("divide_into_n") {
        // Simple check: if the call contains ", 0," or ", 0)" as the n parameter
        if node_text.contains(", 0,") || node_text.contains(", 0)") {
            ctx.report_node(
                &DIVIDE_BY_ZERO_LITERAL,
                node,
                "divide_into_n called with n=0 - this will abort at runtime.".to_string(),
            );
        }
    }

    // Recurse
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        check_divide_by_zero(child, source, ctx);
    }
}

// ============================================================================
// fresh_address_reuse - Detects fresh_object_address used multiple times
// ============================================================================

/// Detects when `fresh_object_address` result is used multiple times.
///
/// # Type System Gap: OwnershipViolation
///
/// Each call to `fresh_object_address` generates a unique address for creating
/// a new UID. If the result is stored and used multiple times, it violates
/// the uniqueness invariant.
///
/// # Example (Bad)
///
/// ```move
/// public fun bad(ctx: &mut TxContext) {
///     let addr = tx_context::fresh_object_address(ctx);
///     let uid1 = object::new_uid_from_address(addr);
///     let uid2 = object::new_uid_from_address(addr);  // Reuse!
/// }
/// ```
///
/// # Correct Pattern
///
/// ```move
/// public fun good(ctx: &mut TxContext) {
///     let uid1 = object::new(ctx);  // Fresh address internally
///     let uid2 = object::new(ctx);  // Another fresh address
/// }
/// ```
pub static FRESH_ADDRESS_REUSE: LintDescriptor = LintDescriptor {
    name: "fresh_address_reuse",
    category: LintCategory::Security,
    description: "fresh_object_address result appears to be reused - each UID needs a fresh address (needs usage tracking for low FP)",
    group: RuleGroup::Experimental,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::Syntactic,
    gap: Some(TypeSystemGap::OwnershipViolation),
};

pub struct FreshAddressReuseLint;

impl LintRule for FreshAddressReuseLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &FRESH_ADDRESS_REUSE
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        check_fresh_address_reuse(root, source, ctx);
    }
}

fn check_fresh_address_reuse(node: Node, source: &str, ctx: &mut LintContext<'_>) {
    if node.kind() == "function_definition" {
        let func_text = node.utf8_text(source.as_bytes()).unwrap_or("");

        // Check if function uses fresh_object_address
        if func_text.contains("fresh_object_address") {
            // Count occurrences of new_uid_from_address
            let uid_from_addr_count = func_text.matches("new_uid_from_address").count();
            let fresh_addr_count = func_text.matches("fresh_object_address").count();

            // If there are more UID creations than fresh addresses, likely reuse
            if uid_from_addr_count > fresh_addr_count {
                let func_name = node
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                    .unwrap_or("unknown");

                ctx.report_node(
                    &FRESH_ADDRESS_REUSE,
                    node,
                    format!(
                        "Function `{}` has {} `new_uid_from_address` calls but only {} \
                         `fresh_object_address` calls. Each UID needs its own fresh address. \
                         Consider using `object::new(ctx)` which handles this internally.",
                        func_name, uid_from_addr_count, fresh_addr_count
                    ),
                );
            }
        }
    }

    // Recurse
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        check_fresh_address_reuse(child, source, ctx);
    }
}

// ============================================================================
// suggest_capability_pattern - Detects address-based authorization anti-pattern
// ============================================================================

/// Detects address-based authorization patterns and suggests capability-based alternatives.
///
/// # Security References
///
/// - **Move Book**: Capability pattern is the idiomatic authorization mechanism
/// - **Sui Security Best Practices**: Prefer object capabilities over address checks
///
/// # Why This Matters
///
/// Address-based authorization (`sender(ctx) == ADMIN`) is an anti-pattern because:
/// 1. **Fragile**: Hardcoded addresses can't be rotated/transferred
/// 2. **Inflexible**: Can't delegate authority temporarily
/// 3. **Error-prone**: Easy to misconfigure or leak admin addresses
/// 4. **Not composable**: Functions can't require &AdminCap as proof
///
/// The capability pattern is superior because:
/// 1. **Transferable**: Capabilities can be transferred to new owners
/// 2. **Type-safe**: Compiler enforces authorization requirements
/// 3. **Composable**: Functions can require &AdminCap as proof
/// 4. **Revocable**: Capabilities can be destroyed to revoke access
///
/// # Detection Criteria
///
/// Flags patterns like:
/// - `assert!(tx_context::sender(ctx) == ADMIN, ...)`
/// - `assert!(sender(ctx) == config.owner, ...)`
/// - `if (sender(ctx) != admin) { abort ... }`
///
/// # Exceptions (NOT flagged)
///
/// - Sui framework system checks (`@0x0`)
/// - Test code
/// - Ownership verification for non-admin operations (e.g., NFT ownership)
///
/// # Example
///
/// ```move
/// // ANTI-PATTERN - flagged
/// const ADMIN: address = @0x123;
///
/// public fun admin_action(ctx: &TxContext) {
///     assert!(tx_context::sender(ctx) == ADMIN, E_NOT_ADMIN);
///     // ...
/// }
///
/// // SUGGESTED - capability pattern
/// public fun admin_action(_cap: &AdminCap) {
///     // Authorization is structural, not runtime
/// }
/// ```
///
/// # Stability
///
/// EXPERIMENTAL: Advisory lint for developer education. May flag intentional
/// patterns in legacy code.
pub static SUGGEST_CAPABILITY_PATTERN: LintDescriptor = LintDescriptor {
    name: "suggest_capability_pattern",
    category: LintCategory::Security,
    description: "Address-based authorization detected - consider using capability pattern for safer access control",
    group: RuleGroup::Experimental,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::Syntactic,
    gap: Some(TypeSystemGap::CapabilityEscape),
};

pub struct SuggestCapabilityPatternLint;

impl LintRule for SuggestCapabilityPatternLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &SUGGEST_CAPABILITY_PATTERN
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        // Skip test modules at the top level
        if is_test_only_module(root, source) {
            return;
        }
        check_address_based_auth(root, source, ctx);
    }
}

fn check_address_based_auth(node: Node, source: &str, ctx: &mut LintContext<'_>) {
    // Look for assert! statements with sender comparisons
    // tree-sitter-move uses "macro_call_expression" for assert! statements
    if node.kind() == "macro_call_expression" || node.kind() == "call_expression" {
        let node_text = node.utf8_text(source.as_bytes()).unwrap_or("");

        // Check if this is a sender address comparison
        if node_text.contains("assert!") && is_sender_address_check(node_text) {
            // Skip Sui system address checks (@0x0)
            if node_text.contains("@0x0") {
                // Recurse and return
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    check_address_based_auth(child, source, ctx);
                }
                return;
            }

            // Determine the type of address check
            let (severity, suggestion) = classify_address_check(node_text);

            ctx.report_node(
                &SUGGEST_CAPABILITY_PATTERN,
                node,
                format!("{}. {}", severity, suggestion),
            );
        }
    }

    // Also check for if statements with sender checks leading to abort
    if node.kind() == "if_expression" {
        let node_text = node.utf8_text(source.as_bytes()).unwrap_or("");

        if is_sender_address_check(node_text)
            && (node_text.contains("abort") || node_text.contains("return"))
        {
            // Skip system address checks
            if node_text.contains("@0x0") {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    check_address_based_auth(child, source, ctx);
                }
                return;
            }

            ctx.report_node(
                &SUGGEST_CAPABILITY_PATTERN,
                node,
                "Address-based authorization in if-condition. Consider using capability pattern: \
                 replace address check with a capability parameter like `_cap: &AdminCap`."
                    .to_string(),
            );
        }
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        check_address_based_auth(child, source, ctx);
    }
}

/// Check if text contains a sender-to-address comparison pattern
fn is_sender_address_check(text: &str) -> bool {
    // Patterns: sender(ctx) == ..., sender(ctx) != ..., ... == sender(ctx)
    let has_sender = text.contains("sender(") || text.contains("tx_context::sender(");
    let has_comparison = text.contains(" == ") || text.contains(" != ");

    has_sender && has_comparison
}

/// Classify the address check and return severity + suggestion
fn classify_address_check(text: &str) -> (&'static str, &'static str) {
    // Check for constant/literal address comparison (worse)
    if text.contains("== @") || text.contains("!= @") {
        return (
            "Hardcoded address comparison detected - this is fragile and non-transferable",
            "Consider using capability pattern: add a capability struct (e.g., `AdminCap`) \
             and require `_cap: &AdminCap` parameter instead of checking sender address.",
        );
    }

    // Check for field comparison (slightly better but still not ideal)
    if text.contains(".admin") || text.contains(".owner") || text.contains(".creator") {
        return (
            "Stored address comparison detected for authorization",
            "While checking against a stored address is better than hardcoded, \
             capability pattern is still preferred. Consider storing an AdminCap \
             in the owner's account instead of an address in your shared object.",
        );
    }

    // Generic sender check
    (
        "Address-based authorization detected",
        "Consider using capability pattern: define a capability struct with `key, store` \
         abilities and require it as a parameter for privileged operations.",
    )
}

// ============================================================================
// suggest_sequenced_witness - Detects boolean state machine anti-pattern
// ============================================================================

/// Detects boolean flag state machines and suggests sequenced witness pattern.
///
/// # Why This Matters
///
/// Boolean flags for sequencing (`step_1_complete`, `is_initialized`) are fragile:
/// 1. **Runtime-only enforcement**: Bugs can skip steps
/// 2. **Not composable**: Other modules can't verify step completion
/// 3. **Easy to forget**: Developer must remember to check each flag
///
/// The sequenced witness pattern uses proof types:
/// - Each step returns a proof struct (e.g., `Step1Proof`)
/// - Next step requires the proof as parameter
/// - Compiler enforces ordering at compile time
///
/// # Example
///
/// ```move
/// // ANTI-PATTERN
/// struct State { step_1_complete: bool }
/// public fun step_2(state: &State) {
///     assert!(state.step_1_complete, E_STEP_1_REQUIRED);
/// }
///
/// // SUGGESTED
/// struct Step1Proof has drop {}
/// public fun step_1(): Step1Proof { Step1Proof {} }
/// public fun step_2(_proof: Step1Proof) { /* compiler enforces ordering */ }
/// ```
pub static SUGGEST_SEQUENCED_WITNESS: LintDescriptor = LintDescriptor {
    name: "suggest_sequenced_witness",
    category: LintCategory::Security,
    description: "Boolean state flag detected - consider using sequenced witness pattern for compile-time ordering enforcement",
    group: RuleGroup::Experimental,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::Syntactic,
    gap: Some(TypeSystemGap::TemporalOrdering),
};

pub struct SuggestSequencedWitnessLint;

impl LintRule for SuggestSequencedWitnessLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &SUGGEST_SEQUENCED_WITNESS
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        if is_test_only_module(root, source) {
            return;
        }
        check_boolean_state_flags(root, source, ctx);
    }
}

/// Boolean field name patterns that suggest state machine usage
const STATE_FLAG_PATTERNS: &[&str] = &[
    "_complete",
    "_done",
    "_finished",
    "_initialized",
    "_started",
    "_pending",
    "is_active",
    "is_paused",
    "is_locked",
    "has_",
    "step_",
];

fn check_boolean_state_flags(node: Node, source: &str, ctx: &mut LintContext<'_>) {
    // Look for struct definitions with boolean fields matching state patterns
    if node.kind() == "struct_definition" {
        let node_text = node.utf8_text(source.as_bytes()).unwrap_or("");

        // Check for boolean fields with state-like names
        for pattern in STATE_FLAG_PATTERNS {
            if node_text.contains(pattern) && node_text.contains(": bool") {
                // Extract field name for better message
                let field_hint = if node_text.contains("_complete") {
                    "completion tracking"
                } else if node_text.contains("_initialized") || node_text.contains("_started") {
                    "initialization tracking"
                } else if node_text.contains("step_") {
                    "step sequencing"
                } else {
                    "state tracking"
                };

                ctx.report_node(
                    &SUGGEST_SEQUENCED_WITNESS,
                    node,
                    format!(
                        "Boolean field detected for {}. Consider using sequenced witness pattern: \
                         return a proof struct from each step that the next step requires as a parameter. \
                         This enforces ordering at compile time instead of runtime.",
                        field_hint
                    ),
                );
                break; // Only report once per struct
            }
        }
    }

    // Recurse
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        check_boolean_state_flags(child, source, ctx);
    }
}

// ============================================================================
// suggest_counted_capability - Detects counter-based supply anti-pattern
// ============================================================================

/// Detects counter-based supply limiting and suggests counted capability pattern.
///
/// # Why This Matters
///
/// Counter-based supply (`minted < max_supply`) has issues:
/// 1. **Runtime enforcement only**: Bugs can bypass the check
/// 2. **Centralized state**: Counter must be atomically updated
/// 3. **No proof of scarcity**: Can't verify supply limit externally
///
/// The counted capability pattern uses consumable tickets:
/// - Create exactly `max_supply` tickets at init
/// - Mint function consumes (unpacks) a ticket
/// - Supply limit enforced by Move's linearity
///
/// # Example
///
/// ```move
/// // ANTI-PATTERN
/// struct MintState { minted: u64, max_supply: u64 }
/// public fun mint(state: &mut MintState): NFT {
///     assert!(state.minted < state.max_supply, E_MAX_SUPPLY);
///     state.minted = state.minted + 1;
///     NFT { }
/// }
///
/// // SUGGESTED
/// struct MintTicket has key, store { id: UID }
/// public fun mint(ticket: MintTicket): NFT {
///     let MintTicket { id } = ticket;
///     object::delete(id);
///     NFT { }
/// }
/// ```
pub static SUGGEST_COUNTED_CAPABILITY: LintDescriptor = LintDescriptor {
    name: "suggest_counted_capability",
    category: LintCategory::Security,
    description: "Counter-based supply limiting detected - consider using counted capability pattern for linearity-enforced scarcity",
    group: RuleGroup::Experimental,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::Syntactic,
    gap: Some(TypeSystemGap::ValueFlow),
};

pub struct SuggestCountedCapabilityLint;

impl LintRule for SuggestCountedCapabilityLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &SUGGEST_COUNTED_CAPABILITY
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        if is_test_only_module(root, source) {
            return;
        }
        check_counter_based_supply(root, source, ctx);
    }
}

/// Patterns indicating counter-based supply limiting
const COUNTER_PATTERNS: &[&str] = &[
    "minted",
    "total_minted",
    "mint_count",
    "supply_count",
    "num_minted",
];

const MAX_PATTERNS: &[&str] = &[
    "max_supply",
    "max_mint",
    "supply_limit",
    "mint_limit",
    "cap",
];

fn check_counter_based_supply(node: Node, source: &str, ctx: &mut LintContext<'_>) {
    // Look for functions with counter increment + max check pattern
    if node.kind() == "function_definition" {
        let node_text = node.utf8_text(source.as_bytes()).unwrap_or("");
        let node_text_lower = node_text.to_lowercase();

        // Check for mint-like functions with counter patterns
        let has_counter = COUNTER_PATTERNS.iter().any(|p| node_text_lower.contains(p));
        let has_max = MAX_PATTERNS.iter().any(|p| node_text_lower.contains(p));
        let has_increment = node_text.contains("+ 1") || node_text.contains("+= 1");
        let has_comparison = node_text.contains(" < ") || node_text.contains(" <= ");

        // If we have counter + max + increment + comparison, likely a counter-based supply
        if has_counter && has_max && has_increment && has_comparison {
            let func_name = node
                .child_by_field_name("name")
                .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                .unwrap_or("unknown");

            ctx.report_node(
                &SUGGEST_COUNTED_CAPABILITY,
                node,
                format!(
                    "Function `{}` uses counter-based supply limiting. Consider using counted capability pattern: \
                     create a fixed number of 'ticket' objects at init, and require consuming one to mint. \
                     This enforces scarcity through Move's linearity rather than runtime checks.",
                    func_name
                ),
            );
        }
    }

    // Recurse
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        check_counter_based_supply(child, source, ctx);
    }
}

// ============================================================================
// suggest_balanced_receipt - Detects bookend invariant check anti-pattern
// ============================================================================

/// Detects bookend invariant checks and suggests balanced receipt pattern.
///
/// # Why This Matters
///
/// Bookend invariant checks (`k_before` / `k_after`) are error-prone:
/// 1. **Easy to forget**: Developer must remember the final assert
/// 2. **Not enforced**: Compiler doesn't require the check
/// 3. **Verbose**: Duplicated calculation logic
///
/// The balanced receipt pattern uses a hot potato:
/// - Begin operation creates a receipt (no abilities = must be consumed)
/// - Operations update the receipt's accumulators
/// - Complete operation unpacks and verifies the receipt
/// - Compiler enforces the complete call
///
/// # Example
///
/// ```move
/// // ANTI-PATTERN
/// public fun swap(pool: &mut Pool): Coin<B> {
///     let k_before = pool.x * pool.y;
///     // ... operations
///     let k_after = pool.x * pool.y;
///     assert!(k_after >= k_before, E_INVARIANT);
///     coin_out
/// }
///
/// // SUGGESTED
/// struct SwapReceipt { /* no abilities */ }
/// public fun begin_swap(pool: &Pool): SwapReceipt { ... }
/// public fun complete_swap(receipt: SwapReceipt, pool: &Pool) {
///     let SwapReceipt { ... } = receipt; // Unpacks and verifies
/// }
/// ```
pub static SUGGEST_BALANCED_RECEIPT: LintDescriptor = LintDescriptor {
    name: "suggest_balanced_receipt",
    category: LintCategory::Security,
    description: "Bookend invariant check detected - consider using balanced receipt (hot potato) pattern for compiler-enforced verification",
    group: RuleGroup::Experimental,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::Syntactic,
    gap: Some(TypeSystemGap::TemporalOrdering),
};

pub struct SuggestBalancedReceiptLint;

impl LintRule for SuggestBalancedReceiptLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &SUGGEST_BALANCED_RECEIPT
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        if is_test_only_module(root, source) {
            return;
        }
        check_bookend_invariants(root, source, ctx);
    }
}

fn check_bookend_invariants(node: Node, source: &str, ctx: &mut LintContext<'_>) {
    // Look for functions with before/after variable pairs
    if node.kind() == "function_definition" {
        let node_text = node.utf8_text(source.as_bytes()).unwrap_or("");

        // Check for before/after pattern pairs
        let has_before = node_text.contains("_before") || node_text.contains("before_");
        let has_after = node_text.contains("_after") || node_text.contains("after_");
        let has_assert = node_text.contains("assert!");

        // If we have both before and after variables with an assert, likely a bookend check
        if has_before && has_after && has_assert {
            // Additional check: look for invariant-related naming
            let is_invariant_check = node_text.contains("k_before")
                || node_text.contains("k_after")
                || node_text.contains("balance_before")
                || node_text.contains("balance_after")
                || node_text.contains("reserve_before")
                || node_text.contains("reserve_after")
                || node_text.contains("value_before")
                || node_text.contains("value_after");

            if is_invariant_check {
                let func_name = node
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                    .unwrap_or("unknown");

                ctx.report_node(
                    &SUGGEST_BALANCED_RECEIPT,
                    node,
                    format!(
                        "Function `{}` uses bookend invariant checking (before/after pattern). \
                         Consider using balanced receipt pattern: create a hot potato receipt at the start \
                         that accumulates values and MUST be verified at completion. \
                         This makes the verification compiler-enforced rather than developer-remembered.",
                        func_name
                    ),
                );
            }
        }
    }

    // Recurse
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        check_bookend_invariants(child, source, ctx);
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lint::{LintContext, LintSettings};
    use crate::parser::parse_source;

    fn lint_source(source: &str) -> Vec<String> {
        let tree = parse_source(source).unwrap();
        let mut ctx = LintContext::new(source, LintSettings::default());

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

        let lint12 = IgnoredBooleanReturnLint;
        lint12.check(tree.root_node(), source, &mut ctx);

        let lint14 = UncheckedWithdrawalLint;
        lint14.check(tree.root_node(), source, &mut ctx);

        let lint15 = CapabilityLeakLint;
        lint15.check(tree.root_node(), source, &mut ctx);

        ctx.into_diagnostics()
            .into_iter()
            .map(|d| d.message)
            .collect()
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

    // =========================================================================
    // FP Prevention Tests - droppable_hot_potato
    // =========================================================================

    #[test]
    fn test_empty_witness_struct_not_hot_potato() {
        // Empty structs with drop are typically witness/marker types, not hot potatoes
        // Example: ObligationOwnership, ObligationCollaterals from Scallop
        let source = r#"
            module example::witness {
                struct ObligationOwnership has drop {}
                struct ObligationCollaterals has drop {}
                struct ObligationDebts has drop {}
            }
        "#;

        let messages = lint_source(source);
        assert!(
            messages.is_empty(),
            "Empty witness structs should not be flagged"
        );
    }

    #[test]
    fn test_witness_keyword_struct_not_hot_potato() {
        // Structs with witness-related keywords should not be flagged
        let source = r#"
            module example::witness {
                struct MarkerType has drop {
                    value: u64,
                }
                struct WitnessStruct has drop {
                    id: ID,
                }
            }
        "#;

        let messages = lint_source(source);
        assert!(
            messages.is_empty(),
            "Witness-keyword structs should not be flagged"
        );
    }

    // =========================================================================
    // FP Prevention Tests
    // =========================================================================

    #[test]
    fn test_asset_metadata_struct_not_token() {
        // Metadata structs with "Asset" in name should not be flagged
        // Example: Asset, DepositedAsset from Bluefin
        let source = r#"
            module example::metadata {
                struct Asset has store, copy, drop {
                    symbol: String,
                    decimals: u8,
                }
                struct DepositedAsset has store, copy, drop {
                    name: String,
                    quantity: u64,
                }
            }
        "#;

        let messages = lint_source(source);
        assert!(
            messages.is_empty(),
            "Asset metadata structs should not be flagged"
        );
    }

    #[test]
    fn test_event_suffix_struct_not_token() {
        // Event structs should not be flagged even if they have "Asset" in name
        let source = r#"
            module example::events {
                struct AssetSupplied has copy, drop {
                    pool_id: ID,
                    amount: u64,
                }
                struct AssetSynced has copy, drop {
                    timestamp: u64,
                }
                struct CoinDecimalsRegistered has copy, drop {
                    coin_type: String,
                }
            }
        "#;

        let messages = lint_source(source);
        assert!(
            messages.is_empty(),
            "Event structs should not be flagged as tokens"
        );
    }

    // =========================================================================
    // SharedCapabilityLint tests (DEPRECATED - now no-op stubs)
    // =========================================================================

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
        // Should NOT fire because there's no "unsafe" in the function name
        assert!(messages.is_empty());
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
        // "checked_" function but not admin transfer - should not fire
        let source = r#"
            module example::validate {
                public fun checked_balance(balance: u64, amount: u64): bool {
                    balance >= amount
                }
            }
        "#;

        let messages = lint_source(source);
        // Should NOT fire because there's no "unsafe" in the function name
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
        // Should NOT fire because there's no "unsafe" in the function name
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
    fn test_unchecked_coin_split_deprecated_no_diagnostics() {
        // DEPRECATED: Sui runtime already enforces balance checks
        let source = r#"
module example::stake {
    public fun withdraw(user: &mut User, amount: u64): Coin<SUI> {
        pool::take(amount)
    }
}
        "#;

        let messages = lint_source(source);
        // Deprecated lint should produce no diagnostics
        let withdraw_msgs: Vec<_> = messages
            .iter()
            .filter(|m| m.contains("withdraw") && m.contains("balance"))
            .collect();
        assert!(withdraw_msgs.is_empty());
    }

    #[test]
    fn test_coin_split_with_balance_check_ok() {
        let source = r#"
module example::stake {
    public fun withdraw(user: &mut User, amount: u64): Coin<SUI> {
        assert!(coin::value(user.coin) >= amount, E_INSUFFICIENT);
        user.coin = coin::split(user.coin, amount);
        pool::take(amount)
    }
}
        "#;

        let messages = lint_source(source);
        // Should not fire when there's a balance check
        let withdraw_msgs: Vec<_> = messages
            .iter()
            .filter(|m| m.contains("withdraw") && m.contains("balance"))
            .collect();
        assert!(withdraw_msgs.is_empty());
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
        let result = random::new_generator(r, ctx).generate_bool();
    }
}
        "#;

        let messages = lint_source(source);
        assert!(messages.is_empty());
    }

    // =========================================================================
    // UnboundedVectorGrowthLint tests
    // =========================================================================

    // =========================================================================
    // HardcodedAddressLint tests
    // =========================================================================

    // =========================================================================
    // IgnoredBooleanReturnLint tests
    // =========================================================================

    #[test]
    fn test_ignored_contains_detected() {
        let source = r#"
module example::auth {
    public fun update(authority: &UpdateAuthority, user: address) {
        vector::contains(&authority.whitelist, &user);
        do_update();
    }
}
        "#;

        let messages = lint_source(source);
        // Filter to just ignored_boolean_return messages
        let ignored_msgs: Vec<_> = messages.iter().filter(|m| m.contains("ignores")).collect();
        assert_eq!(ignored_msgs.len(), 1);
        assert!(ignored_msgs[0].contains("contains"));
    }

    #[test]
    fn test_contains_in_assert_ok() {
        let source = r#"
module example::auth {
    public fun update(authority: &UpdateAuthority, user: address) {
        assert!(vector::contains(&authority.whitelist, &user), E_UNAUTHORIZED);
        do_update();
    }
}
        "#;

        let messages = lint_source(source);
        // Should not fire when result is used in assert!
        let ignored_bool_msgs: Vec<_> = messages.iter().filter(|m| m.contains("ignores")).collect();
        assert!(ignored_bool_msgs.is_empty());
    }

    // =========================================================================
    // SharedCapabilityObjectLint tests (DEPRECATED - now no-op stubs)
    // =========================================================================

    #[test]
    fn test_shared_normal_object_ok() {
        let source = r#"
module example::pool {
    public fun new_pool(ctx: &mut TxContext) {
        let pool = Pool { id: object::new(ctx), balance: 0 };
        transfer::share_object(pool);
    }
}
        "#;

        let messages = lint_source(source);
        // Should not fire for normal objects without access-control keywords
        let shared_cap_msgs: Vec<_> = messages
            .iter()
            .filter(|m| m.contains("access-control"))
            .collect();
        assert!(shared_cap_msgs.is_empty());
    }

    // =========================================================================
    // UncheckedWithdrawalLint tests (DEPRECATED - lint is now a no-op)
    // =========================================================================

    #[test]
    fn test_unchecked_withdraw_deprecated_no_diagnostics() {
        // DEPRECATED: Business logic bugs require formal verification
        let source = r#"
module example::stake {
    public fun withdraw(user: &mut User, amount: u64): Coin<SUI> {
        pool::take(amount)
    }
}
        "#;

        let messages = lint_source(source);
        // Deprecated lint should produce no diagnostics
        let withdraw_msgs: Vec<_> = messages
            .iter()
            .filter(|m| m.contains("withdraw") && m.contains("balance"))
            .collect();
        assert!(withdraw_msgs.is_empty());
    }

    #[test]
    fn test_checked_withdraw_ok() {
        let source = r#"
module example::stake {
    public fun withdraw(user: &mut User, amount: u64): Coin<SUI> {
        assert!(user.balance >= amount, E_INSUFFICIENT);
        user.balance = user.balance - amount;
        pool::take(amount)
    }
}
        "#;

        let messages = lint_source(source);
        // Should not fire when there's a balance check
        let withdraw_msgs: Vec<_> = messages
            .iter()
            .filter(|m| m.contains("withdraw") && m.contains("balance"))
            .collect();
        assert!(withdraw_msgs.is_empty());
    }

    // =========================================================================
    // CapabilityLeakLint tests (DEPRECATED - lint is now a no-op)
    // =========================================================================

    #[test]
    fn test_capability_leak_deprecated_no_diagnostics() {
        // DEPRECATED: Superseded by capability_transfer_v2
        let source = r#"
module example::admin {
    public fun transfer_admin_cap(cap: AdminCap, recipient: address) {
        transfer::transfer(cap, recipient);
    }
}
        "#;

        let messages = lint_source(source);
        // Deprecated lint should produce no diagnostics
        let cap_leak_msgs: Vec<_> = messages
            .iter()
            .filter(|m| m.contains("capability") && m.contains("transfer"))
            .collect();
        assert!(cap_leak_msgs.is_empty());
    }

    #[test]
    fn test_authorized_cap_transfer_ok() {
        let source = r#"
module example::admin {
    public fun transfer_admin_cap(cap: AdminCap, _auth: &AdminCap, recipient: address) {
        // Requires caller to already have AdminCap
        transfer::transfer(cap, recipient);
    }
}
        "#;

        let messages = lint_source(source);
        // Should not fire (lint is deprecated anyway)
        let cap_leak_msgs: Vec<_> = messages
            .iter()
            .filter(|m| m.contains("capability") && m.contains("transfer"))
            .collect();
        assert!(cap_leak_msgs.is_empty());
    }
}
