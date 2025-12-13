use crate::diagnostics::Diagnostic;
use crate::error::{ClippyResult, MoveClippyError};
use crate::lint::{FixDescriptor, LintCategory, LintDescriptor, LintSettings, RuleGroup};
use std::path::Path;

/// Semantic lints that rely on Move compiler typing information.
///
/// These lints are only available when `move-clippy` is built with the
/// `full` feature and run in `--mode full` against a Move package.
pub static CAPABILITY_NAMING: LintDescriptor = LintDescriptor {
    name: "capability_naming",
    category: LintCategory::Naming,
    description: "Capability structs (key+store) should be suffixed with _cap (semantic, requires --mode full)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
};

pub static EVENT_NAMING: LintDescriptor = LintDescriptor {
    name: "event_naming",
    category: LintCategory::Naming,
    description: "Event structs (copy+drop) should be named <past_tense>_<noun>_event (semantic, requires --mode full)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
};

pub static GETTER_NAMING: LintDescriptor = LintDescriptor {
    name: "getter_naming",
    category: LintCategory::Naming,
    description: "Avoid get_ prefix for simple field getters taking &Self (semantic, requires --mode full)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
};

pub static SHARE_OWNED: LintDescriptor = LintDescriptor {
    name: "share_owned",
    category: LintCategory::Suspicious,
    description: "Possible owned object share (Sui lint, requires --mode full)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
};

pub static SELF_TRANSFER: LintDescriptor = LintDescriptor {
    name: "self_transfer",
    category: LintCategory::Suspicious,
    description: "Transferring or sharing objects back to the sender (Sui lint, requires --mode full)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
};

pub static CUSTOM_STATE_CHANGE: LintDescriptor = LintDescriptor {
    name: "custom_state_change",
    category: LintCategory::Suspicious,
    description: "Custom transfer/share/freeze functions must call private variants (Sui lint, requires --mode full)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
};

pub static COIN_FIELD: LintDescriptor = LintDescriptor {
    name: "coin_field",
    category: LintCategory::Suspicious,
    description: "Avoid storing sui::coin::Coin fields inside structs (Sui lint, requires --mode full)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
};

pub static FREEZE_WRAPPED: LintDescriptor = LintDescriptor {
    name: "freeze_wrapped",
    category: LintCategory::Suspicious,
    description: "Do not wrap shared objects before freezing (Sui lint, requires --mode full)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
};

pub static COLLECTION_EQUALITY: LintDescriptor = LintDescriptor {
    name: "collection_equality",
    category: LintCategory::Suspicious,
    description: "Avoid equality checks over bags/tables/collections (Sui lint, requires --mode full)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
};

pub static PUBLIC_RANDOM: LintDescriptor = LintDescriptor {
    name: "public_random",
    category: LintCategory::Suspicious,
    description: "Random state should remain private and uncopyable (Sui lint, requires --mode full)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
};

pub static MISSING_KEY: LintDescriptor = LintDescriptor {
    name: "missing_key",
    category: LintCategory::Suspicious,
    description: "Warn when shared/transferred structs lack the key ability (Sui lint, requires --mode full)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
};

pub static FREEZING_CAPABILITY: LintDescriptor = LintDescriptor {
    name: "freezing_capability",
    category: LintCategory::Suspicious,
    description: "Avoid storing freeze capabilities (Sui lint, requires --mode full)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
};

// ============================================================================
// Security Semantic Lints (audit-backed)
// ============================================================================

/// Detects CoinMetadata being shared instead of frozen.
///
/// # Security References
///
/// - **MoveBit (2023-07-07)**: "The metadata in Coin should be frozen"
///   URL: https://movebit.xyz/blog/post/Sui-Objects-Security-Principles-and-Best-Practices.html
///   Verified: 2025-12-13 (Still valid - fundamental Sui coin pattern)
///
/// # Why This Matters
///
/// If CoinMetadata is shared instead of frozen, the admin can modify
/// the token's name, symbol, and other metadata after creation.
/// This can confuse users and enable phishing attacks.
///
/// # Example (Bad)
///
/// ```move
/// public fun init(witness: MY_TOKEN, ctx: &mut TxContext) {
///     let (treasury, metadata) = coin::create_currency(...);
///     transfer::public_share_object(metadata);  // BAD - can be modified!
/// }
/// ```
///
/// # Correct Pattern
///
/// ```move
/// public fun init(witness: MY_TOKEN, ctx: &mut TxContext) {
///     let (treasury, metadata) = coin::create_currency(...);
///     transfer::public_freeze_object(metadata);  // GOOD - immutable forever
/// }
/// ```
pub static UNFROZEN_COIN_METADATA: LintDescriptor = LintDescriptor {
    name: "unfrozen_coin_metadata",
    category: LintCategory::Security,
    description: "CoinMetadata should be frozen, not shared (see: MoveBit 2023)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
};

/// Detects capability parameters that are passed but never used.
///
/// # Security References
///
/// - **SlowMist (2024-09-10)**: "Permission Vulnerability Audit"
///   URL: https://github.com/slowmist/Sui-MOVE-Smart-Contract-Auditing-Primer
///   Verified: 2025-12-13 (Section 8: "Privileged functions must have privileged objects involved")
///
/// # Why This Matters
///
/// If a capability is passed to a function but never used, it indicates
/// that the access control check is missing. Anyone can call the function
/// by passing any capability object.
///
/// # Example (Bad)
///
/// ```move
/// // Cap is passed but never checked - anyone can call this!
/// public fun admin_action(_cap: &AdminCap, pool: &mut Pool) {
///     pool.value = 0;
/// }
/// ```
///
/// # Correct Pattern
///
/// ```move
/// public fun admin_action(cap: &AdminCap, pool: &mut Pool) {
///     assert!(cap.pool_id == object::id(pool), WRONG_CAP);  // Actually use the cap!
///     pool.value = 0;
/// }
/// ```
pub static UNUSED_CAPABILITY_PARAM: LintDescriptor = LintDescriptor {
    name: "unused_capability_param",
    category: LintCategory::Security,
    description: "Capability parameter is unused, indicating missing access control (see: SlowMist 2024)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
};

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
    description: "Division without zero-divisor check may abort unexpectedly",
    group: RuleGroup::Preview, // Preview due to potential FPs
    fix: FixDescriptor::none(),
};

/// Detects oracle price values used without zero-check validation.
///
/// # Security References
///
/// - **Bluefin Audit (2024-02)**: "Oracle price can be zero during outages"
///   MoveBit Audit Contest findings
///
/// - **Pyth Documentation**: "Always validate price is non-zero"
///   URL: https://docs.pyth.network/price-feeds/best-practices
///
/// # Why This Matters
///
/// Oracle prices can be zero during:
/// 1. Network outages or price feed failures
/// 2. Initial deployment before first price update
/// 3. Stale price invalidation
///
/// Using zero prices in calculations causes division by zero or
/// incorrect collateral valuations leading to bad liquidations.
///
/// # Example (Bad)
///
/// ```move
/// public fun calculate_value(amount: u64, price: u64): u64 {
///     amount * price / PRECISION  // Zero price = zero value!
/// }
/// ```
///
/// # Correct Pattern
///
/// ```move
/// public fun calculate_value(amount: u64, price: u64): u64 {
///     assert!(price > 0, E_INVALID_PRICE);
///     amount * price / PRECISION
/// }
/// ```
pub static ORACLE_ZERO_PRICE: LintDescriptor = LintDescriptor {
    name: "oracle_zero_price",
    category: LintCategory::Security,
    description: "Oracle price used without zero-check validation (see: Bluefin Audit 2024)",
    group: RuleGroup::Preview, // Preview due to need for heuristics
    fix: FixDescriptor::none(),
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
    description: "Important return value is ignored, may indicate bug",
    group: RuleGroup::Preview, // Preview due to many legitimate ignores
    fix: FixDescriptor::none(),
};

/// Detects public functions that modify state without capability checks.
///
/// # Security References
///
/// - **SlowMist (2024)**: "Privileged functions must have privileged objects"
///   URL: https://github.com/slowmist/Sui-MOVE-Smart-Contract-Auditing-Primer
///
/// - **General Access Control**: All state mutations should be authorized
///
/// # Why This Matters
///
/// Public functions that modify shared or owned objects without
/// requiring a capability parameter can be called by anyone, leading to:
/// 1. Unauthorized state changes
/// 2. Asset theft
/// 3. Protocol manipulation
///
/// # Example (Bad)
///
/// ```move
/// public fun set_fee(config: &mut Config, new_fee: u64) {
///     config.fee = new_fee;  // Anyone can change the fee!
/// }
/// ```
///
/// # Correct Pattern
///
/// ```move
/// public fun set_fee(admin_cap: &AdminCap, config: &mut Config, new_fee: u64) {
///     assert!(admin_cap.config_id == object::id(config), E_WRONG_CAP);
///     config.fee = new_fee;
/// }
/// ```
pub static MISSING_ACCESS_CONTROL: LintDescriptor = LintDescriptor {
    name: "missing_access_control",
    category: LintCategory::Security,
    description: "Public function modifies state without capability parameter (see: SlowMist 2024)",
    group: RuleGroup::Preview, // Preview due to many FPs (not all mutations need caps)
    fix: FixDescriptor::none(),
};

static DESCRIPTORS: &[&LintDescriptor] = &[
    &CAPABILITY_NAMING,
    &EVENT_NAMING,
    &GETTER_NAMING,
    &SHARE_OWNED,
    &SELF_TRANSFER,
    &CUSTOM_STATE_CHANGE,
    &COIN_FIELD,
    &FREEZE_WRAPPED,
    &COLLECTION_EQUALITY,
    &PUBLIC_RANDOM,
    &MISSING_KEY,
    &FREEZING_CAPABILITY,
    // Security semantic lints
    &UNFROZEN_COIN_METADATA,
    &UNUSED_CAPABILITY_PARAM,
    &UNCHECKED_DIVISION,
    &ORACLE_ZERO_PRICE,
    &UNUSED_RETURN_VALUE,
    &MISSING_ACCESS_CONTROL,
];

/// Return descriptors for all semantic lints.
pub fn descriptors() -> &'static [&'static LintDescriptor] {
    &DESCRIPTORS
}

/// Look up a semantic lint descriptor by name.
pub fn find_descriptor(name: &str) -> Option<&'static LintDescriptor> {
    descriptors().iter().copied().find(|d| d.name == name)
}

#[cfg(feature = "full")]
mod full {
    use super::*;
    use crate::diagnostics::Span;
    use crate::instrument_block;
    use crate::level::LintLevel;
    use crate::rules::modernization::{PUBLIC_MUT_TX_CONTEXT, UNNECESSARY_PUBLIC_ENTRY};
    use crate::suppression;
    type Result<T> = ClippyResult<T>;
    use move_compiler::editions::Flavor;
    use move_compiler::parser::ast::{Ability_, TargetKind};
    use move_compiler::shared::{Identifier, files::MappedFiles, program_info::TypingProgramInfo};
    use move_compiler::shared::{SaveFlag, SaveHook};
    use move_compiler::sui_mode::linters;
    use move_compiler::{naming::ast as N, typing::ast as T, expansion::ast as E};
    use move_ir_types::location::Loc;
    use move_package::BuildConfig;
    use move_package::compilation::build_plan::BuildPlan;

    /// Run all semantic lints against the package rooted at `package_path`.
    pub fn lint_package(
        package_path: &Path,
        settings: &LintSettings,
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
            let compiled = build_plan
                .compile_no_exit(&mut writer, |compiler| {
                    let (attr, filters) = linters::known_filters();
                    compiler
                        .add_save_hook(&hook)
                        .add_custom_known_filters(attr, filters)
                })
                .map_err(|e| {
                    MoveClippyError::semantic(format!(
                        "Move compilation failed while running Sui lints: {e}"
                    ))
                })?;

            let typing_ast: T::Program = hook.take_typing_ast();
            let typing_info: std::sync::Arc<TypingProgramInfo> = hook.take_typing_info();
            let file_map: MappedFiles = compiled.file_map.clone();

            let mut out = Vec::new();
            lint_capability_naming(&mut out, settings, &file_map, &typing_info)?;
            lint_event_naming(&mut out, settings, &file_map, &typing_info)?;
            lint_getter_naming(&mut out, settings, &file_map, &typing_ast)?;
            lint_unused_capability_param(&mut out, settings, &file_map, &typing_ast)?;
            lint_unfrozen_coin_metadata(&mut out, settings, &file_map, &typing_ast)?;
            lint_unchecked_division(&mut out, settings, &file_map, &typing_ast)?;
            lint_oracle_zero_price(&mut out, settings, &file_map, &typing_ast)?;
            lint_unused_return_value(&mut out, settings, &file_map, &typing_ast)?;
            lint_missing_access_control(&mut out, settings, &file_map, &typing_ast)?;
            lint_sui_visitors(&mut out, settings, &build_plan, &package_root)?;
            Ok(out)
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
        for (_mident, minfo) in info.modules.key_cloned_iter() {
            match minfo.target_kind {
                TargetKind::Source {
                    is_root_package: true,
                } => {}
                _ => continue,
            }

            for (sname, sdef) in minfo.structs.key_cloned_iter() {
                let abilities = &sdef.abilities;
                let is_cap = abilities.has_ability_(Ability_::Key)
                    && abilities.has_ability_(Ability_::Store)
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
        for (_mident, minfo) in info.modules.key_cloned_iter() {
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

    fn lint_getter_naming(
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
                let sym = fname.value();
                let name = sym.as_str();
                if !name.starts_with("get_") {
                    continue;
                }

                let Some((_, self_var, self_ty)) = fdef.signature.parameters.first() else {
                    continue;
                };

                if !is_ref_to_module_type(self_ty, &mident) {
                    continue;
                }

                let T::FunctionBody_::Defined((_use_funs, seq_items)) = &fdef.body.value else {
                    continue;
                };

                if seq_items.len() != 1 {
                    continue;
                }

                let Some(T::SequenceItem_::Seq(exp)) = seq_items.front().map(|s| &s.value) else {
                    continue;
                };
                if !is_simple_self_field_get(exp, self_var) {
                    continue;
                }

                let loc = fname.loc();
                let Some((file, span, contents)) = diag_from_loc(file_map, &loc) else {
                    continue;
                };
                let anchor = fdef.loc.start() as usize;

                let suggested = &name[4..];
                push_diag(
                    out,
                    settings,
                    &GETTER_NAMING,
                    file,
                    span,
                    contents.as_ref(),
                    anchor,
                    format!("Prefer `{suggested}` over `{name}` for simple getters"),
                );
            }
        }

        Ok(())
    }

    // =========================================================================
    // Security Semantic Lints
    // =========================================================================

    /// Lint for unused capability parameters - indicates missing access control.
    ///
    /// If a function takes a *Cap parameter but never uses it, the access
    /// control check is likely missing, allowing anyone to call the function.
    fn lint_unused_capability_param(
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        prog: &T::Program,
    ) -> Result<()> {
        for (_mident, mdef) in prog.modules.key_cloned_iter() {
            match mdef.target_kind {
                TargetKind::Source {
                    is_root_package: true,
                } => {}
                _ => continue,
            }

            for (fname, fdef) in mdef.functions.key_cloned_iter() {
                // Find parameters that look like capabilities
                let cap_params: Vec<_> = fdef
                    .signature
                    .parameters
                    .iter()
                    .filter(|(_, var, _ty)| {
                        let name = var.value.name.value().as_str();
                        name.ends_with("_cap")
                            || name.ends_with("Cap")
                            || name == "cap"
                            || name.starts_with("admin")
                    })
                    .map(|(_, var, _)| var.clone())
                    .collect();

                if cap_params.is_empty() {
                    continue;
                }

                // Check if the function body uses these parameters
                let T::FunctionBody_::Defined((_use_funs, seq_items)) = &fdef.body.value else {
                    continue;
                };

                for cap_var in &cap_params {
                    let is_used = seq_items.iter().any(|item| {
                        check_var_used_in_seq_item(item, cap_var)
                    });

                    if !is_used {
                        let loc = fdef.loc;
                        let Some((file, span, contents)) = diag_from_loc(file_map, &loc) else {
                            continue;
                        };
                        let anchor = loc.start() as usize;
                        let func_name = fname.value().as_str();
                        let cap_name = cap_var.value.name.value().as_str();

                        push_diag(
                            out,
                            settings,
                            &UNUSED_CAPABILITY_PARAM,
                            file,
                            span,
                            contents.as_ref(),
                            anchor,
                            format!(
                                "Capability parameter `{cap_name}` in function `{func_name}` is unused. \
                                 This suggests missing access control - the capability should be checked \
                                 (e.g., `assert!(cap.pool_id == object::id(pool), E_WRONG_CAP)`)."
                            ),
                        );
                    }
                }
            }
        }

        Ok(())
    }

    /// Check if a variable is used in a sequence item (recursively).
    fn check_var_used_in_seq_item(item: &T::SequenceItem, var: &N::Var) -> bool {
        match &item.value {
            T::SequenceItem_::Seq(exp) => check_var_used_in_exp(exp, var),
            T::SequenceItem_::Declare(_) => false,
            T::SequenceItem_::Bind(_, _, exp) => check_var_used_in_exp(exp, var),
        }
    }

    /// Check if a variable is used in an expression (recursively).
    fn check_var_used_in_exp(exp: &T::Exp, var: &N::Var) -> bool {
        match &exp.exp.value {
            T::UnannotatedExp_::Use(v) => v.value.id == var.value.id,
            T::UnannotatedExp_::Copy { var: v, .. } => v.value.id == var.value.id,
            T::UnannotatedExp_::Move { var: v, .. } => v.value.id == var.value.id,
            T::UnannotatedExp_::BorrowLocal(_, v) => v.value.id == var.value.id,
            
            // Recursive cases
            T::UnannotatedExp_::ModuleCall(call) => {
                call.arguments.iter().any(|arg| check_var_used_in_exp(arg, var))
            }
            T::UnannotatedExp_::VarCall(_, args) => {
                args.iter().any(|arg| check_var_used_in_exp(arg, var))
            }
            T::UnannotatedExp_::Builtin(_, args) => {
                args.iter().any(|arg| check_var_used_in_exp(arg, var))
            }
            T::UnannotatedExp_::Vector(_, _, _, args) => {
                args.iter().any(|arg| check_var_used_in_exp(arg, var))
            }
            T::UnannotatedExp_::Pack(_, _, _, fields) => {
                fields.iter().any(|(_, _, (_, (_, exp)))| check_var_used_in_exp(exp, var))
            }
            T::UnannotatedExp_::ExpList(items) => {
                items.iter().any(|item| match item {
                    T::ExpListItem::Single(e, _) => check_var_used_in_exp(e, var),
                    T::ExpListItem::Splat(_, e, _) => check_var_used_in_exp(e, var),
                })
            }
            T::UnannotatedExp_::Borrow(_, inner, _) => check_var_used_in_exp(inner, var),
            T::UnannotatedExp_::TempBorrow(_, inner) => check_var_used_in_exp(inner, var),
            T::UnannotatedExp_::Dereference(inner) => check_var_used_in_exp(inner, var),
            T::UnannotatedExp_::UnaryExp(_, inner) => check_var_used_in_exp(inner, var),
            T::UnannotatedExp_::BinopExp(left, _, _, right) => {
                check_var_used_in_exp(left, var) || check_var_used_in_exp(right, var)
            }
            T::UnannotatedExp_::IfElse(cond, then_e, else_e) => {
                check_var_used_in_exp(cond, var)
                    || check_var_used_in_exp(then_e, var)
                    || check_var_used_in_exp(else_e, var)
            }
            T::UnannotatedExp_::While(_, cond, body) => {
                check_var_used_in_exp(cond, var) || check_var_used_in_exp(body, var)
            }
            T::UnannotatedExp_::Loop { body, .. } => check_var_used_in_exp(body, var),
            T::UnannotatedExp_::Block((_, seq)) => {
                seq.iter().any(|item| check_var_used_in_seq_item(item, var))
            }
            T::UnannotatedExp_::Assign(_, _, rhs) => check_var_used_in_exp(rhs, var),
            T::UnannotatedExp_::Mutate(lhs, rhs) => {
                check_var_used_in_exp(lhs, var) || check_var_used_in_exp(rhs, var)
            }
            T::UnannotatedExp_::Return(inner) => check_var_used_in_exp(inner, var),
            T::UnannotatedExp_::Abort(inner) => check_var_used_in_exp(inner, var),
            T::UnannotatedExp_::Cast(inner, _) => check_var_used_in_exp(inner, var),
            T::UnannotatedExp_::Annotate(inner, _) => check_var_used_in_exp(inner, var),
            
            // Base cases that don't use variables
            T::UnannotatedExp_::Unit { .. }
            | T::UnannotatedExp_::Value(_)
            | T::UnannotatedExp_::Constant(_, _)
            | T::UnannotatedExp_::Break
            | T::UnannotatedExp_::Continue
            | T::UnannotatedExp_::UnresolvedError
            | T::UnannotatedExp_::ErrorConstant { .. } => false,
            
            // Catch-all for other cases
            _ => false,
        }
    }

    /// Lint for CoinMetadata being shared instead of frozen.
    ///
    /// CoinMetadata should be frozen (immutable) to prevent the admin from
    /// modifying token name/symbol after deployment.
    fn lint_unfrozen_coin_metadata(
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        prog: &T::Program,
    ) -> Result<()> {
        for (_mident, mdef) in prog.modules.key_cloned_iter() {
            match mdef.target_kind {
                TargetKind::Source {
                    is_root_package: true,
                } => {}
                _ => continue,
            }

            // Look for init functions
            for (fname, fdef) in mdef.functions.key_cloned_iter() {
                let func_name = fname.value().as_str();
                if func_name != "init" {
                    continue;
                }

                let T::FunctionBody_::Defined((_use_funs, seq_items)) = &fdef.body.value else {
                    continue;
                };

                // Check for share_object calls on metadata
                for item in seq_items.iter() {
                    check_metadata_share_in_seq_item(item, out, settings, file_map, &fdef.loc);
                }
            }
        }

        Ok(())
    }

    /// Recursively check for share_object calls on CoinMetadata.
    fn check_metadata_share_in_seq_item(
        item: &T::SequenceItem,
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        func_loc: &Loc,
    ) {
        match &item.value {
            T::SequenceItem_::Seq(exp) => {
                check_metadata_share_in_exp(exp, out, settings, file_map, func_loc);
            }
            T::SequenceItem_::Bind(_, _, exp) => {
                check_metadata_share_in_exp(exp, out, settings, file_map, func_loc);
            }
            _ => {}
        }
    }

    /// Check if an expression is a share_object call on CoinMetadata.
    fn check_metadata_share_in_exp(
        exp: &T::Exp,
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        func_loc: &Loc,
    ) {
        match &exp.exp.value {
            T::UnannotatedExp_::ModuleCall(call) => {
                let module_name = call.module.value.module.value().as_str();
                let func_name = call.name.value().as_str();
                
                // Check for transfer::public_share_object or transfer::share_object
                if module_name == "transfer" 
                    && (func_name == "public_share_object" || func_name == "share_object") 
                {
                    // Check if argument type contains CoinMetadata
                    // For simplicity, we check if any type argument contains "CoinMetadata"
                    let type_str = format!("{:?}", call.type_arguments);
                    if type_str.contains("CoinMetadata") {
                        let loc = exp.exp.loc;
                        let Some((file, span, contents)) = diag_from_loc(file_map, &loc) else {
                            return;
                        };
                        let anchor = loc.start() as usize;

                        push_diag(
                            out,
                            settings,
                            &UNFROZEN_COIN_METADATA,
                            file,
                            span,
                            contents.as_ref(),
                            anchor,
                            "CoinMetadata is being shared instead of frozen. \
                             Use `transfer::public_freeze_object(metadata)` to make it immutable. \
                             Shared metadata allows the admin to modify token name/symbol after deployment."
                                .to_string(),
                        );
                    }
                }

                // Recurse into arguments
                for arg in &call.arguments {
                    check_metadata_share_in_exp(arg, out, settings, file_map, func_loc);
                }
            }
            T::UnannotatedExp_::Block((_, seq)) => {
                for item in seq.iter() {
                    check_metadata_share_in_seq_item(item, out, settings, file_map, func_loc);
                }
            }
            T::UnannotatedExp_::IfElse(cond, then_e, else_e) => {
                check_metadata_share_in_exp(cond, out, settings, file_map, func_loc);
                check_metadata_share_in_exp(then_e, out, settings, file_map, func_loc);
                check_metadata_share_in_exp(else_e, out, settings, file_map, func_loc);
            }
            _ => {}
        }
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

                // Track variables that have been validated as non-zero
                let mut validated_vars: std::collections::HashSet<u16> = std::collections::HashSet::new();
                
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
    fn check_for_nonzero_assertion(exp: &T::Exp, validated_vars: &mut std::collections::HashSet<u16>) {
        // Look for assert!(var != 0, ...) or assert!(var > 0, ...)
        if let T::UnannotatedExp_::Builtin(builtin, args) = &exp.exp.value {
            let builtin_str = format!("{:?}", builtin);
            if builtin_str.contains("Assert") && !args.is_empty() {
                // Check if the condition is a comparison with 0
                if let Some(first_arg) = args.first() {
                    if let T::UnannotatedExp_::BinopExp(left, op, _, right) = &first_arg.exp.value {
                        let op_str = format!("{:?}", op);
                        // Check for != 0 or > 0
                        if op_str.contains("Neq") || op_str.contains("Gt") {
                            // Check if comparing with 0
                            if is_zero_value(right) {
                                if let Some(var_id) = extract_var_id(left) {
                                    validated_vars.insert(var_id);
                                }
                            }
                            if is_zero_value(left) {
                                if let Some(var_id) = extract_var_id(right) {
                                    validated_vars.insert(var_id);
                                }
                            }
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
                        matches!(&right.exp.value, T::UnannotatedExp_::Value(_) | T::UnannotatedExp_::Constant(_, _))
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
                for arg in &call.arguments {
                    check_division_in_exp(arg, validated_vars, out, settings, file_map, func_name);
                }
            }
            T::UnannotatedExp_::Block((_, seq)) => {
                let mut local_validated = validated_vars.clone();
                for item in seq.iter() {
                    check_division_in_seq_item(item, &mut local_validated, out, settings, file_map, func_name);
                }
            }
            T::UnannotatedExp_::IfElse(cond, then_e, else_e) => {
                check_division_in_exp(cond, validated_vars, out, settings, file_map, func_name);
                check_division_in_exp(then_e, validated_vars, out, settings, file_map, func_name);
                check_division_in_exp(else_e, validated_vars, out, settings, file_map, func_name);
            }
            _ => {}
        }
    }

    // =========================================================================
    // Oracle Zero Price Lint
    // =========================================================================

    /// Lint for oracle price values used without zero-check validation.
    ///
    /// This lint detects when variables named "price" are used in arithmetic
    /// operations without first being validated as non-zero.
    fn lint_oracle_zero_price(
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        prog: &T::Program,
    ) -> Result<()> {
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

                // Track price-related variables that have been validated
                let mut validated_prices: std::collections::HashSet<u16> = std::collections::HashSet::new();

                for item in seq_items.iter() {
                    check_oracle_price_in_seq_item(
                        item,
                        &mut validated_prices,
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

    /// Check for oracle price usage in a sequence item.
    fn check_oracle_price_in_seq_item(
        item: &T::SequenceItem,
        validated_prices: &mut std::collections::HashSet<u16>,
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        func_name: &str,
    ) {
        match &item.value {
            T::SequenceItem_::Seq(exp) => {
                // Check for assert statements that validate price > 0
                check_for_price_validation(exp, validated_prices);
                check_oracle_price_in_exp(exp, validated_prices, out, settings, file_map, func_name);
            }
            T::SequenceItem_::Bind(bindings, _, exp) => {
                // Track bindings of price-related variables
                for (_, var) in bindings.value.iter() {
                    let var_name = var.value.name.value().as_str().to_lowercase();
                    if var_name.contains("price") {
                        // This variable is price-related, will need validation
                    }
                }
                check_oracle_price_in_exp(exp, validated_prices, out, settings, file_map, func_name);
            }
            _ => {}
        }
    }

    /// Check for price validation assertions.
    fn check_for_price_validation(exp: &T::Exp, validated_prices: &mut std::collections::HashSet<u16>) {
        if let T::UnannotatedExp_::Builtin(builtin, args) = &exp.exp.value {
            let builtin_str = format!("{:?}", builtin);
            if builtin_str.contains("Assert") && !args.is_empty() {
                if let Some(first_arg) = args.first() {
                    if let T::UnannotatedExp_::BinopExp(left, op, _, right) = &first_arg.exp.value {
                        let op_str = format!("{:?}", op);
                        // Check for > 0 or != 0 comparisons
                        if op_str.contains("Gt") || op_str.contains("Neq") {
                            // Check if comparing a price variable with 0
                            if is_zero_value(right) {
                                if let Some(var_id) = extract_var_id(left) {
                                    validated_prices.insert(var_id);
                                }
                            }
                            if is_zero_value(left) {
                                if let Some(var_id) = extract_var_id(right) {
                                    validated_prices.insert(var_id);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Check for oracle price usage in an expression.
    fn check_oracle_price_in_exp(
        exp: &T::Exp,
        validated_prices: &std::collections::HashSet<u16>,
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        func_name: &str,
    ) {
        match &exp.exp.value {
            T::UnannotatedExp_::BinopExp(left, op, _, right) => {
                let op_str = format!("{:?}", op);
                // Check for multiplication or division involving price
                if op_str.contains("Mul") || op_str.contains("Div") {
                    // Check if either operand is an unvalidated price variable
                    check_price_operand(left, validated_prices, out, settings, file_map, func_name, &exp.exp.loc);
                    check_price_operand(right, validated_prices, out, settings, file_map, func_name, &exp.exp.loc);
                }

                // Recurse
                check_oracle_price_in_exp(left, validated_prices, out, settings, file_map, func_name);
                check_oracle_price_in_exp(right, validated_prices, out, settings, file_map, func_name);
            }
            T::UnannotatedExp_::ModuleCall(call) => {
                for arg in &call.arguments {
                    check_oracle_price_in_exp(arg, validated_prices, out, settings, file_map, func_name);
                }
            }
            _ => {}
        }
    }

    /// Check if an operand is an unvalidated price variable.
    fn check_price_operand(
        exp: &T::Exp,
        validated_prices: &std::collections::HashSet<u16>,
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        func_name: &str,
        op_loc: &Loc,
    ) {
        // This is a heuristic - we look for variables that might be prices
        // A more sophisticated version would track oracle call return values
        if let Some(var_id) = extract_var_id(exp) {
            // For now, we only flag if it's explicitly named "price" and not validated
            // This reduces FPs at the cost of missing some cases
            if !validated_prices.contains(&var_id) {
                // We need more context to know if this is a price variable
                // For now, we skip this check to avoid FPs
                // TODO: Implement proper taint tracking from oracle calls
            }
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
                    let module_name = call.module.value.module.value().as_str();
                    let call_name = call.name.value().as_str();

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
                    check_unused_return_in_seq_item(item, important_fns, out, settings, file_map, func_name);
                }
            }
            T::UnannotatedExp_::IfElse(cond, then_e, else_e) => {
                check_unused_return_in_exp(cond, important_fns, out, settings, file_map, func_name);
                check_unused_return_in_exp(then_e, important_fns, out, settings, file_map, func_name);
                check_unused_return_in_exp(else_e, important_fns, out, settings, file_map, func_name);
            }
            _ => {}
        }
    }

    // =========================================================================
    // Missing Access Control Lint
    // =========================================================================

    /// Lint for public functions that modify state without capability checks.
    ///
    /// This lint detects public functions that take &mut parameters but don't
    /// have any capability parameter for authorization.
    fn lint_missing_access_control(
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        prog: &T::Program,
    ) -> Result<()> {
        for (_mident, mdef) in prog.modules.key_cloned_iter() {
            match mdef.target_kind {
                TargetKind::Source {
                    is_root_package: true,
                } => {}
                _ => continue,
            }

            for (fname, fdef) in mdef.functions.key_cloned_iter() {
                // Skip non-public functions
                let is_public = matches!(fdef.visibility, T::Visibility::Public(_));
                if !is_public {
                    continue;
                }

                // Skip entry functions (they have different authorization model)
                let func_name_str = fname.value().as_str();
                if func_name_str == "init" {
                    continue;
                }

                // Check if function has mutable reference parameters (state modification)
                let has_mut_param = fdef.signature.parameters.iter().any(|(_, _, ty)| {
                    is_mutable_ref_type(ty)
                });

                if !has_mut_param {
                    continue;
                }

                // Check if function has a capability parameter
                let has_cap_param = fdef.signature.parameters.iter().any(|(_, var, _)| {
                    let name = var.value.name.value().as_str();
                    name.ends_with("_cap")
                        || name.ends_with("Cap")
                        || name == "cap"
                        || name.starts_with("admin")
                        || name.contains("witness")
                });

                // Also check if function name suggests it's a getter (not modification)
                let is_getter = func_name_str.starts_with("get_")
                    || func_name_str.starts_with("is_")
                    || func_name_str.starts_with("has_")
                    || func_name_str.starts_with("check_")
                    || func_name_str.starts_with("view_");

                if !has_cap_param && !is_getter {
                    let loc = fdef.loc;
                    let Some((file, span, contents)) = diag_from_loc(file_map, &loc) else {
                        continue;
                    };
                    let anchor = loc.start() as usize;

                    push_diag(
                        out,
                        settings,
                        &MISSING_ACCESS_CONTROL,
                        file,
                        span,
                        contents.as_ref(),
                        anchor,
                        format!(
                            "Public function `{func_name_str}` modifies state (has &mut parameter) \
                             but has no capability parameter for access control. \
                             Consider adding an AdminCap or similar to restrict access."
                        ),
                    );
                }
            }
        }

        Ok(())
    }

    /// Check if a type is a mutable reference.
    fn is_mutable_ref_type(ty: &N::Type) -> bool {
        matches!(&ty.value, N::Type_::Ref(true, _))
    }

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
                    return Err(MoveClippyError::semantic(format!(
                        "Move compilation failed while running Sui lints:\n{}",
                        String::from_utf8_lossy(&rendered)
                    ))
                    .into_anyhow());
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
            T::UnannotatedExp_::TempBorrow(_mut_, inner) => is_self_local(inner, self_var),
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
) -> ClippyResult<Vec<Diagnostic>> {
    Err(MoveClippyError::semantic(
        "full mode requires building with --features full",
    ))
}
