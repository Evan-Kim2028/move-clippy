use crate::diagnostics::Diagnostic;
use crate::error::Result as ClippyResult;
use crate::lint::LintSettings;
use move_compiler::parser::ast::TargetKind;
use move_compiler::shared::Identifier;
use move_compiler::shared::files::MappedFiles;
use move_compiler::shared::program_info::TypingProgramInfo;
use move_compiler::typing::ast as T;

use super::super::util::{diag_from_loc, push_diag};
use super::super::{
    CAPABILITY_TRANSFER_LITERAL_ADDRESS, CAPABILITY_TRANSFER_V2, SHARED_CAPABILITY_OBJECT,
};
use super::shared::{format_type, is_coin_type};

type Result<T> = ClippyResult<T>;

// =========================================================================
// Phase 4 Preview Lints (type-based, ability-based detection)
// =========================================================================

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
#[allow(unused_variables)]
pub(crate) fn lint_capability_antipatterns(
    _out: &mut Vec<Diagnostic>,
    _settings: &LintSettings,
    _file_map: &MappedFiles,
    _info: &TypingProgramInfo,
    _prog: &T::Program,
) -> Result<()> {
    // DEPRECATED: No-op. See docstring for rationale.
    Ok(())
}

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
pub(crate) fn lint_shared_capability_object(
    out: &mut Vec<Diagnostic>,
    settings: &LintSettings,
    file_map: &MappedFiles,
    prog: &T::Program,
) -> Result<()> {
    const SHARE_FUNCTIONS: &[(&str, &str)] = &[
        ("transfer", "share_object"),
        ("transfer", "public_share_object"),
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
                    item, share_fns, out, settings, file_map, func_name,
                );
            }
        }
        T::UnannotatedExp_::IfElse(cond, if_body, else_body) => {
            check_shared_capability_in_exp(cond, share_fns, out, settings, file_map, func_name);
            check_shared_capability_in_exp(if_body, share_fns, out, settings, file_map, func_name);
            if let Some(else_e) = else_body {
                check_shared_capability_in_exp(
                    else_e, share_fns, out, settings, file_map, func_name,
                );
            }
        }
        T::UnannotatedExp_::While(_, cond, body) => {
            check_shared_capability_in_exp(cond, share_fns, out, settings, file_map, func_name);
            check_shared_capability_in_exp(body, share_fns, out, settings, file_map, func_name);
        }
        T::UnannotatedExp_::Loop { body, .. } => {
            check_shared_capability_in_exp(body, share_fns, out, settings, file_map, func_name);
        }
        _ => {}
    }
}

/// Detects capability transfers to literal addresses.
pub(crate) fn lint_capability_transfer_literal_address(
    out: &mut Vec<Diagnostic>,
    settings: &LintSettings,
    file_map: &MappedFiles,
    prog: &T::Program,
) -> Result<()> {
    const TRANSFER_FUNCTIONS: &[(&str, &str)] =
        &[("transfer", "transfer"), ("transfer", "public_transfer")];

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

// =========================================================================
// Capability Transfer V2 Lint (type-based)
// =========================================================================

/// Detect capability transfers to non-sender addresses.
///
/// Flags transfer::transfer(cap, addr) where:
/// - cap has capability abilities (key + store, no copy, no drop)
/// - addr is not tx_context::sender(ctx)
pub(crate) fn lint_capability_transfer_v2(
    out: &mut Vec<Diagnostic>,
    settings: &LintSettings,
    file_map: &MappedFiles,
    prog: &T::Program,
) -> Result<()> {
    const TRANSFER_FUNCTIONS: &[(&str, &str)] =
        &[("transfer", "transfer"), ("transfer", "public_transfer")];

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
