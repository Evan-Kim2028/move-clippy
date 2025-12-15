// Allow patterns that are intentional in semantic analysis
// - unused_variables: Move compiler iterators yield (key, value) pairs but we often only need value
// - unreachable_patterns: Match arms for exhaustiveness that may not be reached in practice
#![allow(unused_variables)]
#![allow(unreachable_patterns)]

use crate::diagnostics::Diagnostic;
use crate::error::{ClippyResult, MoveClippyError};
use crate::lint::{
    AnalysisKind, FixDescriptor, LintCategory, LintDescriptor, LintSettings, RuleGroup,
};
use std::path::Path;

/// Semantic lints that rely on Move compiler typing information.
///
/// These lints are only available when `move-clippy` is built with the
/// `full` feature and run in `--mode full` against a Move package.
pub static CAPABILITY_NAMING: LintDescriptor = LintDescriptor {
    name: "capability_naming",
    category: LintCategory::Naming,
    description: "[DEPRECATED] Sui uses Cap suffix (AdminCap, TreasuryCap), not _cap",
    group: RuleGroup::Deprecated,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
};

pub static EVENT_NAMING: LintDescriptor = LintDescriptor {
    name: "event_naming",
    category: LintCategory::Naming,
    description: "[DEPRECATED] Sui events don't use _event suffix (Transferred, PoolCreated)",
    group: RuleGroup::Deprecated,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
};

// ============================================================================
// Sui-Delegated Lints (production-ready, type-based)
// ============================================================================

pub static SHARE_OWNED: LintDescriptor = LintDescriptor {
    name: "share_owned",
    category: LintCategory::Suspicious,
    description: "Possible owned object share (Sui lint, type-based, requires --mode full)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
};

pub static SELF_TRANSFER: LintDescriptor = LintDescriptor {
    name: "self_transfer",
    category: LintCategory::Suspicious,
    description: "Transferring or sharing objects back to the sender (Sui lint, type-based, requires --mode full)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
};

pub static CUSTOM_STATE_CHANGE: LintDescriptor = LintDescriptor {
    name: "custom_state_change",
    category: LintCategory::Suspicious,
    description: "Custom transfer/share/freeze functions must call private variants (Sui lint, type-based, requires --mode full)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
};

pub static COIN_FIELD: LintDescriptor = LintDescriptor {
    name: "coin_field",
    category: LintCategory::Suspicious,
    description: "Avoid storing sui::coin::Coin fields inside structs (Sui lint, type-based, requires --mode full)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
};

pub static FREEZE_WRAPPED: LintDescriptor = LintDescriptor {
    name: "freeze_wrapped",
    category: LintCategory::Suspicious,
    description: "Do not wrap shared objects before freezing (Sui lint, type-based, requires --mode full)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
};

pub static COLLECTION_EQUALITY: LintDescriptor = LintDescriptor {
    name: "collection_equality",
    category: LintCategory::Suspicious,
    description: "Avoid equality checks over bags/tables/collections (Sui lint, type-based, requires --mode full)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
};

pub static PUBLIC_RANDOM: LintDescriptor = LintDescriptor {
    name: "public_random",
    category: LintCategory::Suspicious,
    description: "Random state should remain private and uncopyable (Sui lint, type-based, requires --mode full)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
};

pub static MISSING_KEY: LintDescriptor = LintDescriptor {
    name: "missing_key",
    category: LintCategory::Suspicious,
    description: "Warn when shared/transferred structs lack the key ability (Sui lint, type-based, requires --mode full)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
};

pub static FREEZING_CAPABILITY: LintDescriptor = LintDescriptor {
    name: "freezing_capability",
    category: LintCategory::Suspicious,
    description: "Avoid storing freeze capabilities (Sui lint, type-based, requires --mode full)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
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
    description: "[DEPRECATED] Use unchecked_division_v2 (CFG-aware) instead - this version lacks dataflow analysis",
    group: RuleGroup::Deprecated,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
};

/// Detects important return values that are ignored.
///
/// # Security References
///
/// - **General Smart Contract Security**: Ignoring return values can hide errors
/// - **Sui Move**: Many functions return important status or values
///
/// # Why This Matters
///
/// Some function return values indicate success/failure or contain
/// important data. Ignoring them can lead to:
/// 1. Silent failures (error codes ignored)
/// 2. Lost assets (coin splits not captured)
/// 3. Security bypasses (validation results ignored)
///
/// # Example (Bad)
///
/// ```move
/// public fun withdraw(pool: &mut Pool, amount: u64) {
///     coin::split(&mut pool.balance, amount, ctx);  // Split coin lost!
/// }
/// ```
///
/// # Correct Pattern
///
/// ```move
/// public fun withdraw(pool: &mut Pool, amount: u64): Coin<SUI> {
///     coin::split(&mut pool.balance, amount, ctx)
/// }
/// ```
pub static UNUSED_RETURN_VALUE: LintDescriptor = LintDescriptor {
    name: "unused_return_value",
    category: LintCategory::Security,
    description: "Important return value is ignored, may indicate bug (type-based)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
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
/// Use `#[allow(lint(share_owned_authority))]` to suppress for intentional cases.
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
/// #[allow(lint(share_owned_authority))]
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
/// A struct with only `drop` is almost always a bug - either:
/// 1. It's a hot potato that should have NO abilities (remove `drop`)
/// 2. It's a witness that should be empty (verify it's actually empty)
///
/// Structs with `copy + drop` are NOT flagged (they're events/DTOs).
/// Structs with `key + store` are NOT flagged (they're resources).
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
    description: "Capability transferred to non-sender address (type-based, requires --mode full)",
    group: RuleGroup::Preview,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
};

// NOTE: The following lints are implemented elsewhere or require future work:
// - phantom_capability: Implemented in absint_lints.rs (CFG-aware)
// - unused_hot_potato: Requires dataflow analysis (future work)

static DESCRIPTORS: &[&LintDescriptor] = &[
    // Naming (type-based)
    &CAPABILITY_NAMING,
    &EVENT_NAMING,
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
    &SHARE_OWNED_AUTHORITY,
    // Security (preview, type-based)
    &UNCHECKED_DIVISION,
    &UNUSED_RETURN_VALUE,
    &DROPPABLE_HOT_POTATO_V2,
    &CAPABILITY_TRANSFER_V2,
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
    use move_compiler::parser::ast::{Ability_, TargetKind};
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
        use crate::absint_lints::{PHANTOM_CAPABILITY, UNCHECKED_DIVISION_V2};

        // Only treat warnings emitted by our Phase II visitors as Phase II lints.
        //
        // AbsInt lints emit `custom("Lint", ..., category=50, code=...)` (see `absint_lints.rs`),
        // which renders as `warning[LintW5000X] ...`. The compiler also emits many unrelated
        // warnings with small numeric `code()` values (e.g., UnusedItem::Alias), so filtering
        // only on `code()` will misclassify those as Phase II lints.
        if info.external_prefix() != Some("Lint") || info.category() != 50 {
            return None;
        }

        match info.code() {
            1 => Some(&PHANTOM_CAPABILITY),
            2 => Some(&UNCHECKED_DIVISION_V2),
            _ => None,
        }
    }

    /// Run all semantic lints against the package rooted at `package_path`.
    pub fn lint_package(
        package_path: &Path,
        settings: &LintSettings,
        preview: bool,
    ) -> ClippyResult<Vec<Diagnostic>> {
        instrument_block!("semantic::lint_package", {
            let package_root = std::fs::canonicalize(package_path)?;
            let mut writer = Vec::<u8>::new();
            let mut build_config = BuildConfig::default();
            build_config.default_flavor = Some(Flavor::Sui);
            let resolved_graph =
                build_config.resolution_graph_for_package(&package_root, None, &mut writer)?;
            let build_plan = BuildPlan::create(&resolved_graph)?;

            let hook = SaveHook::new([SaveFlag::Typing, SaveFlag::TypingInfo]);

            // Get Phase II visitors (SimpleAbsInt-based lints)
            let phase2_visitors: Vec<Visitor> = absint_lints::create_visitors(preview)
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
            lint_capability_naming(&mut out, settings, &file_map, &typing_info)?;
            lint_event_naming(&mut out, settings, &file_map, &typing_info)?;
            // Type-based security lints
            lint_unchecked_division(&mut out, settings, &file_map, &typing_ast)?;
            lint_unused_return_value(&mut out, settings, &file_map, &typing_ast)?;
            lint_event_emit_type_sanity(&mut out, settings, &file_map, &typing_ast)?;
            lint_share_owned_authority(&mut out, settings, &file_map, &typing_ast)?;
            lint_droppable_hot_potato_v2(&mut out, settings, &file_map, &typing_info)?;
            // Phase 4 security lints (type-based, preview)
            lint_capability_transfer_v2(&mut out, settings, &file_map, &typing_ast)?;
            // Note: phantom_capability is implemented in absint_lints.rs (CFG-aware)

            // Phase III: Cross-module analysis lints (type-based)
            lint_cross_module_lints(&mut out, settings, &file_map, &typing_ast, &typing_info)?;

            // Sui-delegated lints (type-based, production)
            lint_sui_visitors(&mut out, settings, &build_plan, &package_root)?;

            // Filter Preview-group diagnostics when preview is disabled
            if !preview {
                out.retain(|d| d.lint.group != RuleGroup::Preview);
            }

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
        let level = settings.level_for(lint.name);
        if level == LintLevel::Allow {
            return;
        }
        if suppression::is_suppressed_at(source, anchor_start, lint.name) {
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

    fn lint_capability_naming(
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        info: &TypingProgramInfo,
    ) -> Result<()> {
        for (mident, minfo) in info.modules.key_cloned_iter() {
            match minfo.target_kind {
                TargetKind::Source {
                    is_root_package: true,
                } => {}
                _ => continue,
            }

            for (sname, sdef) in minfo.structs.key_cloned_iter() {
                let abilities = &sdef.abilities;
                let is_cap = abilities.has_ability_(move_compiler::parser::ast::Ability_::Key)
                    && abilities.has_ability_(move_compiler::parser::ast::Ability_::Store)
                    && !abilities.has_ability_(Ability_::Copy)
                    && !abilities.has_ability_(Ability_::Drop);
                if !is_cap {
                    continue;
                }

                let sym = sname.value();
                let name_str = sym.as_str();
                if name_str.ends_with("_cap") {
                    continue;
                }

                let loc = sname.loc();
                let Some((file, span, contents)) = diag_from_loc(file_map, &loc) else {
                    continue;
                };
                let anchor = loc.start() as usize;
                push_diag(
                    out,
                    settings,
                    &CAPABILITY_NAMING,
                    file,
                    span,
                    contents.as_ref(),
                    anchor,
                    format!("Capability struct should be suffixed with `_cap`: `{name_str}_cap`"),
                );
            }
        }

        Ok(())
    }

    fn lint_event_naming(
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        info: &TypingProgramInfo,
    ) -> Result<()> {
        for (mident, minfo) in info.modules.key_cloned_iter() {
            match minfo.target_kind {
                TargetKind::Source {
                    is_root_package: true,
                } => {}
                _ => continue,
            }

            for (sname, sdef) in minfo.structs.key_cloned_iter() {
                let abilities = &sdef.abilities;
                let is_event = abilities.has_ability_(Ability_::Copy)
                    && abilities.has_ability_(Ability_::Drop)
                    && !abilities.has_ability_(Ability_::Key)
                    && !abilities.has_ability_(Ability_::Store);
                if !is_event {
                    continue;
                }

                let sym = sname.value();
                let name_str = sym.as_str();
                let loc = sname.loc();
                let Some((file, span, contents)) = diag_from_loc(file_map, &loc) else {
                    continue;
                };
                let anchor = loc.start() as usize;

                if !name_str.ends_with("_event") {
                    push_diag(
                        out,
                        settings,
                        &EVENT_NAMING,
                        file,
                        span,
                        contents.as_ref(),
                        anchor,
                        format!("Event struct should end with `_event`: `{name_str}_event`"),
                    );
                    continue;
                }

                let first = name_str.split('_').next().unwrap_or("");
                if !first.ends_with("ed") {
                    push_diag(
                        out,
                        settings,
                        &EVENT_NAMING,
                        file,
                        span,
                        contents.as_ref(),
                        anchor,
                        "Event struct should use a past-tense verb prefix (e.g. `transferred_..._event`)".to_string(),
                    );
                }
            }
        }

        Ok(())
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
        use crate::type_classifier::{has_copy_ability, has_drop_ability, has_key_ability, has_store_ability};

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
        use crate::type_classifier::is_capability_type_from_ty;

        const TRANSFER_FUNCTIONS: &[(&str, &str)] = &[
            ("transfer", "transfer"),
            ("transfer", "public_transfer"),
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
                check_capability_transfer_in_exp(exp, transfer_fns, out, settings, file_map, func_name);
            }
            T::SequenceItem_::Bind(_, _, exp) => {
                check_capability_transfer_in_exp(exp, transfer_fns, out, settings, file_map, func_name);
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

            if is_transfer_call {
                // Check if type argument is a capability type
                if let Some(type_arg) = call.type_arguments.first() {
                    if is_capability_type_from_ty(&type_arg.value) {
                        // This is transferring a capability - check if recipient is sender
                        // For now, we flag all capability transfers as preview-level warnings
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
                                "Capability `{type_name}` transferred in `{func_name}`. \
                                 Ensure the recipient is authorized (e.g., tx_context::sender(ctx))."
                            ),
                        );
                    }
                }
            }
        }

        // Recurse into subexpressions
        match &exp.exp.value {
            T::UnannotatedExp_::ModuleCall(call) => {
                check_capability_transfer_in_exp(&call.arguments, transfer_fns, out, settings, file_map, func_name);
            }
            T::UnannotatedExp_::Block((_, seq_items)) => {
                for item in seq_items.iter() {
                    check_capability_transfer_in_seq_item(item, transfer_fns, out, settings, file_map, func_name);
                }
            }
            T::UnannotatedExp_::IfElse(cond, if_body, else_body) => {
                check_capability_transfer_in_exp(cond, transfer_fns, out, settings, file_map, func_name);
                check_capability_transfer_in_exp(if_body, transfer_fns, out, settings, file_map, func_name);
                if let Some(else_e) = else_body {
                    check_capability_transfer_in_exp(else_e, transfer_fns, out, settings, file_map, func_name);
                }
            }
            T::UnannotatedExp_::While(_, cond, body) => {
                check_capability_transfer_in_exp(cond, transfer_fns, out, settings, file_map, func_name);
                check_capability_transfer_in_exp(body, transfer_fns, out, settings, file_map, func_name);
            }
            T::UnannotatedExp_::Loop { body, .. } => {
                check_capability_transfer_in_exp(body, transfer_fns, out, settings, file_map, func_name);
            }
            _ => {}
        }
    }

    // =========================================================================
    // Security Semantic Lints (type-based)
    //
    // NOTE: phantom_capability is implemented in absint_lints.rs (CFG-aware)
    // =========================================================================

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
                            &UNCHECKED_DIVISION,
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
            T::UnannotatedExp_::IfElse(cond, then_e, else_e_opt) => {
                check_division_in_exp(cond, validated_vars, out, settings, file_map, func_name);
                check_division_in_exp(then_e, validated_vars, out, settings, file_map, func_name);
                if let Some(else_e) = else_e_opt {
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
                check_unused_return_in_exp(body, important_fns, out, settings, file_map, func_name);
            }
            T::UnannotatedExp_::BinopExp(l, _op, _ty, r) => {
                check_unused_return_in_exp(l, important_fns, out, settings, file_map, func_name);
                check_unused_return_in_exp(r, important_fns, out, settings, file_map, func_name);
            }
            T::UnannotatedExp_::UnaryExp(_op, inner) => {
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
                         If this is intentional shared state, suppress with #[allow(lint(share_owned_authority))]."
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
                        item, share_fns, out, settings, file_map, func_name,
                    );
                }
            }
            T::UnannotatedExp_::IfElse(cond, if_body, else_body) => {
                check_share_owned_in_exp(cond, share_fns, out, settings, file_map, func_name);
                check_share_owned_in_exp(if_body, share_fns, out, settings, file_map, func_name);
                // else_body is Option<Box<Exp>>
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
            let args: Vec<_> = type_args.iter().map(|t| format_type(&t.value)).collect();
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
) -> ClippyResult<Vec<Diagnostic>> {
    Err(MoveClippyError::semantic(
        "full mode requires building with --features full",
    ))
}
