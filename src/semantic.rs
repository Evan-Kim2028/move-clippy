// Allow patterns that are intentional in semantic analysis
// - unused_variables: Move compiler iterators yield (key, value) pairs but we often only need value
// - unreachable_patterns: Match arms for exhaustiveness that may not be reached in practice
#![allow(unused_variables)]
#![allow(unreachable_patterns)]

use crate::diagnostics::Diagnostic;
use crate::error::{ClippyResult, MoveClippyError};
use crate::lint::{
    AnalysisKind, FixDescriptor, LintCategory, LintDescriptor, LintSettings, RuleGroup,
    TypeSystemGap,
};
use std::path::Path;

/// Semantic lints that rely on Move compiler typing information.
///
/// These lints are only available when `move-clippy` is built with the
/// `full` feature and run in `--mode full` against a Move package.
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
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
    gap: Some(TypeSystemGap::OwnershipViolation),
};

pub static SELF_TRANSFER: LintDescriptor = LintDescriptor {
    name: "self_transfer",
    category: LintCategory::Suspicious,
    description: "[Sui Linter] Transferring object to self - consider returning instead (from sui_mode::linters)",
    group: RuleGroup::Stable,
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
pub static UNUSED_RETURN_VALUE: LintDescriptor = LintDescriptor {
    name: "unused_return_value",
    category: LintCategory::Security,
    description: "Important return value is ignored, may indicate bug (type-based)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
    gap: Some(TypeSystemGap::ApiMisuse),
};

/// Detects entry functions that return non-unit values.
///
/// In Sui Move, entry function return values are discarded by the runtime.
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
    group: RuleGroup::Stable,
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
pub static CAPABILITY_TRANSFER_LITERAL_ADDRESS: LintDescriptor = LintDescriptor {
    name: "capability_transfer_literal_address",
    category: LintCategory::Security,
    description: "Capability-like object transferred to a literal address - likely authorization leak (type-based, preview)",
    group: RuleGroup::Preview,
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
pub static DROPPABLE_HOT_POTATO_V2: LintDescriptor = LintDescriptor {
    name: "droppable_hot_potato_v2",
    category: LintCategory::Security,
    description: "Struct has only `drop` ability - likely a broken hot potato (type-based, zero FP)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
    gap: Some(TypeSystemGap::ApiMisuse),
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
///   URL: https://docs.sui.io/guides/developer/advanced/randomness
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
    &ENTRY_FUNCTION_RETURNS_VALUE,
    &PRIVATE_ENTRY_FUNCTION,
    &SHARE_OWNED_AUTHORITY,
    &COPYABLE_CAPABILITY,
    &DROPPABLE_CAPABILITY,
    &NON_TRANSFERABLE_FUNGIBLE_OBJECT,
    &PUBLIC_RANDOM_ACCESS_V2,
    // Security (preview, type-based)
    &SHARED_CAPABILITY_OBJECT,
    &UNUSED_RETURN_VALUE,
    &DROPPABLE_HOT_POTATO_V2,
    &CAPABILITY_TRANSFER_LITERAL_ADDRESS,
    &MUT_KEY_PARAM_MISSING_AUTHORITY,
    &UNBOUNDED_ITERATION_OVER_PARAM_VECTOR,
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

#[cfg(feature = "full")]
mod full {
    use super::*;
    use crate::absint_lints;
    use crate::cross_module_lints;
    use crate::diagnostics::Span;
    use crate::instrument_block;
    use crate::level::LintLevel;
    use crate::rules::modernization::{PUBLIC_MUT_TX_CONTEXT, UNNECESSARY_PUBLIC_ENTRY};
    use crate::suppression;
    use crate::type_classifier;
    type Result<T> = ClippyResult<T>;
    use move_compiler::command_line::compiler::Visitor;
    use move_compiler::editions::Flavor;
    use move_compiler::parser::ast::TargetKind;
    use move_compiler::shared::{Identifier, files::MappedFiles, program_info::TypingProgramInfo};
    use move_compiler::shared::{SaveFlag, SaveHook};
    use move_compiler::sui_mode::linters;
    use move_compiler::{naming::ast as N, typing::ast as T};
    use move_ir_types::location::Loc;
    use move_package::BuildConfig;
    use move_package::compilation::build_plan::BuildPlan;

    fn descriptor_for_absint_diag(
        info: &move_compiler::diagnostics::codes::DiagnosticInfo,
    ) -> Option<&'static LintDescriptor> {
        // Only treat warnings emitted by our Phase II visitors as Phase II lints.
        //
        // AbsInt lints emit `custom("Lint", ..., category=50, code=...)` (see `absint_lints.rs`),
        // which renders as `warning[LintW5000X] ...`. The compiler also emits many unrelated
        // warnings with small numeric `code()` values (e.g., UnusedItem::Alias), so filtering
        // only on `code()` will misclassify those as Phase II lints.
        if info.external_prefix() != Some("Lint") || info.category() != 50 {
            return None;
        }

        crate::absint_lints::descriptor_for_diag_code(info.code())
    }

    /// Run all semantic lints against the package rooted at `package_path`.
    pub fn lint_package(
        package_path: &Path,
        settings: &LintSettings,
        preview: bool,
        experimental: bool,
    ) -> ClippyResult<Vec<Diagnostic>> {
        instrument_block!("semantic::lint_package", {
            let package_root = std::fs::canonicalize(package_path)?;
            let mut writer = Vec::<u8>::new();
            let mut build_config = BuildConfig::default();
            build_config.default_flavor = Some(Flavor::Sui);
            // Isolate build artifacts per invocation so tests (and parallel runs) don't race by
            // writing into the fixture/package directory.
            let install_dir = tempfile::tempdir()?;
            build_config.install_dir = Some(install_dir.path().to_path_buf());
            let resolved_graph =
                build_config.resolution_graph_for_package(&package_root, None, &mut writer)?;
            let build_plan = BuildPlan::create(&resolved_graph)?;

            let hook = SaveHook::new([SaveFlag::Typing, SaveFlag::TypingInfo]);

            // Get Phase II visitors (SimpleAbsInt-based lints)
            let phase2_visitors: Vec<Visitor> =
                absint_lints::create_visitors(preview, experimental)
                    .into_iter()
                    .map(Visitor::AbsIntVisitor)
                    .collect();

            // IMPORTANT: avoid `compile_no_exit` here; it prints compiler diagnostics to stdout,
            // which corrupts `--format json` output for ecosystem validation. Instead, capture
            // warnings and convert them into JSON diagnostics.
            let collected_phase2 = std::cell::RefCell::new(Vec::new());
            let deps = build_plan.compute_dependencies();
            let compiled =
                build_plan.compile_with_driver_and_deps(deps, &mut writer, |compiler| {
                    use move_compiler::diagnostics::report_diagnostics_to_buffer_with_env_color;

                    let (attr, filters) = linters::known_filters();
                    let compiler = compiler
                        .add_save_hook(&hook)
                        .add_custom_known_filters(attr, filters)
                        .add_visitors(phase2_visitors);

                    let (files, res) = compiler.build()?;
                    match res {
                        Ok((units, warnings)) => {
                            collected_phase2
                                .borrow_mut()
                                .push((files.clone(), warnings));
                            Ok((files, units))
                        }
                        Err(errors) => {
                            let rendered =
                                report_diagnostics_to_buffer_with_env_color(&files, errors);
                            Err(MoveClippyError::semantic(format!(
                                "Move compilation failed while running Phase II visitors:\n{}",
                                String::from_utf8_lossy(&rendered)
                            ))
                            .into_anyhow())
                        }
                    }
                })?;

            let typing_ast: T::Program = hook.take_typing_ast();
            let typing_info: std::sync::Arc<TypingProgramInfo> = hook.take_typing_info();
            let file_map: MappedFiles = compiled.file_map.clone();

            let mut out = Vec::new();

            // Phase II: convert AbsInt visitor diagnostics into our JSON diagnostics.
            for (_files, warnings) in collected_phase2.into_inner() {
                for compiler_diag in warnings.into_vec() {
                    let Some(descriptor) = descriptor_for_absint_diag(compiler_diag.info()) else {
                        continue;
                    };
                    if let Some(diag) =
                        convert_compiler_diagnostic(&compiler_diag, settings, &file_map, descriptor)
                    {
                        out.push(diag);
                    }
                }
            }

            // Type-based naming lints
            // Type-based security lints
            lint_unchecked_division(&mut out, settings, &file_map, &typing_ast)?;
            lint_unused_return_value(&mut out, settings, &file_map, &typing_ast)?;
            lint_entry_function_returns_value(&mut out, settings, &file_map, &typing_ast)?;
            lint_private_entry_function(&mut out, settings, &file_map, &typing_ast)?;
            lint_event_emit_type_sanity(&mut out, settings, &file_map, &typing_ast)?;
            lint_share_owned_authority(&mut out, settings, &file_map, &typing_ast)?;
            lint_droppable_hot_potato_v2(&mut out, settings, &file_map, &typing_info)?;
            lint_copyable_capability(&mut out, settings, &file_map, &typing_info)?;
            lint_droppable_capability(&mut out, settings, &file_map, &typing_info)?;
            lint_non_transferable_fungible_object(&mut out, settings, &file_map, &typing_info)?;
            lint_public_random_access_v2(&mut out, settings, &file_map, &typing_ast)?;
            // Phase 4 security lints (type-based, preview)
            if preview {
                lint_shared_capability_object(&mut out, settings, &file_map, &typing_ast)?;
                lint_capability_transfer_literal_address(
                    &mut out,
                    settings,
                    &file_map,
                    &typing_ast,
                )?;
                lint_mut_key_param_missing_authority(&mut out, settings, &file_map, &typing_ast)?;
                lint_unbounded_iteration_over_param_vector(
                    &mut out,
                    settings,
                    &file_map,
                    &typing_ast,
                )?;
            }
            // Phase 4 security lints (type-based, experimental)
            if experimental {
                lint_capability_transfer_v2(&mut out, settings, &file_map, &typing_ast)?;
                lint_generic_type_witness_unused(&mut out, settings, &file_map, &typing_ast)?;
            }
            // Note: phantom_capability is implemented in absint_lints.rs (CFG-aware)

            // Phase III: Cross-module analysis lints (type-based)
            if experimental {
                lint_cross_module_lints(&mut out, settings, &file_map, &typing_ast, &typing_info)?;
            }

            // Sui-delegated lints (type-based, production)
            lint_sui_visitors(&mut out, settings, &build_plan, &package_root)?;

            // Filter Preview-group diagnostics when preview is disabled
            if !preview {
                out.retain(|d| d.lint.group != RuleGroup::Preview);
            }

            // Filter Experimental-group diagnostics when experimental is disabled
            if !experimental {
                out.retain(|d| d.lint.group != RuleGroup::Experimental);
            }

            append_unfulfilled_expectations(&mut out, &typing_ast, &file_map);

            Ok(out)
        })
    }

    /// Run cross-module analysis lints (Phase III)
    fn lint_cross_module_lints(
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        prog: &T::Program,
        info: &TypingProgramInfo,
    ) -> Result<()> {
        // Run transitive capability leak detection
        let cap_leak_diags = cross_module_lints::lint_transitive_capability_leak(prog, info);
        for compiler_diag in cap_leak_diags {
            if let Some(diag) = convert_compiler_diagnostic(
                &compiler_diag,
                settings,
                file_map,
                &cross_module_lints::TRANSITIVE_CAPABILITY_LEAK,
            ) {
                out.push(diag);
            }
        }

        // Run flashloan repayment analysis
        let flashloan_diags = cross_module_lints::lint_flashloan_without_repay(prog, info);
        for compiler_diag in flashloan_diags {
            if let Some(diag) = convert_compiler_diagnostic(
                &compiler_diag,
                settings,
                file_map,
                &cross_module_lints::FLASHLOAN_WITHOUT_REPAY,
            ) {
                out.push(diag);
            }
        }

        // NOTE: lint_price_manipulation_window removed - used name-based heuristics

        Ok(())
    }

    /// Convert a CompilerDiagnostic to our Diagnostic type
    fn convert_compiler_diagnostic(
        compiler_diag: &move_compiler::diagnostics::Diagnostic,
        settings: &LintSettings,
        file_map: &MappedFiles,
        descriptor: &'static LintDescriptor,
    ) -> Option<Diagnostic> {
        // Check if this lint is enabled
        if settings.level_for(descriptor.name) == LintLevel::Allow {
            return None;
        }

        // Get the primary location and message from the compiler diagnostic
        let primary_loc = compiler_diag.primary_loc();
        let primary_msg = compiler_diag.primary_msg();

        // Convert location to our span format
        let (file, span, contents) = diag_from_loc(file_map, &primary_loc)?;

        Some(Diagnostic {
            lint: descriptor,
            level: LintLevel::Warn,
            file: Some(file),
            span,
            message: primary_msg.to_string(),
            help: None,
            suggestion: None,
        })
    }

    fn diag_from_loc(
        file_map: &MappedFiles,
        loc: &Loc,
    ) -> Option<(String, Span, std::sync::Arc<str>)> {
        let (fname, contents) = file_map.get(&loc.file_hash())?;
        let p = file_map.position_opt(loc)?;

        let file = fname.as_str().to_string();
        let span = Span {
            start: crate::diagnostics::Position {
                row: p.start.line_offset() + 1,
                column: p.start.column_offset() + 1,
            },
            end: crate::diagnostics::Position {
                row: p.end.line_offset() + 1,
                column: p.end.column_offset() + 1,
            },
        };

        Some((file, span, contents))
    }

    fn push_diag(
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        lint: &'static LintDescriptor,
        file: String,
        span: Span,
        source: &str,
        anchor_start: usize,
        message: String,
    ) {
        let module_scope = crate::annotations::module_scope(source);
        let item_scope = crate::annotations::item_scope(source, anchor_start);
        let level =
            crate::lint::effective_level_for_scopes(settings, lint, &module_scope, &item_scope);
        if level == LintLevel::Allow {
            return;
        }

        out.push(Diagnostic {
            lint,
            level,
            file: Some(file),
            span,
            message,
            help: None,
            suggestion: None,
        });
    }

    fn position_from_byte_offset(source: &str, byte_offset: usize) -> crate::diagnostics::Position {
        let mut row = 1usize;
        let mut col = 1usize;
        let end = byte_offset.min(source.len());
        for b in source.as_bytes().iter().take(end) {
            if *b == b'\n' {
                row += 1;
                col = 1;
            } else {
                col += 1;
            }
        }
        crate::diagnostics::Position { row, column: col }
    }

    fn append_unfulfilled_expectations(
        out: &mut Vec<Diagnostic>,
        prog: &T::Program,
        file_map: &MappedFiles,
    ) {
        use std::collections::{BTreeMap, BTreeSet};

        let mut fired: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for d in out.iter() {
            let Some(file) = d.file.as_deref() else {
                continue;
            };
            let entry = fired.entry(file.to_string()).or_default();
            entry.insert(d.lint.name.to_string());
            entry.insert(d.lint.category.as_str().to_string());
        }

        let mut module_expected: BTreeMap<String, (std::sync::Arc<str>, BTreeSet<String>)> =
            BTreeMap::new();
        let mut item_expected: BTreeMap<String, BTreeMap<usize, BTreeSet<String>>> =
            BTreeMap::new();

        for (_mident, mdef) in prog.modules.key_cloned_iter() {
            match mdef.target_kind {
                TargetKind::Source {
                    is_root_package: true,
                } => {}
                _ => continue,
            }

            // Collect module-level expectations once per file.
            let loc = mdef.loc;
            let Some((fname, contents)) = file_map.get(&loc.file_hash()) else {
                continue;
            };
            let file = fname.as_str().to_string();
            module_expected.entry(file.clone()).or_insert_with(|| {
                let scope = crate::annotations::module_scope(contents.as_ref());
                let expected: BTreeSet<String> = scope
                    .unfired_expectations()
                    .cloned()
                    .collect::<BTreeSet<_>>();
                (contents.clone(), expected)
            });

            // Collect item-level expectations for each function anchor.
            for (_fname, fdef) in mdef.functions.key_cloned_iter() {
                let anchor = fdef.loc.start() as usize;
                let scope = crate::annotations::item_scope(contents.as_ref(), anchor);
                let expected: BTreeSet<String> = scope
                    .unfired_expectations()
                    .cloned()
                    .collect::<BTreeSet<_>>();
                if expected.is_empty() {
                    continue;
                }
                item_expected
                    .entry(file.clone())
                    .or_default()
                    .entry(anchor)
                    .or_default()
                    .extend(expected);
            }
        }

        // Module-level unfulfilled expectations: require any matching lint or category in file.
        for (file, (contents, expected)) in module_expected {
            let fired_set = fired.get(&file);
            for name in expected {
                let fired_any = fired_set.is_some_and(|s| s.contains(&name));
                if fired_any {
                    continue;
                }

                out.push(Diagnostic {
                    lint: &crate::lint::UNFULFILLED_EXPECTATION,
                    level: LintLevel::Error,
                    file: Some(file.clone()),
                    span: Span {
                        start: crate::diagnostics::Position { row: 1, column: 1 },
                        end: crate::diagnostics::Position { row: 1, column: 1 },
                    },
                    message: format!(
                        "Expected `lint::{}` to produce a diagnostic in this file, but it did not",
                        name
                    ),
                    help: Some(
                        "Remove the `#![expect(...)]` directive or adjust the code/lint so it triggers."
                            .to_string(),
                    ),
                    suggestion: None,
                });
            }

            // Item-level unfulfilled expectations: approximate by file-level fired set.
            if let Some(anchors) = item_expected.get(&file) {
                let fired_set = fired.get(&file);
                for (&anchor, names) in anchors {
                    for name in names {
                        let fired_any = fired_set.is_some_and(|s| s.contains(name));
                        if fired_any {
                            continue;
                        }

                        let pos = position_from_byte_offset(contents.as_ref(), anchor);
                        out.push(Diagnostic {
                            lint: &crate::lint::UNFULFILLED_EXPECTATION,
                            level: LintLevel::Error,
                            file: Some(file.clone()),
                            span: Span { start: pos, end: pos },
                            message: format!(
                                "Expected `lint::{}` to produce a diagnostic in this scope, but it did not",
                                name
                            ),
                            help: Some(
                                "Remove the `#[expect(...)]` directive or adjust the code/lint so it triggers."
                                    .to_string(),
                            ),
                            suggestion: None,
                        });
                    }
                }
            }
        }
    }

    // =========================================================================
    // Droppable Hot Potato V2 Lint (type-based, zero FP)
    // =========================================================================

    /// Detect structs with ONLY the `drop` ability (no other abilities).
    ///
    /// A struct with only `drop` is almost always a bug:
    /// 1. If it's a hot potato, it should have NO abilities
    /// 2. If it's a witness, it should be empty
    ///
    /// This lint is type-based with zero false positives because:
    /// - Structs with `copy + drop` are events (legitimate)
    /// - Structs with `key + store` are resources (legitimate)
    /// - Structs with no abilities are hot potatoes (correct)
    /// - Structs with ONLY `drop` are broken hot potatoes (bug!)
    fn lint_droppable_hot_potato_v2(
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        info: &TypingProgramInfo,
    ) -> Result<()> {
        use crate::type_classifier::{
            has_copy_ability, has_drop_ability, has_key_ability, has_store_ability,
        };

        for (mident, minfo) in info.modules.key_cloned_iter() {
            match minfo.target_kind {
                TargetKind::Source {
                    is_root_package: true,
                } => {}
                _ => continue,
            }

            for (sname, sdef) in minfo.structs.key_cloned_iter() {
                let abilities = &sdef.abilities;

                // Check for "only drop" pattern: has drop, but no copy, no key, no store
                let has_only_drop = has_drop_ability(abilities)
                    && !has_copy_ability(abilities)
                    && !has_key_ability(abilities)
                    && !has_store_ability(abilities);

                if !has_only_drop {
                    continue;
                }

                // Skip empty structs (0 fields) - these are witness/marker types
                // Witness types legitimately have only `drop` ability
                let is_empty = match &sdef.fields {
                    N::StructFields::Defined(_, fields) => fields.is_empty(),
                    N::StructFields::Native(_) => true, // Native structs, skip them
                };
                if is_empty {
                    continue;
                }

                let sym = sname.value();
                let name_str = sym.as_str();
                let loc = sname.loc();
                let Some((file, span, contents)) = diag_from_loc(file_map, &loc) else {
                    continue;
                };
                let anchor = loc.start() as usize;

                push_diag(
                    out,
                    settings,
                    &DROPPABLE_HOT_POTATO_V2,
                    file,
                    span,
                    contents.as_ref(),
                    anchor,
                    format!(
                        "Struct `{name_str}` has only `drop` ability (no copy/key/store). \
                         If this is a hot potato, remove `drop` to enforce consumption. \
                         If this is a witness, ensure it has no fields. \
                         See: https://blog.trailofbits.com/2025/09/10/how-sui-move-rethinks-flash-loan-security/"
                    ),
                );
            }
        }

        Ok(())
    }

    // =========================================================================
    // Ability Mistake Lints (type-based, zero FP)
    // =========================================================================

    fn lint_copyable_capability(
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        info: &TypingProgramInfo,
    ) -> Result<()> {
        use crate::type_classifier::{has_copy_ability, has_key_ability, has_store_ability};

        for (_mident, minfo) in info.modules.key_cloned_iter() {
            match minfo.target_kind {
                TargetKind::Source {
                    is_root_package: true,
                } => {}
                _ => continue,
            }

            for (sname, sdef) in minfo.structs.key_cloned_iter() {
                let abilities = &sdef.abilities;
                let is_copyable_transferable = has_key_ability(abilities)
                    && has_store_ability(abilities)
                    && has_copy_ability(abilities);
                if !is_copyable_transferable {
                    continue;
                }

                let sym = sname.value();
                let name_str = sym.as_str();
                let loc = sname.loc();
                let Some((file, span, contents)) = diag_from_loc(file_map, &loc) else {
                    continue;
                };
                let anchor = loc.start() as usize;

                push_diag(
                    out,
                    settings,
                    &COPYABLE_CAPABILITY,
                    file,
                    span,
                    contents.as_ref(),
                    anchor,
                    format!(
                        "Struct `{name_str}` is `key + store + copy`. This creates a transferable, copyable authority/asset, \
                         which is almost always a severe security bug (privileges or value can be duplicated). Remove `copy`."
                    ),
                );
            }
        }

        Ok(())
    }

    fn lint_droppable_capability(
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        info: &TypingProgramInfo,
    ) -> Result<()> {
        use crate::type_classifier::{
            has_copy_ability, has_drop_ability, has_key_ability, has_store_ability,
        };

        for (_mident, minfo) in info.modules.key_cloned_iter() {
            match minfo.target_kind {
                TargetKind::Source {
                    is_root_package: true,
                } => {}
                _ => continue,
            }

            for (sname, sdef) in minfo.structs.key_cloned_iter() {
                let abilities = &sdef.abilities;
                let is_droppable_transferable = has_key_ability(abilities)
                    && has_store_ability(abilities)
                    && has_drop_ability(abilities)
                    && !has_copy_ability(abilities);
                if !is_droppable_transferable {
                    continue;
                }

                let sym = sname.value();
                let name_str = sym.as_str();
                let loc = sname.loc();
                let Some((file, span, contents)) = diag_from_loc(file_map, &loc) else {
                    continue;
                };
                let anchor = loc.start() as usize;

                push_diag(
                    out,
                    settings,
                    &DROPPABLE_CAPABILITY,
                    file,
                    span,
                    contents.as_ref(),
                    anchor,
                    format!(
                        "Struct `{name_str}` is `key + store + drop` (and not `copy`). This allows a transferable authority/asset to be silently discarded, \
                         which commonly breaks invariants (e.g., obligations can be bypassed). Remove `drop` or redesign the type."
                    ),
                );
            }
        }

        Ok(())
    }

    fn lint_non_transferable_fungible_object(
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        info: &TypingProgramInfo,
    ) -> Result<()> {
        use crate::type_classifier::{
            has_copy_ability, has_drop_ability, has_key_ability, has_store_ability,
        };

        for (_mident, minfo) in info.modules.key_cloned_iter() {
            match minfo.target_kind {
                TargetKind::Source {
                    is_root_package: true,
                } => {}
                _ => continue,
            }

            for (sname, sdef) in minfo.structs.key_cloned_iter() {
                let abilities = &sdef.abilities;
                let is_non_transferable =
                    has_key_ability(abilities) && !has_store_ability(abilities);
                let is_copy_or_drop = has_copy_ability(abilities) || has_drop_ability(abilities);
                if !(is_non_transferable && is_copy_or_drop) {
                    continue;
                }

                let sym = sname.value();
                let name_str = sym.as_str();
                let loc = sname.loc();
                let Some((file, span, contents)) = diag_from_loc(file_map, &loc) else {
                    continue;
                };
                let anchor = loc.start() as usize;

                push_diag(
                    out,
                    settings,
                    &NON_TRANSFERABLE_FUNGIBLE_OBJECT,
                    file,
                    span,
                    contents.as_ref(),
                    anchor,
                    format!(
                        "Struct `{name_str}` is `key` without `store` but has `copy` and/or `drop`. \
                         A `key`-without-`store` type is a legitimate non-transferable (soulbound) object pattern, \
                         but adding `copy`/`drop` makes it duplicable/discardable while still non-transferable. \
                         Remove `copy`/`drop` or redesign the type."
                    ),
                );
            }
        }

        Ok(())
    }

    // =========================================================================
    // Phase 4 Preview Lints (type-based)
    // =========================================================================

    fn exp_list_nth_single(args: &T::Exp, idx: usize) -> Option<&T::Exp> {
        match &args.exp.value {
            T::UnannotatedExp_::ExpList(items) => items.get(idx).and_then(|item| match item {
                T::ExpListItem::Single(e, _) => Some(e),
                _ => None,
            }),
            _ if idx == 0 => Some(args),
            _ => None,
        }
    }

    fn looks_like_address_literal(exp: &T::Exp) -> bool {
        match &exp.exp.value {
            T::UnannotatedExp_::Value(val) => {
                matches!(val.value, move_compiler::expansion::ast::Value_::Address(_))
            }
            _ => false,
        }
    }

    /// Detects sharing of capability-like objects via `transfer::share_object`.
    fn lint_shared_capability_object(
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        prog: &T::Program,
    ) -> Result<()> {
        const SHARE_FUNCTIONS: &[(&str, &str)] = &[
            ("transfer", "share_object"),
            ("transfer", "public_share_object"),
        ];

        for (mident, mdef) in prog.modules.key_cloned_iter() {
            match mdef.target_kind {
                TargetKind::Source {
                    is_root_package: true,
                } => {}
                _ => continue,
            }

            for (fname, fdef) in mdef.functions.key_cloned_iter() {
                let T::FunctionBody_::Defined((_use_funs, seq_items)) = &fdef.body.value else {
                    continue;
                };

                for item in seq_items.iter() {
                    check_shared_capability_in_seq_item(
                        item,
                        SHARE_FUNCTIONS,
                        out,
                        settings,
                        file_map,
                        fname.value().as_str(),
                    );
                }
            }
        }

        Ok(())
    }

    fn check_shared_capability_in_seq_item(
        item: &T::SequenceItem,
        share_fns: &[(&str, &str)],
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        func_name: &str,
    ) {
        match &item.value {
            T::SequenceItem_::Seq(exp) => {
                check_shared_capability_in_exp(exp, share_fns, out, settings, file_map, func_name);
            }
            T::SequenceItem_::Bind(_, _, exp) => {
                check_shared_capability_in_exp(exp, share_fns, out, settings, file_map, func_name);
            }
            _ => {}
        }
    }

    fn check_shared_capability_in_exp(
        exp: &T::Exp,
        share_fns: &[(&str, &str)],
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        func_name: &str,
    ) {
        use crate::type_classifier::is_capability_type_from_ty;

        if let T::UnannotatedExp_::ModuleCall(call) = &exp.exp.value {
            let module_sym = call.module.value.module.value();
            let module_name = module_sym.as_str();
            let call_sym = call.name.value();
            let call_name = call_sym.as_str();

            let is_share_call = share_fns
                .iter()
                .any(|(mod_pat, fn_pat)| module_name == *mod_pat && call_name == *fn_pat);

            if is_share_call
                && let Some(type_arg) = call.type_arguments.first()
                && !is_coin_type(&type_arg.value)
                && is_capability_type_from_ty(&type_arg.value)
            {
                let loc = exp.exp.loc;
                let Some((file, span, contents)) = diag_from_loc(file_map, &loc) else {
                    return;
                };
                let anchor = loc.start() as usize;
                let type_name = format_type(&type_arg.value);

                push_diag(
                    out,
                    settings,
                    &SHARED_CAPABILITY_OBJECT,
                    file,
                    span,
                    contents.as_ref(),
                    anchor,
                    format!(
                        "Sharing capability-like object `{type_name}` in `{func_name}` makes it publicly accessible. \
                         Ensure this cannot be used to bypass authorization, or suppress if intentional."
                    ),
                );
            }
        }

        match &exp.exp.value {
            T::UnannotatedExp_::ModuleCall(call) => {
                check_shared_capability_in_exp(
                    &call.arguments,
                    share_fns,
                    out,
                    settings,
                    file_map,
                    func_name,
                );
            }
            T::UnannotatedExp_::Block((_, seq_items)) => {
                for item in seq_items.iter() {
                    check_shared_capability_in_seq_item(
                        item,
                        share_fns,
                        out,
                        settings,
                        file_map,
                        func_name,
                    );
                }
            }
            T::UnannotatedExp_::IfElse(cond, if_body, else_body) => {
                check_shared_capability_in_exp(cond, share_fns, out, settings, file_map, func_name);
                check_shared_capability_in_exp(
                    if_body,
                    share_fns,
                    out,
                    settings,
                    file_map,
                    func_name,
                );
                if let Some(else_e) = else_body {
                    check_shared_capability_in_exp(
                        else_e,
                        share_fns,
                        out,
                        settings,
                        file_map,
                        func_name,
                    );
                }
            }
            T::UnannotatedExp_::While(_, cond, body) => {
                check_shared_capability_in_exp(cond, share_fns, out, settings, file_map, func_name);
                check_shared_capability_in_exp(
                    body,
                    share_fns,
                    out,
                    settings,
                    file_map,
                    func_name,
                );
            }
            T::UnannotatedExp_::Loop { body, .. } => {
                check_shared_capability_in_exp(
                    body,
                    share_fns,
                    out,
                    settings,
                    file_map,
                    func_name,
                );
            }
            _ => {}
        }
    }

    /// Detects capability-like transfers to literal addresses.
    fn lint_capability_transfer_literal_address(
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        prog: &T::Program,
    ) -> Result<()> {
        const TRANSFER_FUNCTIONS: &[(&str, &str)] =
            &[("transfer", "transfer"), ("transfer", "public_transfer")];

        for (mident, mdef) in prog.modules.key_cloned_iter() {
            match mdef.target_kind {
                TargetKind::Source {
                    is_root_package: true,
                } => {}
                _ => continue,
            }

            for (fname, fdef) in mdef.functions.key_cloned_iter() {
                let T::FunctionBody_::Defined((_use_funs, seq_items)) = &fdef.body.value else {
                    continue;
                };

                for item in seq_items.iter() {
                    check_capability_transfer_literal_in_seq_item(
                        item,
                        TRANSFER_FUNCTIONS,
                        out,
                        settings,
                        file_map,
                        fname.value().as_str(),
                    );
                }
            }
        }

        Ok(())
    }

    fn check_capability_transfer_literal_in_seq_item(
        item: &T::SequenceItem,
        transfer_fns: &[(&str, &str)],
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        func_name: &str,
    ) {
        match &item.value {
            T::SequenceItem_::Seq(exp) => {
                check_capability_transfer_literal_in_exp(
                    exp,
                    transfer_fns,
                    out,
                    settings,
                    file_map,
                    func_name,
                );
            }
            T::SequenceItem_::Bind(_, _, exp) => {
                check_capability_transfer_literal_in_exp(
                    exp,
                    transfer_fns,
                    out,
                    settings,
                    file_map,
                    func_name,
                );
            }
            _ => {}
        }
    }

    fn check_capability_transfer_literal_in_exp(
        exp: &T::Exp,
        transfer_fns: &[(&str, &str)],
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        func_name: &str,
    ) {
        use crate::type_classifier::is_capability_type_from_ty;

        if let T::UnannotatedExp_::ModuleCall(call) = &exp.exp.value {
            let module_sym = call.module.value.module.value();
            let module_name = module_sym.as_str();
            let call_sym = call.name.value();
            let call_name = call_sym.as_str();

            let is_transfer_call = transfer_fns
                .iter()
                .any(|(mod_pat, fn_pat)| module_name == *mod_pat && call_name == *fn_pat);

            if is_transfer_call
                && let Some(type_arg) = call.type_arguments.first()
                && !is_coin_type(&type_arg.value)
                && is_capability_type_from_ty(&type_arg.value)
                && let Some(recipient) = exp_list_nth_single(&call.arguments, 1)
                && looks_like_address_literal(recipient)
            {
                let loc = exp.exp.loc;
                let Some((file, span, contents)) = diag_from_loc(file_map, &loc) else {
                    return;
                };
                let anchor = loc.start() as usize;
                let type_name = format_type(&type_arg.value);

                push_diag(
                    out,
                    settings,
                    &CAPABILITY_TRANSFER_LITERAL_ADDRESS,
                    file,
                    span,
                    contents.as_ref(),
                    anchor,
                    format!(
                        "Capability-like object `{type_name}` transferred to a literal address in `{func_name}`. \
                         Prefer transferring to tx_context::sender(ctx) or otherwise prove recipient authorization."
                    ),
                );
            }
        }

        match &exp.exp.value {
            T::UnannotatedExp_::ModuleCall(call) => {
                check_capability_transfer_literal_in_exp(
                    &call.arguments,
                    transfer_fns,
                    out,
                    settings,
                    file_map,
                    func_name,
                );
            }
            T::UnannotatedExp_::Block((_, seq_items)) => {
                for item in seq_items.iter() {
                    check_capability_transfer_literal_in_seq_item(
                        item,
                        transfer_fns,
                        out,
                        settings,
                        file_map,
                        func_name,
                    );
                }
            }
            T::UnannotatedExp_::IfElse(cond, if_body, else_body) => {
                check_capability_transfer_literal_in_exp(
                    cond,
                    transfer_fns,
                    out,
                    settings,
                    file_map,
                    func_name,
                );
                check_capability_transfer_literal_in_exp(
                    if_body,
                    transfer_fns,
                    out,
                    settings,
                    file_map,
                    func_name,
                );
                if let Some(else_e) = else_body {
                    check_capability_transfer_literal_in_exp(
                        else_e,
                        transfer_fns,
                        out,
                        settings,
                        file_map,
                        func_name,
                    );
                }
            }
            T::UnannotatedExp_::While(_, cond, body) => {
                check_capability_transfer_literal_in_exp(
                    cond,
                    transfer_fns,
                    out,
                    settings,
                    file_map,
                    func_name,
                );
                check_capability_transfer_literal_in_exp(
                    body,
                    transfer_fns,
                    out,
                    settings,
                    file_map,
                    func_name,
                );
            }
            T::UnannotatedExp_::Loop { body, .. } => {
                check_capability_transfer_literal_in_exp(
                    body,
                    transfer_fns,
                    out,
                    settings,
                    file_map,
                    func_name,
                );
            }
            _ => {}
        }
    }

    fn is_public_entry_function(fdef: &T::Function) -> bool {
        fdef.entry.is_some()
            && matches!(
                fdef.visibility,
                move_compiler::expansion::ast::Visibility::Public(_)
            )
    }

    fn strip_refs(ty: &N::Type_) -> &N::Type_ {
        match ty {
            N::Type_::Ref(_, inner) => strip_refs(&inner.value),
            other => other,
        }
    }

    fn is_signer_type(ty: &N::Type_) -> bool {
        match strip_refs(ty) {
            N::Type_::Apply(_, tname, _) => matches!(
                &tname.value,
                N::TypeName_::Builtin(b) if matches!(b.value, N::BuiltinTypeName_::Signer)
            ),
            _ => false,
        }
    }

    fn is_vector_type(ty: &N::Type_) -> bool {
        match strip_refs(ty) {
            N::Type_::Apply(_, tname, _) => matches!(
                &tname.value,
                N::TypeName_::Builtin(b) if matches!(b.value, N::BuiltinTypeName_::Vector)
            ),
            _ => false,
        }
    }

    fn is_mut_ref_to_key_type(ty: &N::Type_) -> bool {
        let N::Type_::Ref(is_mut, inner) = ty else {
            return false;
        };
        if !*is_mut {
            return false;
        }
        type_classifier::abilities_of_type(&inner.value)
            .is_some_and(|a| type_classifier::has_key_ability(&a))
    }

    fn is_capability_like_type(ty: &N::Type_) -> bool {
        let inner = strip_refs(ty);
        !is_coin_type(inner) && type_classifier::is_capability_type_from_ty(inner)
    }

    /// Detects public entry functions that take `&mut` key objects without any explicit authority parameter.
    fn lint_mut_key_param_missing_authority(
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        prog: &T::Program,
    ) -> Result<()> {
        for (mident, mdef) in prog.modules.key_cloned_iter() {
            match mdef.target_kind {
                TargetKind::Source {
                    is_root_package: true,
                } => {}
                _ => continue,
            }

            for (fname, fdef) in mdef.functions.key_cloned_iter() {
                if !is_public_entry_function(fdef) {
                    continue;
                }

                let mut mut_key_param_ids: std::collections::BTreeSet<u16> =
                    std::collections::BTreeSet::new();
                let mut first_mut_key_type: Option<String> = None;

                for (_mut_, var, ty) in &fdef.signature.parameters {
                    if is_mut_ref_to_key_type(&ty.value) {
                        mut_key_param_ids.insert(var.value.id);
                        if first_mut_key_type.is_none() {
                            let inner = match &ty.value {
                                N::Type_::Ref(_, inner) => &inner.value,
                                other => other,
                            };
                            first_mut_key_type = Some(format_type(inner));
                        }
                    }
                }

                if mut_key_param_ids.is_empty() {
                    continue;
                }

                let has_explicit_authority = fdef.signature.parameters.iter().any(|(_m, v, t)| {
                    if mut_key_param_ids.contains(&v.value.id) {
                        return false;
                    }
                    is_signer_type(&t.value) || is_capability_like_type(&t.value)
                });

                if has_explicit_authority {
                    continue;
                }

                let loc = fdef.loc;
                let Some((file, span, contents)) = diag_from_loc(file_map, &loc) else {
                    continue;
                };
                let anchor = loc.start() as usize;
                let key_ty = first_mut_key_type.unwrap_or_else(|| "<key object>".to_string());
                let fn_name_sym = fname.value();
                let fn_name = fn_name_sym.as_str();

                push_diag(
                    out,
                    settings,
                    &MUT_KEY_PARAM_MISSING_AUTHORITY,
                    file,
                    span,
                    contents.as_ref(),
                    anchor,
                    format!(
                        "Public entry `{fn_name}` mutates `{key_ty}` by `&mut` but takes no explicit authority parameter. \
                         If this object can be passed as shared input, add an explicit capability check or sender validation."
                    ),
                );
            }
        }

        Ok(())
    }

    fn extract_local_var_id(exp: &T::Exp) -> Option<u16> {
        match &exp.exp.value {
            T::UnannotatedExp_::Use(v) => Some(v.value.id),
            T::UnannotatedExp_::Copy { var, .. } => Some(var.value.id),
            T::UnannotatedExp_::Move { var, .. } => Some(var.value.id),
            T::UnannotatedExp_::BorrowLocal(_, v) => Some(v.value.id),
            T::UnannotatedExp_::TempBorrow(_, inner) => extract_local_var_id(inner),
            T::UnannotatedExp_::Dereference(inner) => extract_local_var_id(inner),
            T::UnannotatedExp_::Cast(inner, _) => extract_local_var_id(inner),
            T::UnannotatedExp_::Annotate(inner, _) => extract_local_var_id(inner),
            T::UnannotatedExp_::Borrow(_, base, _) => extract_local_var_id(base),
            _ => None,
        }
    }

    fn vector_length_param_id(
        exp: &T::Exp,
        vector_param_ids: &std::collections::BTreeSet<u16>,
    ) -> Option<u16> {
        let T::UnannotatedExp_::ModuleCall(call) = &exp.exp.value else {
            return None;
        };
        let module_sym = call.module.value.module.value();
        let module_name = module_sym.as_str();
        let call_sym = call.name.value();
        let call_name = call_sym.as_str();

        if module_name != "vector" || call_name != "length" {
            return None;
        }

        let arg0 = exp_list_nth_single(&call.arguments, 0)?;
        let var_id = extract_local_var_id(arg0)?;
        if vector_param_ids.contains(&var_id) {
            Some(var_id)
        } else {
            None
        }
    }

    fn is_vector_length_bound(
        cond: &T::Exp,
        vector_param_ids: &std::collections::BTreeSet<u16>,
    ) -> bool {
        let T::UnannotatedExp_::BinopExp(left, op, _ty, right) = &cond.exp.value else {
            return false;
        };
        let op_str = format!("{:?}", op);
        let is_cmp = op_str.contains("Lt")
            || op_str.contains("Le")
            || op_str.contains("Gt")
            || op_str.contains("Ge");
        if !is_cmp {
            return false;
        }

        vector_length_param_id(right, vector_param_ids).is_some()
            || vector_length_param_id(left, vector_param_ids).is_some()
    }

    fn check_unbounded_iter_in_seq_item(
        item: &T::SequenceItem,
        vector_param_ids: &std::collections::BTreeSet<u16>,
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        func_name: &str,
    ) {
        match &item.value {
            T::SequenceItem_::Seq(exp) => {
                check_unbounded_iter_in_exp(
                    exp,
                    vector_param_ids,
                    out,
                    settings,
                    file_map,
                    func_name,
                );
            }
            T::SequenceItem_::Bind(_, _, exp) => {
                check_unbounded_iter_in_exp(
                    exp,
                    vector_param_ids,
                    out,
                    settings,
                    file_map,
                    func_name,
                );
            }
            _ => {}
        }
    }

    fn check_unbounded_iter_in_exp(
        exp: &T::Exp,
        vector_param_ids: &std::collections::BTreeSet<u16>,
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        func_name: &str,
    ) {
        if let T::UnannotatedExp_::While(_, cond, body) = &exp.exp.value {
            if is_vector_length_bound(cond, vector_param_ids) {
                let loc = exp.exp.loc;
                if let Some((file, span, contents)) = diag_from_loc(file_map, &loc) {
                    let anchor = loc.start() as usize;
                    push_diag(
                        out,
                        settings,
                        &UNBOUNDED_ITERATION_OVER_PARAM_VECTOR,
                        file,
                        span,
                        contents.as_ref(),
                        anchor,
                        format!(
                            "Loop in `{func_name}` is bounded by `vector::length` of an entry parameter. \
                             Add an explicit maximum length check to prevent resource-exhaustion DoS."
                        ),
                    );
                }
            }
            check_unbounded_iter_in_exp(cond, vector_param_ids, out, settings, file_map, func_name);
            check_unbounded_iter_in_exp(body, vector_param_ids, out, settings, file_map, func_name);
            return;
        }

        match &exp.exp.value {
            T::UnannotatedExp_::ModuleCall(call) => {
                check_unbounded_iter_in_exp(
                    &call.arguments,
                    vector_param_ids,
                    out,
                    settings,
                    file_map,
                    func_name,
                );
            }
            T::UnannotatedExp_::Block((_, seq_items)) => {
                for item in seq_items.iter() {
                    check_unbounded_iter_in_seq_item(
                        item,
                        vector_param_ids,
                        out,
                        settings,
                        file_map,
                        func_name,
                    );
                }
            }
            T::UnannotatedExp_::IfElse(cond, if_body, else_body) => {
                check_unbounded_iter_in_exp(
                    cond,
                    vector_param_ids,
                    out,
                    settings,
                    file_map,
                    func_name,
                );
                check_unbounded_iter_in_exp(
                    if_body,
                    vector_param_ids,
                    out,
                    settings,
                    file_map,
                    func_name,
                );
                if let Some(else_e) = else_body {
                    check_unbounded_iter_in_exp(
                        else_e,
                        vector_param_ids,
                        out,
                        settings,
                        file_map,
                        func_name,
                    );
                }
            }
            T::UnannotatedExp_::Loop { body, .. } => {
                check_unbounded_iter_in_exp(
                    body,
                    vector_param_ids,
                    out,
                    settings,
                    file_map,
                    func_name,
                );
            }
            _ => {}
        }
    }

    /// Detects unbounded loops over a vector parameter in public entry functions.
    fn lint_unbounded_iteration_over_param_vector(
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        prog: &T::Program,
    ) -> Result<()> {
        for (mident, mdef) in prog.modules.key_cloned_iter() {
            match mdef.target_kind {
                TargetKind::Source {
                    is_root_package: true,
                } => {}
                _ => continue,
            }

            for (fname, fdef) in mdef.functions.key_cloned_iter() {
                if !is_public_entry_function(fdef) {
                    continue;
                }

                let vector_param_ids: std::collections::BTreeSet<u16> = fdef
                    .signature
                    .parameters
                    .iter()
                    .filter_map(|(_m, v, t)| is_vector_type(&t.value).then_some(v.value.id))
                    .collect();
                if vector_param_ids.is_empty() {
                    continue;
                }

                let T::FunctionBody_::Defined((_use_funs, seq_items)) = &fdef.body.value else {
                    continue;
                };
                let fn_name_sym = fname.value();
                let fn_name = fn_name_sym.as_str();
                for item in seq_items.iter() {
                    check_unbounded_iter_in_seq_item(
                        item,
                        &vector_param_ids,
                        out,
                        settings,
                        file_map,
                        fn_name,
                    );
                }
            }
        }

        Ok(())
    }

    fn is_type_name_witness_type(ty: &N::Type_) -> bool {
        match strip_refs(ty) {
            N::Type_::Apply(_, type_name, _) => {
                if let N::TypeName_::ModuleType(mident, struct_name) = &type_name.value {
                    let module_sym = mident.value.module.value();
                    let struct_sym = struct_name.value();
                    // Match sui::coin::Coin or any coin module's Coin type
                    module_sym.as_str() == "type_name" && struct_sym.as_str() == "TypeName"
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn exp_uses_var(exp: &T::Exp, target: u16) -> bool {
        match &exp.exp.value {
            T::UnannotatedExp_::Use(v) => v.value.id == target,
            T::UnannotatedExp_::Copy { var, .. } => var.value.id == target,
            T::UnannotatedExp_::Move { var, .. } => var.value.id == target,
            T::UnannotatedExp_::BorrowLocal(_, v) => v.value.id == target,
            T::UnannotatedExp_::TempBorrow(_, inner) => exp_uses_var(inner, target),
            T::UnannotatedExp_::Dereference(inner) => exp_uses_var(inner, target),
            T::UnannotatedExp_::Borrow(_, base, _) => exp_uses_var(base, target),
            T::UnannotatedExp_::UnaryExp(_, inner) => exp_uses_var(inner, target),
            T::UnannotatedExp_::Cast(inner, _) => exp_uses_var(inner, target),
            T::UnannotatedExp_::Annotate(inner, _) => exp_uses_var(inner, target),
            T::UnannotatedExp_::Return(inner) => exp_uses_var(inner, target),
            T::UnannotatedExp_::Abort(inner) => exp_uses_var(inner, target),
            T::UnannotatedExp_::Give(_, inner) => exp_uses_var(inner, target),
            T::UnannotatedExp_::BinopExp(left, _op, _ty, right) => {
                exp_uses_var(left, target) || exp_uses_var(right, target)
            }
            T::UnannotatedExp_::Mutate(left, right) => {
                exp_uses_var(left, target) || exp_uses_var(right, target)
            }
            T::UnannotatedExp_::Assign(_lvalues, _expected_types, rhs) => exp_uses_var(rhs, target),
            T::UnannotatedExp_::ModuleCall(call) => exp_uses_var(&call.arguments, target),
            T::UnannotatedExp_::Builtin(_, args) => exp_uses_var(args, target),
            T::UnannotatedExp_::Vector(_loc, _n, _ty, args) => exp_uses_var(args, target),
            T::UnannotatedExp_::ExpList(items) => items.iter().any(|item| match item {
                T::ExpListItem::Single(e, _) => exp_uses_var(e, target),
                T::ExpListItem::Splat(_, e, _) => exp_uses_var(e, target),
            }),
            T::UnannotatedExp_::IfElse(cond, if_body, else_body) => {
                exp_uses_var(cond, target)
                    || exp_uses_var(if_body, target)
                    || else_body
                        .as_deref()
                        .is_some_and(|e| exp_uses_var(e, target))
            }
            T::UnannotatedExp_::While(_, cond, body) => {
                exp_uses_var(cond, target) || exp_uses_var(body, target)
            }
            T::UnannotatedExp_::Loop { body, .. } => exp_uses_var(body, target),
            T::UnannotatedExp_::Block((_, seq_items))
            | T::UnannotatedExp_::NamedBlock(_, (_, seq_items)) => {
                seq_items.iter().any(|item| match &item.value {
                    T::SequenceItem_::Seq(e) => exp_uses_var(e, target),
                    T::SequenceItem_::Bind(_, _, e) => exp_uses_var(e, target),
                    _ => false,
                })
            }
            T::UnannotatedExp_::Match(scrut, arms) => {
                exp_uses_var(scrut, target)
                    || arms.value.iter().any(|arm| {
                        arm.value
                            .guard
                            .as_deref()
                            .is_some_and(|g| exp_uses_var(g, target))
                            || exp_uses_var(&arm.value.rhs, target)
                    })
            }
            T::UnannotatedExp_::VariantMatch(scrut, _t, arms) => {
                exp_uses_var(scrut, target)
                    || arms.iter().any(|(_vname, e)| exp_uses_var(e, target))
            }
            T::UnannotatedExp_::Pack(_, _, _tys, fields) => fields
                .iter()
                .any(|(_f, _idx, (_, (_, e)))| exp_uses_var(e, target)),
            T::UnannotatedExp_::PackVariant(_, _, _, _tys, fields) => fields
                .iter()
                .any(|(_f, _idx, (_, (_, e)))| exp_uses_var(e, target)),
            _ => false,
        }
    }

    /// Detects generic functions that accept a `type_name::TypeName` witness but never use it.
    fn lint_generic_type_witness_unused(
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        prog: &T::Program,
    ) -> Result<()> {
        for (mident, mdef) in prog.modules.key_cloned_iter() {
            match mdef.target_kind {
                TargetKind::Source {
                    is_root_package: true,
                } => {}
                _ => continue,
            }

            for (fname, fdef) in mdef.functions.key_cloned_iter() {
                if fdef.signature.type_parameters.is_empty() {
                    continue;
                }

                let witness_params: Vec<(u16, String)> = fdef
                    .signature
                    .parameters
                    .iter()
                    .filter(|(_m, _v, t)| is_type_name_witness_type(&t.value))
                    .map(|(_m, v, t)| (v.value.id, format_type(strip_refs(&t.value))))
                    .collect();

                if witness_params.is_empty() {
                    continue;
                }

                let T::FunctionBody_::Defined((_use_funs, seq_items)) = &fdef.body.value else {
                    continue;
                };

                for (witness_id, witness_ty) in witness_params {
                    let used = seq_items.iter().any(|item| match &item.value {
                        T::SequenceItem_::Seq(e) => exp_uses_var(e, witness_id),
                        T::SequenceItem_::Bind(_, _, e) => exp_uses_var(e, witness_id),
                        _ => false,
                    });

                    if used {
                        continue;
                    }

                    let loc = fdef.loc;
                    let Some((file, span, contents)) = diag_from_loc(file_map, &loc) else {
                        continue;
                    };
                    let anchor = loc.start() as usize;
                    let fn_name_sym = fname.value();
                    let fn_name = fn_name_sym.as_str();

                    push_diag(
                        out,
                        settings,
                        &GENERIC_TYPE_WITNESS_UNUSED,
                        file,
                        span,
                        contents.as_ref(),
                        anchor,
                        format!(
                            "Generic function `{fn_name}` takes a `{witness_ty}` witness but never uses it. \
                             Either remove the witness parameter or use it to validate the generic type argument."
                        ),
                    );
                }
            }
        }

        Ok(())
    }

    // =========================================================================
    // Capability Transfer V2 Lint (type-based)
    // =========================================================================

    /// Detect capability transfers to non-sender addresses.
    ///
    /// Flags transfer::transfer(cap, addr) where:
    /// - cap has capability abilities (key + store, no copy, no drop)
    /// - addr is not tx_context::sender(ctx)
    fn lint_capability_transfer_v2(
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        prog: &T::Program,
    ) -> Result<()> {
        const TRANSFER_FUNCTIONS: &[(&str, &str)] =
            &[("transfer", "transfer"), ("transfer", "public_transfer")];

        for (mident, mdef) in prog.modules.key_cloned_iter() {
            match mdef.target_kind {
                TargetKind::Source {
                    is_root_package: true,
                } => {}
                _ => continue,
            }

            for (fname, fdef) in mdef.functions.key_cloned_iter() {
                let T::FunctionBody_::Defined((_use_funs, seq_items)) = &fdef.body.value else {
                    continue;
                };

                for item in seq_items.iter() {
                    check_capability_transfer_in_seq_item(
                        item,
                        TRANSFER_FUNCTIONS,
                        out,
                        settings,
                        file_map,
                        fname.value().as_str(),
                    );
                }
            }
        }

        Ok(())
    }

    fn check_capability_transfer_in_seq_item(
        item: &T::SequenceItem,
        transfer_fns: &[(&str, &str)],
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        func_name: &str,
    ) {
        match &item.value {
            T::SequenceItem_::Seq(exp) => {
                check_capability_transfer_in_exp(
                    exp,
                    transfer_fns,
                    out,
                    settings,
                    file_map,
                    func_name,
                );
            }
            T::SequenceItem_::Bind(_, _, exp) => {
                check_capability_transfer_in_exp(
                    exp,
                    transfer_fns,
                    out,
                    settings,
                    file_map,
                    func_name,
                );
            }
            _ => {}
        }
    }

    fn check_capability_transfer_in_exp(
        exp: &T::Exp,
        transfer_fns: &[(&str, &str)],
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        func_name: &str,
    ) {
        use crate::type_classifier::is_capability_type_from_ty;

        if let T::UnannotatedExp_::ModuleCall(call) = &exp.exp.value {
            let module_sym = call.module.value.module.value();
            let module_name = module_sym.as_str();
            let call_sym = call.name.value();
            let call_name = call_sym.as_str();

            let is_transfer_call = transfer_fns
                .iter()
                .any(|(mod_pat, fn_pat)| module_name == *mod_pat && call_name == *fn_pat);

            if is_transfer_call
                && let Some(type_arg) = call.type_arguments.first()
                && !is_coin_type(&type_arg.value)
                && is_capability_type_from_ty(&type_arg.value)
            {
                let loc = exp.exp.loc;
                let Some((file, span, contents)) = diag_from_loc(file_map, &loc) else {
                    return;
                };
                let anchor = loc.start() as usize;
                let type_name = format_type(&type_arg.value);

                push_diag(
                    out,
                    settings,
                    &CAPABILITY_TRANSFER_V2,
                    file,
                    span,
                    contents.as_ref(),
                    anchor,
                    format!(
                        "Capability-like object `{type_name}` transferred in `{func_name}`. \
                         Ensure the recipient is authorized (e.g., tx_context::sender(ctx))."
                    ),
                );
            }
        }

        // Recurse into subexpressions
        match &exp.exp.value {
            T::UnannotatedExp_::ModuleCall(call) => {
                check_capability_transfer_in_exp(
                    &call.arguments,
                    transfer_fns,
                    out,
                    settings,
                    file_map,
                    func_name,
                );
            }
            T::UnannotatedExp_::Block((_, seq_items)) => {
                for item in seq_items.iter() {
                    check_capability_transfer_in_seq_item(
                        item,
                        transfer_fns,
                        out,
                        settings,
                        file_map,
                        func_name,
                    );
                }
            }
            T::UnannotatedExp_::IfElse(cond, if_body, else_body) => {
                check_capability_transfer_in_exp(
                    cond,
                    transfer_fns,
                    out,
                    settings,
                    file_map,
                    func_name,
                );
                check_capability_transfer_in_exp(
                    if_body,
                    transfer_fns,
                    out,
                    settings,
                    file_map,
                    func_name,
                );
                if let Some(else_e) = else_body {
                    check_capability_transfer_in_exp(
                        else_e,
                        transfer_fns,
                        out,
                        settings,
                        file_map,
                        func_name,
                    );
                }
            }
            T::UnannotatedExp_::While(_, cond, body) => {
                check_capability_transfer_in_exp(
                    cond,
                    transfer_fns,
                    out,
                    settings,
                    file_map,
                    func_name,
                );
                check_capability_transfer_in_exp(
                    body,
                    transfer_fns,
                    out,
                    settings,
                    file_map,
                    func_name,
                );
            }
            T::UnannotatedExp_::Loop { body, .. } => {
                check_capability_transfer_in_exp(
                    body,
                    transfer_fns,
                    out,
                    settings,
                    file_map,
                    func_name,
                );
            }
            _ => {}
        }
    }

    // =========================================================================
    // Security Semantic Lints (type-based)
    //
    // NOTE: phantom_capability is implemented in absint_lints.rs (CFG-aware)
    // =========================================================================

    fn lint_entry_function_returns_value(
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        prog: &T::Program,
    ) -> Result<()> {
        for (mident, mdef) in prog.modules.key_cloned_iter() {
            match mdef.target_kind {
                TargetKind::Source {
                    is_root_package: true,
                } => {}
                _ => continue,
            }

            for (fname, fdef) in mdef.functions.key_cloned_iter() {
                if fdef.entry.is_none() {
                    continue;
                }

                if matches!(fdef.signature.return_type.value, N::Type_::Unit) {
                    continue;
                }

                let loc = fdef.loc;
                let Some((file, span, contents)) = diag_from_loc(file_map, &loc) else {
                    continue;
                };
                let anchor = loc.start() as usize;

                let fn_name_sym = fname.value();
                let fn_name = fn_name_sym.as_str();
                let ret_ty = format_type(&fdef.signature.return_type.value);

                push_diag(
                    out,
                    settings,
                    &ENTRY_FUNCTION_RETURNS_VALUE,
                    file,
                    span,
                    contents.as_ref(),
                    anchor,
                    format!(
                        "Entry function `{fn_name}` returns `{ret_ty}`, but entry function return values are discarded by the runtime. \
                         Return unit `()` and write values into objects instead."
                    ),
                );
            }
        }

        Ok(())
    }

    fn lint_private_entry_function(
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        prog: &T::Program,
    ) -> Result<()> {
        for (mident, mdef) in prog.modules.key_cloned_iter() {
            match mdef.target_kind {
                TargetKind::Source {
                    is_root_package: true,
                } => {}
                _ => continue,
            }

            for (fname, fdef) in mdef.functions.key_cloned_iter() {
                if fdef.entry.is_none() {
                    continue;
                }

                if !matches!(
                    fdef.visibility,
                    move_compiler::expansion::ast::Visibility::Internal
                ) {
                    continue;
                }

                let loc = fdef.loc;
                let Some((file, span, contents)) = diag_from_loc(file_map, &loc) else {
                    continue;
                };
                let anchor = loc.start() as usize;

                let fn_name_sym = fname.value();
                let fn_name = fn_name_sym.as_str();

                push_diag(
                    out,
                    settings,
                    &PRIVATE_ENTRY_FUNCTION,
                    file,
                    span,
                    contents.as_ref(),
                    anchor,
                    format!(
                        "Private entry function `{fn_name}` is unreachable from transactions. \
                         Remove the `entry` modifier or make it `public entry` / `public(package) entry`."
                    ),
                );
            }
        }

        Ok(())
    }

    /// Lint for division operations without zero-divisor checks.
    ///
    /// Division by zero will abort the transaction. This lint detects divisions
    /// where the divisor hasn't been validated as non-zero.
    fn lint_unchecked_division(
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        prog: &T::Program,
    ) -> Result<()> {
        for (mident, mdef) in prog.modules.key_cloned_iter() {
            match mdef.target_kind {
                TargetKind::Source {
                    is_root_package: true,
                } => {}
                _ => continue,
            }

            for (fname, fdef) in mdef.functions.key_cloned_iter() {
                let T::FunctionBody_::Defined((_use_funs, seq_items)) = &fdef.body.value else {
                    continue;
                };

                // Track variables that have been validated as non-zero
                let mut validated_vars: std::collections::HashSet<u16> =
                    std::collections::HashSet::new();

                for item in seq_items.iter() {
                    check_division_in_seq_item(
                        item,
                        &mut validated_vars,
                        out,
                        settings,
                        file_map,
                        fname.value().as_str(),
                    );
                }
            }
        }

        Ok(())
    }

    /// Check for division operations in a sequence item.
    fn check_division_in_seq_item(
        item: &T::SequenceItem,
        validated_vars: &mut std::collections::HashSet<u16>,
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        func_name: &str,
    ) {
        match &item.value {
            T::SequenceItem_::Seq(exp) => {
                // Check for assert statements that validate non-zero
                check_for_nonzero_assertion(exp, validated_vars);
                check_division_in_exp(exp, validated_vars, out, settings, file_map, func_name);
            }
            T::SequenceItem_::Bind(_, _, exp) => {
                check_division_in_exp(exp, validated_vars, out, settings, file_map, func_name);
            }
            _ => {}
        }
    }

    /// Check if an expression is an assertion that validates a variable is non-zero.
    fn check_for_nonzero_assertion(
        exp: &T::Exp,
        validated_vars: &mut std::collections::HashSet<u16>,
    ) {
        // Look for assert!(var != 0, ...) or assert!(var > 0, ...)
        if let T::UnannotatedExp_::Builtin(builtin, args) = &exp.exp.value {
            let builtin_str = format!("{:?}", builtin);
            if builtin_str.contains("Assert") {
                // args is Box<Exp> - extract first argument from ExpList if present
                let first_arg = if let T::UnannotatedExp_::ExpList(items) = &args.exp.value {
                    items.first().and_then(|item| match item {
                        T::ExpListItem::Single(e, _) => Some(e),
                        _ => None,
                    })
                } else {
                    Some(args.as_ref())
                };

                if let Some(first_arg) = first_arg
                    && let T::UnannotatedExp_::BinopExp(left, op, _, right) = &first_arg.exp.value
                {
                    let op_str = format!("{:?}", op);
                    // Check for != 0 or > 0
                    if op_str.contains("Neq") || op_str.contains("Gt") {
                        // Check if comparing with 0
                        if is_zero_value(right)
                            && let Some(var_id) = extract_var_id(left)
                        {
                            validated_vars.insert(var_id);
                        }
                        if is_zero_value(left)
                            && let Some(var_id) = extract_var_id(right)
                        {
                            validated_vars.insert(var_id);
                        }
                    }
                }
            }
        }
    }

    /// Check if an expression is a zero value.
    fn is_zero_value(exp: &T::Exp) -> bool {
        if let T::UnannotatedExp_::Value(val) = &exp.exp.value {
            let val_str = format!("{:?}", val);
            val_str.contains("0") && !val_str.contains("0x")
        } else {
            false
        }
    }

    /// Extract variable ID from an expression if it's a simple variable reference.
    fn extract_var_id(exp: &T::Exp) -> Option<u16> {
        match &exp.exp.value {
            T::UnannotatedExp_::Use(v) => Some(v.value.id),
            T::UnannotatedExp_::Copy { var, .. } => Some(var.value.id),
            T::UnannotatedExp_::Move { var, .. } => Some(var.value.id),
            _ => None,
        }
    }

    /// Check for division operations in an expression.
    fn check_division_in_exp(
        exp: &T::Exp,
        validated_vars: &std::collections::HashSet<u16>,
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        func_name: &str,
    ) {
        match &exp.exp.value {
            T::UnannotatedExp_::BinopExp(left, op, _, right) => {
                let op_str = format!("{:?}", op);
                if op_str.contains("Div") || op_str.contains("Mod") {
                    // Check if the divisor (right) is a validated variable
                    let divisor_validated = if let Some(var_id) = extract_var_id(right) {
                        validated_vars.contains(&var_id)
                    } else {
                        // If it's a constant or complex expression, assume it might be safe
                        // (conservative approach to reduce FPs)
                        matches!(
                            &right.exp.value,
                            T::UnannotatedExp_::Value(_) | T::UnannotatedExp_::Constant(_, _)
                        )
                    };

                    if !divisor_validated {
                        let loc = exp.exp.loc;
                        let Some((file, span, contents)) = diag_from_loc(file_map, &loc) else {
                            return;
                        };
                        let anchor = loc.start() as usize;

                        push_diag(
                            out,
                            settings,
                            &UNUSED_RETURN_VALUE,
                            file,
                            span,
                            contents.as_ref(),
                            anchor,
                            format!(
                                "Division in function `{func_name}` may divide by zero. \
                                 Consider adding `assert!(divisor != 0, E_DIVISION_BY_ZERO)` before this operation."
                            ),
                        );
                    }
                }

                // Recurse
                check_division_in_exp(left, validated_vars, out, settings, file_map, func_name);
                check_division_in_exp(right, validated_vars, out, settings, file_map, func_name);
            }
            T::UnannotatedExp_::ModuleCall(call) => {
                check_division_in_exp(
                    &call.arguments,
                    validated_vars,
                    out,
                    settings,
                    file_map,
                    func_name,
                );
            }
            T::UnannotatedExp_::Block((_, seq)) => {
                let mut local_validated = validated_vars.clone();
                for item in seq.iter() {
                    check_division_in_seq_item(
                        item,
                        &mut local_validated,
                        out,
                        settings,
                        file_map,
                        func_name,
                    );
                }
            }
            T::UnannotatedExp_::IfElse(cond, t, e_opt) => {
                check_division_in_exp(cond, validated_vars, out, settings, file_map, func_name);
                check_division_in_exp(t, validated_vars, out, settings, file_map, func_name);
                if let Some(else_e) = e_opt {
                    check_division_in_exp(
                        else_e,
                        validated_vars,
                        out,
                        settings,
                        file_map,
                        func_name,
                    );
                }
            }
            _ => {}
        }
    }

    // =========================================================================
    // Unused Return Value Lint
    // =========================================================================

    /// Lint for important return values that are ignored.
    ///
    /// This lint detects when function calls that return non-unit values
    /// have their return values discarded.
    fn lint_unused_return_value(
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        prog: &T::Program,
    ) -> Result<()> {
        // TODO(infra): Move to crate::framework_catalog and match on fully-qualified IDs.
        // Functions whose return values should not be ignored
        const IMPORTANT_FUNCTIONS: &[(&str, &str)] = &[
            ("coin", "split"),
            ("coin", "take"),
            ("balance", "split"),
            ("balance", "withdraw_all"),
            ("option", "extract"),
            ("option", "destroy_some"),
            ("vector", "pop_back"),
            ("table", "remove"),
            ("bag", "remove"),
        ];

        for (mident, mdef) in prog.modules.key_cloned_iter() {
            match mdef.target_kind {
                TargetKind::Source {
                    is_root_package: true,
                } => {}
                _ => continue,
            }

            for (fname, fdef) in mdef.functions.key_cloned_iter() {
                let T::FunctionBody_::Defined((_use_funs, seq_items)) = &fdef.body.value else {
                    continue;
                };

                for item in seq_items.iter() {
                    check_unused_return_in_seq_item(
                        item,
                        IMPORTANT_FUNCTIONS,
                        out,
                        settings,
                        file_map,
                        fname.value().as_str(),
                    );
                }
            }
        }

        Ok(())
    }

    /// Check for unused return values in a sequence item.
    fn check_unused_return_in_seq_item(
        item: &T::SequenceItem,
        important_fns: &[(&str, &str)],
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        func_name: &str,
    ) {
        match &item.value {
            T::SequenceItem_::Seq(exp) => {
                // If a Seq item is a function call, its return value is discarded
                if let T::UnannotatedExp_::ModuleCall(call) = &exp.exp.value {
                    let module_sym = call.module.value.module.value();
                    let module_name = module_sym.as_str();
                    let call_sym = call.name.value();
                    let call_name = call_sym.as_str();

                    for (mod_pattern, fn_pattern) in important_fns {
                        if module_name == *mod_pattern && call_name == *fn_pattern {
                            let loc = exp.exp.loc;
                            let Some((file, span, contents)) = diag_from_loc(file_map, &loc) else {
                                continue;
                            };
                            let anchor = loc.start() as usize;

                            push_diag(
                                out,
                                settings,
                                &UNUSED_RETURN_VALUE,
                                file,
                                span,
                                contents.as_ref(),
                                anchor,
                                format!(
                                    "Return value of `{module_name}::{call_name}` in function `{func_name}` is ignored. \
                                     This may indicate a bug - the returned value (often a Coin or extracted value) should be used."
                                ),
                            );
                        }
                    }
                }
            }
            T::SequenceItem_::Bind(_, _, exp) => {
                // Bound expressions are using their return value, so recurse into nested calls
                check_unused_return_in_exp(exp, important_fns, out, settings, file_map, func_name);
            }
            _ => {}
        }
    }

    /// Recursively check for unused return values in expressions.
    fn check_unused_return_in_exp(
        exp: &T::Exp,
        important_fns: &[(&str, &str)],
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        func_name: &str,
    ) {
        match &exp.exp.value {
            T::UnannotatedExp_::Block((_, seq)) => {
                for item in seq.iter() {
                    check_unused_return_in_seq_item(
                        item,
                        important_fns,
                        out,
                        settings,
                        file_map,
                        func_name,
                    );
                }
            }
            T::UnannotatedExp_::IfElse(cond, t, e_opt) => {
                check_unused_return_in_exp(cond, important_fns, out, settings, file_map, func_name);
                check_unused_return_in_exp(t, important_fns, out, settings, file_map, func_name);
                if let Some(e) = e_opt {
                    check_unused_return_in_exp(
                        e,
                        important_fns,
                        out,
                        settings,
                        file_map,
                        func_name,
                    );
                }
            }
            T::UnannotatedExp_::While(_, cond, body) => {
                check_unused_return_in_exp(cond, important_fns, out, settings, file_map, func_name);
                check_unused_return_in_exp(body, important_fns, out, settings, file_map, func_name);
            }
            T::UnannotatedExp_::Loop { body, .. } => {
                check_unused_return_in_exp(
                    body,
                    important_fns,
                    out,
                    settings,
                    file_map,
                    func_name,
                );
            }
            T::UnannotatedExp_::BinopExp(l, _op, _ty, r) => {
                check_unused_return_in_exp(l, important_fns, out, settings, file_map, func_name);
                check_unused_return_in_exp(r, important_fns, out, settings, file_map, func_name);
            }
            T::UnannotatedExp_::UnaryExp(_, inner) => {
                check_unused_return_in_exp(
                    inner,
                    important_fns,
                    out,
                    settings,
                    file_map,
                    func_name,
                );
            }
            T::UnannotatedExp_::Borrow(_, inner, _) => {
                check_unused_return_in_exp(
                    inner,
                    important_fns,
                    out,
                    settings,
                    file_map,
                    func_name,
                );
            }
            T::UnannotatedExp_::TempBorrow(_, inner) => {
                check_unused_return_in_exp(
                    inner,
                    important_fns,
                    out,
                    settings,
                    file_map,
                    func_name,
                );
            }
            T::UnannotatedExp_::Dereference(inner) => {
                check_unused_return_in_exp(
                    inner,
                    important_fns,
                    out,
                    settings,
                    file_map,
                    func_name,
                );
            }
            T::UnannotatedExp_::Vector(_, _, _, args) => {
                check_unused_return_in_exp(args, important_fns, out, settings, file_map, func_name);
            }
            T::UnannotatedExp_::Builtin(_, args) => {
                check_unused_return_in_exp(args, important_fns, out, settings, file_map, func_name);
            }
            T::UnannotatedExp_::ExpList(items) => {
                for item in items.iter() {
                    match item {
                        T::ExpListItem::Single(e, _) => {
                            check_unused_return_in_exp(
                                e,
                                important_fns,
                                out,
                                settings,
                                file_map,
                                func_name,
                            );
                        }
                        T::ExpListItem::Splat(_, e, _) => {
                            check_unused_return_in_exp(
                                e,
                                important_fns,
                                out,
                                settings,
                                file_map,
                                func_name,
                            );
                        }
                    }
                }
            }
            T::UnannotatedExp_::Pack(_, _, _, fields) => {
                for (_, _, (_, (_, e))) in fields.iter() {
                    check_unused_return_in_exp(
                        e,
                        important_fns,
                        out,
                        settings,
                        file_map,
                        func_name,
                    );
                }
            }
            T::UnannotatedExp_::PackVariant(_, _, _, _, fields) => {
                for (_, _, (_, (_, e))) in fields.iter() {
                    check_unused_return_in_exp(
                        e,
                        important_fns,
                        out,
                        settings,
                        file_map,
                        func_name,
                    );
                }
            }
            _ => {}
        }
    }

    // =========================================================================
    // Share Owned Authority Lint (type-grounded)
    // =========================================================================

    /// Lint for sharing objects with `key + store` abilities.
    ///
    /// Objects with `key + store` represent "transferable authority" and sharing
    /// them makes them publicly accessible. This is type-grounded, not name-based.
    fn lint_share_owned_authority(
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        prog: &T::Program,
    ) -> Result<()> {
        // TODO(infra): Move to crate::framework_catalog and match on fully-qualified IDs.
        // Share functions to detect
        const SHARE_FUNCTIONS: &[(&str, &str)] = &[
            ("transfer", "share_object"),
            ("transfer", "public_share_object"),
        ];

        for (mident, mdef) in prog.modules.key_cloned_iter() {
            match mdef.target_kind {
                TargetKind::Source {
                    is_root_package: true,
                } => {}
                _ => continue,
            }

            for (fname, fdef) in mdef.functions.key_cloned_iter() {
                let T::FunctionBody_::Defined((_use_funs, seq_items)) = &fdef.body.value else {
                    continue;
                };

                for item in seq_items.iter() {
                    check_share_owned_in_seq_item(
                        item,
                        SHARE_FUNCTIONS,
                        out,
                        settings,
                        file_map,
                        fname.value().as_str(),
                    );
                }
            }
        }

        Ok(())
    }

    /// Check for share_object calls on key+store types in a sequence item.
    fn check_share_owned_in_seq_item(
        item: &T::SequenceItem,
        share_fns: &[(&str, &str)],
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        func_name: &str,
    ) {
        match &item.value {
            T::SequenceItem_::Seq(exp) => {
                check_share_owned_in_exp(exp, share_fns, out, settings, file_map, func_name);
            }
            T::SequenceItem_::Bind(_, _, exp) => {
                check_share_owned_in_exp(exp, share_fns, out, settings, file_map, func_name);
            }
            _ => {}
        }
    }

    /// Check for share_object calls on key+store types in an expression.
    fn check_share_owned_in_exp(
        exp: &T::Exp,
        share_fns: &[(&str, &str)],
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        func_name: &str,
    ) {
        if let T::UnannotatedExp_::ModuleCall(call) = &exp.exp.value {
            let module_sym = call.module.value.module.value();
            let module_name = module_sym.as_str();
            let call_sym = call.name.value();
            let call_name = call_sym.as_str();

            let is_share_call = share_fns
                .iter()
                .any(|(mod_pat, fn_pat)| module_name == *mod_pat && call_name == *fn_pat);

            if is_share_call
                && let Some(type_arg) = call.type_arguments.first()
                && type_classifier::is_key_store_type(&type_arg.value)
            {
                let loc = exp.exp.loc;
                let Some((file, span, contents)) = diag_from_loc(file_map, &loc) else {
                    return;
                };
                let anchor = loc.start() as usize;

                // Get the type name for the message
                let type_name = format_type(&type_arg.value);

                push_diag(
                    out,
                    settings,
                    &SHARE_OWNED_AUTHORITY,
                    file,
                    span,
                    contents.as_ref(),
                    anchor,
                    format!(
                        "Sharing `{type_name}` (has key+store) in `{func_name}` makes it publicly accessible. \
                         This is dangerous for authority objects (capabilities). \
                         If this is intentional shared state, suppress with #[ext(move_clippy(allow(share_owned_authority)))]."
                    ),
                );
            }
        }

        // Recurse into subexpressions
        match &exp.exp.value {
            T::UnannotatedExp_::ModuleCall(call) => {
                // arguments is Box<Exp>, not iterable - recurse into it directly
                check_share_owned_in_exp(
                    &call.arguments,
                    share_fns,
                    out,
                    settings,
                    file_map,
                    func_name,
                );
            }
            T::UnannotatedExp_::Block((_, seq_items)) => {
                for item in seq_items.iter() {
                    check_share_owned_in_seq_item(
                        item,
                        share_fns,
                        out,
                        settings,
                        file_map,
                        func_name,
                    );
                }
            }
            T::UnannotatedExp_::IfElse(cond, if_body, else_body) => {
                check_share_owned_in_exp(cond, share_fns, out, settings, file_map, func_name);
                check_share_owned_in_exp(if_body, share_fns, out, settings, file_map, func_name);
                if let Some(else_e) = else_body {
                    check_share_owned_in_exp(else_e, share_fns, out, settings, file_map, func_name);
                }
            }
            T::UnannotatedExp_::While(_, cond, body) => {
                check_share_owned_in_exp(cond, share_fns, out, settings, file_map, func_name);
                check_share_owned_in_exp(body, share_fns, out, settings, file_map, func_name);
            }
            T::UnannotatedExp_::Loop { body, .. } => {
                check_share_owned_in_exp(body, share_fns, out, settings, file_map, func_name);
            }
            _ => {}
        }
    }

    /// Extract abilities from a Type (using naming::ast::Type_ structure).
    /// The typing AST re-exports Type_ from naming::ast.
    #[allow(dead_code)]
    fn get_type_abilities(ty: &N::Type_) -> Option<move_compiler::expansion::ast::AbilitySet> {
        match ty {
            N::Type_::Apply(abilities, _, _) => abilities.clone(),
            N::Type_::Ref(_, inner) => get_type_abilities(&inner.value),
            N::Type_::Param(tp) => Some(tp.abilities.clone()),
            _ => None,
        }
    }

    /// Check if a type is `sui::coin::Coin<T>`.
    ///
    /// Coin types have the same ability pattern as capabilities (key+store, no copy/drop)
    /// but they are value tokens, not access control objects. We exclude them from
    /// capability transfer warnings to avoid false positives.
    fn is_coin_type(ty: &N::Type_) -> bool {
        match ty {
            N::Type_::Apply(_, type_name, _) => {
                if let N::TypeName_::ModuleType(mident, struct_name) = &type_name.value {
                    let module_sym = mident.value.module.value();
                    let struct_sym = struct_name.value();
                    // Match sui::coin::Coin or any coin module's Coin type
                    module_sym.as_str() == "coin" && struct_sym.as_str() == "Coin"
                } else {
                    false
                }
            }
            N::Type_::Ref(_, inner) => is_coin_type(&inner.value),
            _ => false,
        }
    }

    /// Format a type for display in error messages (using naming::ast::Type_ structure).
    fn format_type(ty: &N::Type_) -> String {
        match ty {
            N::Type_::Unit => "()".to_string(),
            N::Type_::Ref(is_mut, inner) => {
                let prefix = if *is_mut { "&mut " } else { "&" };
                format!("{}{}", prefix, format_type(&inner.value))
            }
            N::Type_::Apply(_, type_name, type_args) => format_apply_type(type_name, type_args),
            N::Type_::Param(tp) => tp.user_specified_name.value.to_string(),
            N::Type_::Fun(args, ret) => {
                let arg_strs: Vec<_> = args.iter().map(|t| format_type(&t.value)).collect();
                format!(
                    "fun({}) -> {}",
                    arg_strs.join(", "),
                    format_type(&ret.value)
                )
            }
            N::Type_::Var(_) => "_".to_string(),
            N::Type_::Anything => "any".to_string(),
            N::Type_::Void => "void".to_string(),
            N::Type_::UnresolvedError => "error".to_string(),
        }
    }

    /// Format an Apply type (module::Type<args>) for display.
    fn format_apply_type(type_name: &N::TypeName, type_args: &[N::Type]) -> String {
        let name = match &type_name.value {
            N::TypeName_::Builtin(builtin) => format!("{:?}", builtin.value),
            N::TypeName_::ModuleType(mident, struct_name) => {
                format!("{}::{}", mident.value.module.value(), struct_name.value())
            }
            N::TypeName_::Multiple(_) => "tuple".to_string(),
        };
        if type_args.is_empty() {
            name
        } else {
            let args: Vec<_> = type_args.iter().map(|t| format_type(strip_refs(&t.value))).collect();
            format!("{}<{}>", name, args.join(", "))
        }
    }

    // =========================================================================
    // Event Emit Type Sanity Lint
    // =========================================================================

    fn lint_event_emit_type_sanity(
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        prog: &T::Program,
    ) -> Result<()> {
        // TODO(infra): Move to crate::framework_catalog and match on fully-qualified IDs.
        // Event emit functions to detect
        const EVENT_EMIT_FUNCTIONS: &[(&str, &str)] = &[("event", "emit")];

        for (_mident, mdef) in prog.modules.key_cloned_iter() {
            match mdef.target_kind {
                TargetKind::Source {
                    is_root_package: true,
                } => {}
                _ => continue,
            }

            for (fname, fdef) in mdef.functions.key_cloned_iter() {
                let T::FunctionBody_::Defined((_use_funs, seq_items)) = &fdef.body.value else {
                    continue;
                };

                for item in seq_items.iter() {
                    check_event_emit_in_seq_item(
                        item,
                        EVENT_EMIT_FUNCTIONS,
                        out,
                        settings,
                        file_map,
                        fname.value().as_str(),
                    );
                }
            }
        }

        Ok(())
    }

    fn check_event_emit_in_seq_item(
        item: &T::SequenceItem,
        emit_fns: &[(&str, &str)],
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        func_name: &str,
    ) {
        match &item.value {
            T::SequenceItem_::Seq(exp) => {
                check_event_emit_in_exp(exp, emit_fns, out, settings, file_map, func_name);
            }
            T::SequenceItem_::Bind(_, _, exp) => {
                check_event_emit_in_exp(exp, emit_fns, out, settings, file_map, func_name);
            }
            _ => {}
        }
    }

    fn check_event_emit_in_exp(
        exp: &T::Exp,
        emit_fns: &[(&str, &str)],
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        func_name: &str,
    ) {
        if let T::UnannotatedExp_::ModuleCall(call) = &exp.exp.value {
            let module_sym = call.module.value.module.value();
            let module_name = module_sym.as_str();
            let call_sym = call.name.value();
            let call_name = call_sym.as_str();

            let is_emit_call = emit_fns
                .iter()
                .any(|(mod_pat, fn_pat)| module_name == *mod_pat && call_name == *fn_pat);

            if is_emit_call && let Some(type_arg) = call.type_arguments.first() {
                let abilities = type_classifier::abilities_of_type(&type_arg.value);
                let is_event_like = type_classifier::is_event_like_type(&type_arg.value);

                if abilities.is_some() && !is_event_like {
                    let loc = exp.exp.loc;
                    let Some((file, span, contents)) = diag_from_loc(file_map, &loc) else {
                        return;
                    };
                    let anchor = loc.start() as usize;
                    let type_name = format_type(&type_arg.value);

                    push_diag(
                        out,
                        settings,
                        &EVENT_EMIT_TYPE_SANITY,
                        file,
                        span,
                        contents.as_ref(),
                        anchor,
                        format!(
                            "Emitting `{type_name}` via `event::emit` in `{func_name}`; event types should be `copy + drop` and must not have `key`."
                        ),
                    );
                }
            }
        }

        match &exp.exp.value {
            T::UnannotatedExp_::ModuleCall(call) => {
                check_event_emit_in_exp(
                    &call.arguments,
                    emit_fns,
                    out,
                    settings,
                    file_map,
                    func_name,
                );
            }
            T::UnannotatedExp_::Block((_, seq_items)) => {
                for item in seq_items.iter() {
                    check_event_emit_in_seq_item(
                        item, emit_fns, out, settings, file_map, func_name,
                    );
                }
            }
            T::UnannotatedExp_::IfElse(cond, if_body, else_body) => {
                check_event_emit_in_exp(cond, emit_fns, out, settings, file_map, func_name);
                check_event_emit_in_exp(if_body, emit_fns, out, settings, file_map, func_name);
                if let Some(else_e) = else_body {
                    check_event_emit_in_exp(else_e, emit_fns, out, settings, file_map, func_name);
                }
            }
            T::UnannotatedExp_::While(_, cond, body) => {
                check_event_emit_in_exp(cond, emit_fns, out, settings, file_map, func_name);
                check_event_emit_in_exp(body, emit_fns, out, settings, file_map, func_name);
            }
            T::UnannotatedExp_::Loop { body, .. } => {
                check_event_emit_in_exp(body, emit_fns, out, settings, file_map, func_name);
            }
            _ => {}
        }
    }

    // =========================================================================
    // Public Random Access V2 Lint (type-based)
    // =========================================================================

    /// Lint for public (non-entry) functions that expose sui::random::Random objects.
    ///
    /// Random objects should only be accessible in entry functions to prevent
    /// front-running attacks where validators can see random values before
    /// including transactions.
    fn lint_public_random_access_v2(
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        prog: &T::Program,
    ) -> Result<()> {
        for (mident, mdef) in prog.modules.key_cloned_iter() {
            match mdef.target_kind {
                TargetKind::Source {
                    is_root_package: true,
                } => {}
                _ => continue,
            }

            for (fname, fdef) in mdef.functions.key_cloned_iter() {
                // Only check public non-entry functions
                // Entry functions are allowed to take Random
                if fdef.entry.is_some() {
                    continue;
                }

                // Check if function is public
                let is_public = matches!(
                    fdef.visibility,
                    move_compiler::expansion::ast::Visibility::Public(_)
                );

                if !is_public {
                    continue;
                }

                // Check if any parameter is sui::random::Random
                for (_, _, param_ty) in fdef.signature.parameters.iter() {
                    if is_random_type(&param_ty.value) {
                        let loc = fdef.loc;
                        let Some((file, span, contents)) = diag_from_loc(file_map, &loc) else {
                            continue;
                        };
                        let anchor = loc.start() as usize;

                        let fn_name_sym = fname.value();
                        let fn_name = fn_name_sym.as_str();

                        push_diag(
                            out,
                            settings,
                            &PUBLIC_RANDOM_ACCESS_V2,
                            file,
                            span,
                            contents.as_ref(),
                            anchor,
                            format!(
                                "Public function `{fn_name}` exposes `sui::random::Random` object. \
                                 This enables front-running attacks where validators can see random \
                                 values before including transactions. Use `entry` visibility instead, \
                                 or make the function private/package-internal."
                            ),
                        );
                        break; // Only report once per function
                    }
                }
            }
        }

        Ok(())
    }

    /// Check if a type is sui::random::Random (including references).
    fn is_random_type(ty: &N::Type_) -> bool {
        match ty {
            N::Type_::Apply(_, type_name, _) => {
                if let N::TypeName_::ModuleType(mident, struct_name) = &type_name.value {
                    let addr = &mident.value.address;
                    let module_sym = mident.value.module.value();
                    let struct_sym = struct_name.value();

                    // Check for 0x2::random::Random
                    // The address should be the Sui framework address (0x2)
                    let is_sui_addr = match addr {
                        move_compiler::expansion::ast::Address::Numerical {
                            value: addr_value, ..
                        } => {
                            // Check if address bytes end with 0x02
                            let bytes = addr_value.value.into_bytes();
                            bytes.iter().take(31).all(|&b| b == 0) && bytes[31] == 2
                        }
                        move_compiler::expansion::ast::Address::NamedUnassigned(name) => {
                            name.value.as_str() == "sui" || name.value.as_str() == "0x2"
                        }
                    };

                    is_sui_addr
                        && module_sym.as_str() == "random"
                        && struct_sym.as_str() == "Random"
                } else {
                    false
                }
            }
            N::Type_::Ref(_, inner) => is_random_type(&inner.value),
            _ => false,
        }
    }

    // =========================================================================
    // Sui-delegated Lints
    // =========================================================================

    fn lint_sui_visitors(
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        build_plan: &BuildPlan,
        package_root: &Path,
    ) -> ClippyResult<()> {
        use move_compiler::diagnostics::report_diagnostics_to_buffer_with_env_color;
        use move_compiler::linters::{LintLevel as CompilerLintLevel, LinterDiagnosticCategory};
        use move_compiler::sui_mode::linters;

        let mut writer = Vec::new();
        let deps = build_plan.compute_dependencies();
        let collected = std::cell::RefCell::new(Vec::new());

        build_plan.compile_with_driver_and_deps(deps, &mut writer, |compiler| {
            let (attr, filters) = linters::known_filters();
            let compiler = compiler
                .add_custom_known_filters(attr, filters)
                .add_visitors(linters::linter_visitors(CompilerLintLevel::All));
            let (files, res) = compiler.build()?;
            match res {
                Ok((units, warnings)) => {
                    collected.borrow_mut().push((files.clone(), warnings));
                    Ok((files, units))
                }
                Err(errors) => {
                    let rendered = report_diagnostics_to_buffer_with_env_color(&files, errors);
                    Err(MoveClippyError::semantic(format!(
                        "Move compilation failed while running Sui lints:\n{}",
                        String::from_utf8_lossy(&rendered)
                    ))
                    .into_anyhow())
                }
            }
        })?;

        let mut seen: std::collections::BTreeSet<(
            &'static str,
            String,
            usize,
            usize,
            usize,
            usize,
            String,
        )> = std::collections::BTreeSet::new();

        for (file_map, warnings) in collected.into_inner() {
            for diag in warnings.into_vec() {
                if diag.info().category() != LinterDiagnosticCategory::Sui as u8 {
                    continue;
                }
                let Some(descriptor) = descriptor_for_sui_code(diag.info().code()) else {
                    continue;
                };
                let level = settings.level_for(descriptor.name);
                if level == LintLevel::Allow {
                    continue;
                }

                let Some((file, span, contents)) = diag_from_loc(&file_map, &diag.primary_loc())
                else {
                    continue;
                };

                if !Path::new(&file).starts_with(package_root) {
                    continue;
                }

                let anchor = diag.primary_loc().start() as usize;
                if suppression::is_suppressed_at(contents.as_ref(), anchor, descriptor.name) {
                    continue;
                }

                let message = compose_sui_message(&diag);
                let key = (
                    descriptor.name,
                    file.clone(),
                    span.start.row,
                    span.start.column,
                    span.end.row,
                    span.end.column,
                    message.clone(),
                );
                if !seen.insert(key) {
                    continue;
                }
                out.push(Diagnostic {
                    lint: descriptor,
                    level,
                    file: Some(file),
                    span,
                    message,
                    help: None,
                    suggestion: None,
                });
            }
        }

        Ok(())
    }

    fn descriptor_for_sui_code(code: u8) -> Option<&'static LintDescriptor> {
        use move_compiler::sui_mode::linters::LinterDiagnosticCode::*;

        match code {
            x if x == ShareOwned as u8 => Some(&SHARE_OWNED),
            x if x == SelfTransfer as u8 => Some(&SELF_TRANSFER),
            x if x == CustomStateChange as u8 => Some(&CUSTOM_STATE_CHANGE),
            x if x == CoinField as u8 => Some(&COIN_FIELD),
            x if x == FreezeWrapped as u8 => Some(&FREEZE_WRAPPED),
            x if x == CollectionEquality as u8 => Some(&COLLECTION_EQUALITY),
            x if x == PublicRandom as u8 => Some(&PUBLIC_RANDOM),
            x if x == MissingKey as u8 => Some(&MISSING_KEY),
            x if x == FreezingCapability as u8 => Some(&FREEZING_CAPABILITY),
            x if x == PreferMutableTxContext as u8 => Some(&PUBLIC_MUT_TX_CONTEXT),
            x if x == UnnecessaryPublicEntry as u8 => Some(&UNNECESSARY_PUBLIC_ENTRY),
            _ => None,
        }
    }

    fn compose_sui_message(diag: &move_compiler::diagnostics::Diagnostic) -> String {
        let base = diag.info().message().to_string();
        let label = diag.primary_msg().trim();
        if label.is_empty() || base.contains(label) {
            base
        } else {
            format!("{base}: {label}")
        }
    }

    #[allow(dead_code)]
    fn is_ref_to_module_type(
        ty: &N::Type,
        module: &move_compiler::expansion::ast::ModuleIdent,
    ) -> bool {
        let N::Type_::Ref(_is_mut, inner) = &ty.value else {
            return false;
        };

        let N::Type_::Apply(_abilities, tname, _tys) = &inner.value else {
            return false;
        };

        match &tname.value {
            N::TypeName_::ModuleType(m, _dtype) => m == module,
            _ => false,
        }
    }

    #[allow(dead_code)]
    fn is_simple_self_field_get(exp: &T::Exp, self_var: &N::Var) -> bool {
        match &exp.exp.value {
            T::UnannotatedExp_::Borrow(_mut_, base, _field) => is_self_local(base, self_var),
            T::UnannotatedExp_::Dereference(inner) => match &inner.exp.value {
                T::UnannotatedExp_::Borrow(_mut_, base, _field) => is_self_local(base, self_var),
                _ => false,
            },
            _ => false,
        }
    }

    #[allow(dead_code)]
    fn is_self_local(base: &T::Exp, self_var: &N::Var) -> bool {
        match &base.exp.value {
            T::UnannotatedExp_::BorrowLocal(_mut_, v) => v.value.id == self_var.value.id,
            T::UnannotatedExp_::TempBorrow(_, inner) => is_self_local(inner, self_var),
            T::UnannotatedExp_::Copy { var, .. } => var.value.id == self_var.value.id,
            T::UnannotatedExp_::Move { var, .. } => var.value.id == self_var.value.id,
            T::UnannotatedExp_::Use(v) => v.value.id == self_var.value.id,
            _ => false,
        }
    }
}

#[cfg(feature = "full")]
pub use full::lint_package;

#[cfg(not(feature = "full"))]
pub fn lint_package(
    _package_path: &Path,
    _settings: &LintSettings,
    _preview: bool,
    _experimental: bool,
) -> ClippyResult<Vec<Diagnostic>> {
    Err(MoveClippyError::semantic(
        "full mode requires building with --features full",
    ))
}
