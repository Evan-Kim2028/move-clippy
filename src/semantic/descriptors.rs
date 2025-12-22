use crate::lint::{
    AnalysisKind, FixDescriptor, LintCategory, LintDescriptor, RuleGroup, TypeSystemGap,
};

// ============================================================================
// Sui Monorepo Lints (delegated from sui_mode::linters)
//
// These lints are pass-through wrappers for the official Sui Move compiler
// lints from the Sui monorepo. They run in --mode full and provide unified
// output formatting through move-clippy.
//
// Source: https://github.com/MystenLabs/sui/tree/main/external-crates/move/crates/move-compiler/src/sui_mode/linters
// ============================================================================
pub static SHARE_OWNED: LintDescriptor = LintDescriptor {
    name: "share_owned",
    category: LintCategory::Suspicious,
    description: "[Sui Linter] Possible owned object share (from sui_mode::linters)",
    group: RuleGroup::Experimental,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
    gap: Some(TypeSystemGap::OwnershipViolation),
};

pub static SELF_TRANSFER: LintDescriptor = LintDescriptor {
    name: "self_transfer",
    category: LintCategory::Suspicious,
    description: "[Sui Linter] Transferring object to self - consider returning instead (from sui_mode::linters)",
    group: RuleGroup::Experimental,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
    gap: Some(TypeSystemGap::OwnershipViolation),
};

pub static CUSTOM_STATE_CHANGE: LintDescriptor = LintDescriptor {
    name: "custom_state_change",
    category: LintCategory::Suspicious,
    description: "[Sui Linter] Custom transfer/share/freeze should call private variants (from sui_mode::linters)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
    gap: Some(TypeSystemGap::ApiMisuse),
};

pub static COIN_FIELD: LintDescriptor = LintDescriptor {
    name: "coin_field",
    category: LintCategory::Suspicious,
    description: "[Sui Linter] Use Balance instead of Coin in struct fields (from sui_mode::linters)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
    gap: Some(TypeSystemGap::ApiMisuse),
};

pub static FREEZE_WRAPPED: LintDescriptor = LintDescriptor {
    name: "freeze_wrapped",
    category: LintCategory::Suspicious,
    description: "[Sui Linter] Do not freeze objects containing wrapped objects (from sui_mode::linters)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
    gap: Some(TypeSystemGap::OwnershipViolation),
};

pub static COLLECTION_EQUALITY: LintDescriptor = LintDescriptor {
    name: "collection_equality",
    category: LintCategory::Suspicious,
    description: "[Sui Linter] Avoid equality checks on collections (from sui_mode::linters)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
    gap: Some(TypeSystemGap::ApiMisuse),
};

pub static PUBLIC_RANDOM: LintDescriptor = LintDescriptor {
    name: "public_random",
    category: LintCategory::Suspicious,
    description: "[Sui Linter] Random state should remain private (from sui_mode::linters)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
    gap: Some(TypeSystemGap::ApiMisuse),
};

pub static MISSING_KEY: LintDescriptor = LintDescriptor {
    name: "missing_key",
    category: LintCategory::Suspicious,
    description: "[Sui Linter] Shared/transferred object missing key ability (from sui_mode::linters)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
    gap: Some(TypeSystemGap::AbilityMismatch),
};

pub static FREEZING_CAPABILITY: LintDescriptor = LintDescriptor {
    name: "freezing_capability",
    category: LintCategory::Suspicious,
    description: "[Sui Linter] Avoid freezing capability objects (from sui_mode::linters)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
    gap: Some(TypeSystemGap::OwnershipViolation),
};

// ============================================================================
// Security Semantic Lints (type-based)
// ============================================================================

/// Detects division operations without zero-divisor validation.
///
/// # Security References
///
/// - **General Smart Contract Security**: Division by zero causes transaction abort
/// - **Sui Move**: No automatic zero-check for division operations
///
/// # Why This Matters
///
/// Division or modulo by zero will abort the transaction. If the divisor
/// comes from user input or external data, the function should validate
/// it before performing the division.
///
/// # Example (Bad)
///
/// ```move
/// public fun calculate_share(total: u64, count: u64): u64 {
///     total / count  // Will abort if count is 0!
/// }
/// ```
///
/// # Correct Pattern
///
/// ```move
/// public fun calculate_share(total: u64, count: u64): u64 {
///     assert!(count != 0, E_DIVISION_BY_ZERO);
///     total / count
/// }
/// ```
pub static UNCHECKED_DIVISION: LintDescriptor = LintDescriptor {
    name: "unchecked_division",
    category: LintCategory::Security,
    description: "Division without zero-check may abort transaction (type-based)",
    group: RuleGroup::Experimental,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
    gap: Some(TypeSystemGap::ArithmeticSafety),
};

/// Detects important return values that are ignored.
///
/// Some APIs signal failure via return values. Ignoring them can hide errors
/// or bypass safety checks.
pub static UNUSED_RETURN_VALUE: LintDescriptor = LintDescriptor {
    name: "unused_return_value",
    category: LintCategory::Security,
    description: "Important return value is ignored, may indicate bug (type-based)",
    group: RuleGroup::Experimental,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
    gap: Some(TypeSystemGap::ApiMisuse),
};

/// Detects entry functions that return non-unit values.
///
/// In Sui Move, entry function return values are discarded by the runtime.
/// This is a principled, zero-false-positive lint based on compiler semantics.
pub static ENTRY_FUNCTION_RETURNS_VALUE: LintDescriptor = LintDescriptor {
    name: "entry_function_returns_value",
    category: LintCategory::Suspicious,
    description: "Entry function returns a value that will be discarded by the runtime (type-based)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
    gap: Some(TypeSystemGap::ValueFlow),
};

/// Detects entry functions that are private (unreachable from transactions).
pub static PRIVATE_ENTRY_FUNCTION: LintDescriptor = LintDescriptor {
    name: "private_entry_function",
    category: LintCategory::Suspicious,
    description: "Private entry function is unreachable - remove `entry` or make it public (type-based)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
    gap: None,
};

/// Detects emitting non-event-like types via `event::emit<T>(...)`.
///
/// Event types should be `copy + drop` and should not have `key`.
pub static EVENT_EMIT_TYPE_SANITY: LintDescriptor = LintDescriptor {
    name: "event_emit_type_sanity",
    category: LintCategory::Security,
    description: "Emitting non-event-like type via event::emit (type-based, requires --mode full)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
    gap: Some(TypeSystemGap::ApiMisuse),
};

/// Detects event structs named with present tense verbs instead of past tense.
pub static EVENT_PAST_TENSE: LintDescriptor = LintDescriptor {
    name: "event_past_tense",
    category: LintCategory::Style,
    description: "Event name uses present tense instead of past tense (type-based, requires --mode full)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
    gap: None,
};

/// Detects sharing of objects with `key + store` abilities.
///
/// # Security Rationale
///
/// Objects with `key + store` abilities represent "transferable authority" in Sui Move:
/// - `key` = the object has identity (UID)
/// - `store` = the object can be transferred to another owner
///
/// When such objects are shared via `share_object()` or `public_share_object()`,
/// they become publicly accessible. This is dangerous for authority
/// objects (capabilities) but may be intentional for shared state objects.
///
/// # Type System Grounding
///
/// This lint is grounded in Move's ability system, not name heuristics:
/// - ALL Sui capabilities have `key + store` (TreasuryCap, UpgradeCap, etc.)
/// - A capability without `store` couldn't be transferred - useless
/// - A capability without `key` couldn't exist as an object - not a capability
///
/// # False Positives
///
/// This lint may fire on intentional shared state patterns (Kiosk, Pool objects).
/// Use `#[ext(move_clippy(allow(share_owned_authority)))]` to suppress for intentional cases.
///
/// # Example (Dangerous)
///
/// ```move
/// public fun init(ctx: &mut TxContext) {
///     let cap = TreasuryCap { id: object::new(ctx) };
///     transfer::share_object(cap);  // DANGEROUS: anyone can mint!
/// }
/// ```
///
/// # Correct Patterns
///
/// ```move
/// // Transfer to owner
/// public fun init(ctx: &mut TxContext) {
///     let cap = TreasuryCap { id: object::new(ctx) };
///     transfer::transfer(cap, tx_context::sender(ctx));
/// }
///
/// // Or intentional sharing with suppression
/// #[ext(move_clippy(allow(share_owned_authority)))]
/// public fun init(ctx: &mut TxContext) {
///     let kiosk = Kiosk { id: object::new(ctx) };
///     transfer::share_object(kiosk);  // Intentional for marketplace
/// }
/// ```
pub static SHARE_OWNED_AUTHORITY: LintDescriptor = LintDescriptor {
    name: "share_owned_authority",
    category: LintCategory::Security,
    description: "Sharing key+store object makes it publicly accessible - dangerous for authority objects (type-based)",
    group: RuleGroup::Preview,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
    gap: Some(TypeSystemGap::OwnershipViolation),
};

/// Detects sharing of capability-like objects.
///
/// This lint is intentionally conservative and currently tiered as Preview because
/// ability patterns alone may over-approximate what developers consider a "capability".
pub static SHARED_CAPABILITY_OBJECT: LintDescriptor = LintDescriptor {
    name: "shared_capability_object",
    category: LintCategory::Security,
    description: "Capability-like object is shared - potential authorization leak (type-based, preview)",
    group: RuleGroup::Preview,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
    gap: Some(TypeSystemGap::CapabilityEscape),
};

/// Detects capability-like transfers to literal addresses.
///
/// Narrow by design: only flags literal recipients to keep false positives low.
/// Uses ability-based detection (key+store, no copy/drop) combined with literal
/// address check for principled, low-false-positive detection.
pub static CAPABILITY_TRANSFER_LITERAL_ADDRESS: LintDescriptor = LintDescriptor {
    name: "capability_transfer_literal_address",
    category: LintCategory::Security,
    description: "Capability-like object transferred to a literal address - likely authorization leak (type-based)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
    gap: Some(TypeSystemGap::CapabilityEscape),
};

/// Detects public entry functions that take `&mut` key objects without any explicit authority parameter.
///
/// This is a heuristic lint (Preview): the authority may be implicit (owned objects) or enforced via
/// internal checks, so this is best used as an audit signal.
pub static MUT_KEY_PARAM_MISSING_AUTHORITY: LintDescriptor = LintDescriptor {
    name: "mut_key_param_missing_authority",
    category: LintCategory::Security,
    description: "Public entry takes &mut key object without explicit authority param (type-based, preview)",
    group: RuleGroup::Preview,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
    gap: Some(TypeSystemGap::CapabilityEscape),
};

/// Detects unbounded loops over a vector parameter.
///
/// In entry functions, vector parameters are attacker-controlled and can cause DoS via large loops.
pub static UNBOUNDED_ITERATION_OVER_PARAM_VECTOR: LintDescriptor = LintDescriptor {
    name: "unbounded_iteration_over_param_vector",
    category: LintCategory::Security,
    description: "Loop bound depends on vector parameter length - add explicit bound (type-based, preview)",
    group: RuleGroup::Preview,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
    gap: Some(TypeSystemGap::ResourceExhaustion),
};

/// Detects generic functions that accept a `type_name::TypeName` witness but never use it.
///
/// If a witness parameter is unused, the function may be missing a type validation check.
pub static GENERIC_TYPE_WITNESS_UNUSED: LintDescriptor = LintDescriptor {
    name: "generic_type_witness_unused",
    category: LintCategory::Security,
    description: "Generic function takes TypeName witness but never uses it (type-based, experimental)",
    group: RuleGroup::Experimental,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
    gap: Some(TypeSystemGap::TypeConfusion),
};

/// Detects structs that should be hot potatoes but have the `drop` ability.
///
/// # Type System Grounding
///
/// A "hot potato" is a struct with NO abilities at all:
/// - No `key` (not an object)
/// - No `store` (cannot be stored)
/// - No `copy` (cannot be duplicated)
/// - No `drop` (MUST be consumed)
///
/// The security property comes from the LACK OF `drop`. If a hot potato
/// has `drop`, it can be silently discarded, breaking the enforcement model.
///
/// # Security References
///
/// - **Trail of Bits (2025)**: "How Sui Move rethinks flash loan security"
/// - **Mirage Audits (2025)**: "The Accidental Droppable Hot Potato"
///
/// # Why This Matters
///
/// Adding `drop` to a hot potato silently breaks the security model.
/// The compiler accepts it, but attackers can borrow assets and simply
/// drop the receipt without repaying.
///
/// # Detection Logic
///
/// This lint fires when a struct has ONLY the `drop` ability (no other abilities).
/// A struct with `copy + drop` is NOT flagged (they're events/DTOs).
/// A struct with `key + store` is NOT flagged (they're resources).
///
/// # Example (Bad)
///
/// ```move
/// struct FlashLoanReceipt has drop {  // BUG: drop enables theft!
///     pool_id: ID,
///     amount: u64,
/// }
/// ```
///
/// # Correct Pattern
///
/// ```move
/// struct FlashLoanReceipt {  // No abilities = true hot potato
///     pool_id: ID,
///     amount: u64,
/// }
/// ```
///
/// DEPRECATED: This lint has a ~67% false positive rate because it flags ALL structs
/// with only `drop` ability, including legitimate drop-only types like RandomGenerator,
/// Receiving<T>, etc. Use `droppable_flash_loan_receipt` instead, which detects the
/// actual security-critical pattern: functions returning Coin/Balance with a droppable receipt.
pub static DROPPABLE_HOT_POTATO_V2: LintDescriptor = LintDescriptor {
    name: "droppable_hot_potato_v2",
    category: LintCategory::Security,
    description: "Struct has only `drop` ability (deprecated: use droppable_flash_loan_receipt for accurate detection)",
    group: RuleGroup::Deprecated,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
    gap: Some(TypeSystemGap::ApiMisuse),
};

/// Detects droppable receipts returned alongside Coin/Balance values.
///
/// Flash loan receipts should not have `drop`; otherwise borrowers can ignore repayment.
/// This is the recommended lint for detecting broken hot potato patterns in flash loans,
/// replacing the deprecated `droppable_hot_potato_v2` which had high false positive rates.
pub static DROPPABLE_FLASH_LOAN_RECEIPT: LintDescriptor = LintDescriptor {
    name: "droppable_flash_loan_receipt",
    category: LintCategory::Security,
    description: "Function returns Coin/Balance with a droppable receipt struct (type-based, requires --mode full --preview)",
    group: RuleGroup::Preview,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
    gap: Some(TypeSystemGap::AbilityMismatch),
};

/// Detects receipt structs that fail to preserve coin type via phantom parameters.
pub static RECEIPT_MISSING_PHANTOM_TYPE: LintDescriptor = LintDescriptor {
    name: "receipt_missing_phantom_type",
    category: LintCategory::Security,
    description: "Receipt returned without phantom coin type enables type confusion (type-based, experimental)",
    group: RuleGroup::Experimental,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
    gap: Some(TypeSystemGap::TypeConfusion),
};

/// Detects copyable fungible value types that can be duplicated.
pub static COPYABLE_FUNGIBLE_TYPE: LintDescriptor = LintDescriptor {
    name: "copyable_fungible_type",
    category: LintCategory::Security,
    description: "Copyable fungible value type can be duplicated (type-based, experimental)",
    group: RuleGroup::Experimental,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
    gap: Some(TypeSystemGap::AbilityMismatch),
};

/// Detects structs that are transferable (`key + store`) but also copyable.
///
/// A `key + store + copy` type is almost always a severe bug:
/// - If it represents an asset, copyability defeats scarcity/accounting.
/// - If it represents authority, copyability defeats uniqueness of privileges.
pub static COPYABLE_CAPABILITY: LintDescriptor = LintDescriptor {
    name: "copyable_capability",
    category: LintCategory::Security,
    description: "Struct is key+store+copy - transferable authority/asset can be duplicated (type-based, zero FP)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
    gap: Some(TypeSystemGap::AbilityMismatch),
};

/// Detects structs that are transferable (`key + store`) but also droppable.
///
/// A `key + store + drop` type can be silently discarded, which often breaks invariants:
/// - capabilities can be destroyed to bypass obligations
/// - assets can be dropped to evade repayment/accounting
pub static DROPPABLE_CAPABILITY: LintDescriptor = LintDescriptor {
    name: "droppable_capability",
    category: LintCategory::Security,
    description: "Struct is key+store+drop (and not copy) - transferable authority/asset can be silently discarded (type-based, zero FP)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
    gap: Some(TypeSystemGap::AbilityMismatch),
};

/// Detects objects that are non-transferable (`key` but not `store`) but behave like fungible values.
///
/// A `key` type without `store` is a legitimate “soulbound object” pattern. Adding `copy` or `drop`
/// makes it incoherent: it becomes duplicable/discardable but still non-transferable.
pub static NON_TRANSFERABLE_FUNGIBLE_OBJECT: LintDescriptor = LintDescriptor {
    name: "non_transferable_fungible_object",
    category: LintCategory::Security,
    description: "Struct is key without store but has copy/drop - incoherent non-transferable fungible object (type-based, zero FP)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
    gap: Some(TypeSystemGap::AbilityMismatch),
};

/// Detects capability transfers to non-sender addresses.
///
/// # Type System Grounding
///
/// A capability is a struct with:
/// - `key` (is an object)
/// - `store` (can be transferred)
/// - NO `copy` (cannot be duplicated)
/// - NO `drop` (cannot be silently discarded)
///
/// This lint flags when such objects are transferred to addresses other than
/// `tx_context::sender(ctx)`, which may indicate a capability leak.
///
/// # Security References
///
/// - **MoveScanner (2025)**: Capability leak detection patterns
/// - **OtterSec Audits**: Multiple findings of capabilities transferred incorrectly
///
/// # Example (Suspicious)
///
/// ```move
/// public fun transfer_admin(cap: AdminCap, recipient: address) {
///     transfer::transfer(cap, recipient);  // Who is recipient?
/// }
/// ```
///
/// # Correct Pattern
///
/// ```move
/// public fun claim_admin(cap: AdminCap, ctx: &TxContext) {
///     transfer::transfer(cap, tx_context::sender(ctx));  // Explicit sender
/// }
/// ```
pub static CAPABILITY_TRANSFER_V2: LintDescriptor = LintDescriptor {
    name: "capability_transfer_v2",
    category: LintCategory::Security,
    description: "Capability transferred to non-sender address (type-based, requires --mode full --experimental)",
    group: RuleGroup::Experimental,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
    gap: Some(TypeSystemGap::OwnershipViolation),
};

/// Detects public (non-entry) functions that expose `sui::random::Random` objects.
///
/// # Security References
///
/// - **Sui Documentation**: "Randomness"
///   URL: <https://docs.sui.io/guides/developer/advanced/randomness>
///   Verified: 2024-12-13 (Random must be private)
///
/// # Why This Matters
///
/// `Random` objects should never be exposed publicly because:
/// 1. Validators can see the random value before including the transaction
/// 2. This enables front-running and manipulation of random outcomes
/// 3. Random values should only be consumed within the same PTB
///
/// # Example (Bad)
///
/// ```move
/// public fun get_random(r: &Random): u64 {
///     random::new_generator(r).generate_u64()
/// }
/// ```
///
/// # Correct Pattern
///
/// ```move
/// entry fun flip_coin(r: &Random, ctx: &mut TxContext) {
///     let gen = random::new_generator(r, ctx);
///     let result = gen.generate_bool();
///     // Use result internally, don't return it
/// }
/// ```
///
/// # Type-Based Detection
///
/// This lint uses type-based detection to identify `0x2::random::Random` parameters,
/// avoiding false positives from similarly-named custom types.
pub static PUBLIC_RANDOM_ACCESS_V2: LintDescriptor = LintDescriptor {
    name: "public_random_access_v2",
    category: LintCategory::Security,
    description: "Public function exposes sui::random::Random object - enables front-running (type-based, requires --mode full)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
    gap: Some(TypeSystemGap::ApiMisuse),
};

/// Detects one-time witness (OTW) structs with pattern violations.
///
/// Uses the compiler's module context to verify struct name matches module name.
pub static MISSING_WITNESS_DROP_V2: LintDescriptor = LintDescriptor {
    name: "missing_witness_drop_v2",
    category: LintCategory::Security,
    description: "OTW struct name doesn't match module name or missing drop (type-based, requires --mode full)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
    gap: Some(TypeSystemGap::AbilityMismatch),
};

/// Detects one-time witness (OTW) structs that violate Sui Adapter rules.
pub static INVALID_OTW: LintDescriptor = LintDescriptor {
    name: "invalid_otw",
    category: LintCategory::Security,
    description: "One-time witness violates Sui Adapter rules - has wrong abilities, fields, or is generic (type-based)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
    gap: Some(TypeSystemGap::AbilityMismatch),
};

/// Detects witness structs with antipatterns that may indicate security issues.
pub static WITNESS_ANTIPATTERNS: LintDescriptor = LintDescriptor {
    name: "witness_antipatterns",
    category: LintCategory::Security,
    description: "Witness struct has copy/store/key ability or public constructor - may defeat proof pattern (type-based)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
    gap: Some(TypeSystemGap::AbilityMismatch),
};

/// Detects public functions returning capability types.
///
/// DEPRECATED: This lint cannot be implemented with principled detection.
///
/// - Name-based detection (`*Cap`) produces false positives on non-capabilities
///   and false negatives on capabilities with different naming conventions.
/// - Ability-based detection (key+store, no copy/drop) is too broad - it flags
///   ALL public object factory functions (pools, positions, accounts), not just
///   security-sensitive capability creation.
///
/// The important security cases are covered by dedicated, principled lints:
/// - `copyable_capability`: key+store+copy (allows duplication)
/// - `droppable_capability`: key+store+drop (allows silent discard)
/// - `capability_transfer_v2`: transfer to literal address
pub static CAPABILITY_ANTIPATTERNS: LintDescriptor = LintDescriptor {
    name: "capability_antipatterns",
    category: LintCategory::Security,
    description: "[DEPRECATED] Public function returns capability - superseded by copyable_capability and droppable_capability",
    group: RuleGroup::Deprecated,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
    gap: Some(TypeSystemGap::CapabilityEscape),
};

/// Detects usage of unsafe oracle price functions from known oracle providers.
///
/// Uses type-based detection to verify the call is to a known oracle module.
///
/// DEPRECATED: Superseded by stale_oracle_price_v3 which uses CFG-aware dataflow
/// analysis to track whether prices are validated before use.
pub static STALE_ORACLE_PRICE_V2: LintDescriptor = LintDescriptor {
    name: "stale_oracle_price_v2",
    category: LintCategory::Security,
    description: "Using get_price_unsafe from known oracle may return stale prices (deprecated: use v3 with --preview)",
    group: RuleGroup::Deprecated,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
    gap: Some(TypeSystemGap::TemporalOrdering),
};

// NOTE: The following lints are implemented elsewhere or require future work:
// - phantom_capability: Implemented in absint_lints.rs (CFG-aware)
// - unused_hot_potato: Implemented in absint_lints.rs (CFG-aware dataflow analysis)

// ============================================================================
// ============================================================================
// Lint Registry
// ============================================================================

/// ## Extension Point: Adding a semantic (type-based) lint
///
/// Semantic lints run in `--mode full` and rely on Move compiler typing information.
///
/// To add a new semantic lint:
/// 1. Define a `static LintDescriptor` in this module (pick `group` and `analysis` carefully).
/// 2. Add the descriptor to `DESCRIPTORS` so it shows up in `list-rules` and generated docs.
/// 3. Implement the check in the `full` module and call it from `full::lint_package` in the
///    appropriate gating block (`Stable`, `Preview` behind `preview`, `Experimental` behind
///    `experimental`).
/// 4. Add a minimal fixture package under `tests/fixtures/` and a snapshot entry in
///    `tests/semantic_package_snapshots.rs` (plus an allow/deny/expect test if directives matter).
static DESCRIPTORS: &[&LintDescriptor] = &[
    // Naming (type-based)
    // Sui-delegated (production, type-based)
    &SHARE_OWNED,
    &SELF_TRANSFER,
    &CUSTOM_STATE_CHANGE,
    &COIN_FIELD,
    &FREEZE_WRAPPED,
    &COLLECTION_EQUALITY,
    &PUBLIC_RANDOM,
    &MISSING_KEY,
    &FREEZING_CAPABILITY,
    // Security (stable, type-grounded)
    &EVENT_EMIT_TYPE_SANITY,
    &EVENT_PAST_TENSE,
    &ENTRY_FUNCTION_RETURNS_VALUE,
    &PRIVATE_ENTRY_FUNCTION,
    &COPYABLE_CAPABILITY,
    &DROPPABLE_CAPABILITY,
    &CAPABILITY_ANTIPATTERNS,
    &NON_TRANSFERABLE_FUNGIBLE_OBJECT,
    &PUBLIC_RANDOM_ACCESS_V2,
    &MISSING_WITNESS_DROP_V2,
    &INVALID_OTW,
    &WITNESS_ANTIPATTERNS,
    &STALE_ORACLE_PRICE_V2,
    // Security (preview, type-based)
    &SHARED_CAPABILITY_OBJECT,
    &CAPABILITY_TRANSFER_LITERAL_ADDRESS,
    &MUT_KEY_PARAM_MISSING_AUTHORITY,
    &UNBOUNDED_ITERATION_OVER_PARAM_VECTOR,
    // Security (experimental, type-based)
    &UNCHECKED_DIVISION,
    &UNUSED_RETURN_VALUE,
    &SHARE_OWNED_AUTHORITY,
    &DROPPABLE_HOT_POTATO_V2,
    &DROPPABLE_FLASH_LOAN_RECEIPT,
    &RECEIPT_MISSING_PHANTOM_TYPE,
    &COPYABLE_FUNGIBLE_TYPE,
    &CAPABILITY_TRANSFER_V2,
    &GENERIC_TYPE_WITNESS_UNUSED,
    // NOTE: phantom_capability is in absint_lints.rs (CFG-aware)
    // NOTE: unused_hot_potato requires dataflow analysis (future work)
];

/// Return descriptors for all semantic lints.
pub fn descriptors() -> &'static [&'static LintDescriptor] {
    DESCRIPTORS
}

/// Look up a semantic lint descriptor by name.
pub fn find_descriptor(name: &str) -> Option<&'static LintDescriptor> {
    descriptors().iter().copied().find(|d| d.name == name)
}
