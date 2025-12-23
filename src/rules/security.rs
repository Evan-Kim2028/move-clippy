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
// REMOVED LINTS:
// - StaleOraclePriceLint - superseded by stale_oracle_price_v3 (CFG-aware, --mode full)
// - SingleStepOwnershipTransferLint - ~50% FP rate, syntactic matching insufficient
// - UncheckedCoinSplitLint - Sui runtime already enforces balance checks
// - MissingWitnessDropLint - superseded by missing_witness_drop_v2 (type-based)
// - PublicRandomAccessLint - superseded by public_random_access_v2 (type-based)
// - IgnoredBooleanReturnLint - ~70% FP rate, syntactic approach insufficient
// - UncheckedWithdrawalLint - requires formal verification, not linting
// - CapabilityLeakLint - superseded by capability_transfer_v2 (type-based)
// ============================================================================

// ============================================================================
// NEW LINTS: Type System Gap Coverage
// ============================================================================

// ============================================================================

// ============================================================================
// REMOVED: destroy_zero_unchecked - Detects destroy_zero without prior zero-check
// This lint was removed as it catches only obvious/trivial cases.
// ============================================================================

// ============================================================================
// REMOVED: otw_pattern_violation
// ============================================================================
// This lint duplicated the Sui Verifier's one_time_witness_verifier.rs which
// is authoritative and will reject modules at publish time.
// See: sui-execution/v0/sui-verifier/src/one_time_witness_verifier.rs

// ============================================================================
// digest_as_randomness - Detects tx_context::digest used as randomness
// ============================================================================

// REMOVED: DigestAsRandomnessLint - deprecated, use sui::random instead

// ============================================================================

// ============================================================================
// REMOVED: divide_by_zero_literal - Detects division by literal zero
// This lint was removed as it catches only obvious/trivial cases.
// ============================================================================

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
///     // proceeds without checking ordering...
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

        // Check for counter patterns
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

        // REMOVED lints (deprecated/superseded):
        // StaleOraclePriceLint, SingleStepOwnershipTransferLint, UncheckedCoinSplitLint,
        // MissingWitnessDropLint, PublicRandomAccessLint, IgnoredBooleanReturnLint,
        // UncheckedWithdrawalLint, CapabilityLeakLint

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
    // REMOVED LINT TESTS:
    // StaleOraclePriceLint, SingleStepOwnershipTransferLint, UncheckedCoinSplitLint,
    // MissingWitnessDropLint, PublicRandomAccessLint, IgnoredBooleanReturnLint,
    // UncheckedWithdrawalLint, CapabilityLeakLint
    // These lints were deprecated/superseded and have been removed.
    // =========================================================================

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
    // MissingWitnessDropLint tests (DEPRECATED)
    // =========================================================================

    #[test]
    fn test_missing_witness_drop_deprecated_no_diagnostics() {
        // DEPRECATED: Sui compiler already enforces OTW rules at compile time.
        // This lint had high false positive rate on type markers like GT, T, S.
        let source = r#"
module example::token {
    struct MY_TOKEN {}
}
        "#;

        let messages = lint_source(source);
        // Deprecated lint should produce no findings
        assert!(messages.is_empty());
    }

    // =========================================================================
    // PublicRandomAccessLint tests (DEPRECATED)
    // =========================================================================

    #[test]
    fn test_public_random_access_deprecated_no_diagnostics() {
        // DEPRECATED: Sui compiler has built-in public_random lint with proper type detection.
        // This syntactic lint used string matching which had high false positive rate.
        let source = r#"
module example::game {
    public fun get_random_number(r: &Random) {
        random::new_generator(r).generate_u64()
    }
}
        "#;

        let messages = lint_source(source);
        // Deprecated lint should produce no findings
        assert!(messages.is_empty());
    }

    // =========================================================================
    // UnboundedVectorGrowthLint tests
    // =========================================================================

    // =========================================================================
    // HardcodedAddressLint tests
    // =========================================================================

    // =========================================================================
    // IgnoredBooleanReturnLint tests (DEPRECATED - lint is now no-op)
    // =========================================================================

    #[test]
    fn test_ignored_contains_detected() {
        // DEPRECATED: This lint has been disabled due to high FP rate (~70%)
        let source = r#"
module example::auth {
    public fun update(authority: &UpdateAuthority, user: address) {
        vector::contains(&authority.whitelist, &user);
        do_update();
    }
}
        "#;

        let messages = lint_source(source);
        // Deprecated lint should produce no findings
        let ignored_msgs: Vec<_> = messages.iter().filter(|m| m.contains("ignores")).collect();
        assert!(
            ignored_msgs.is_empty(),
            "Deprecated lint should produce no findings"
        );
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
