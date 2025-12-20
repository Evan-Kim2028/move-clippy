use crate::diagnostics::Diagnostic;
use crate::error::Result as ClippyResult;
use crate::lint::LintSettings;
use move_compiler::naming::ast as N;
use move_compiler::parser::ast::TargetKind;
use move_compiler::shared::Identifier;
use move_compiler::shared::{files::MappedFiles, program_info::TypingProgramInfo};
use move_compiler::typing::ast as T;

use super::super::{COPYABLE_FUNGIBLE_TYPE, NON_TRANSFERABLE_FUNGIBLE_OBJECT};
use super::super::util::{diag_from_loc, push_diag};
use super::shared::strip_refs;

type Result<T> = ClippyResult<T>;

pub(crate) fn lint_non_transferable_fungible_object(
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
            let is_non_transferable = has_key_ability(abilities) && !has_store_ability(abilities);
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
// Copyable Fungible Type Lint (type-based, experimental)
// =========================================================================

fn module_type_key(ty: &N::Type_) -> Option<String> {
    let N::Type_::Apply(_, type_name, _) = ty else {
        return None;
    };
    let N::TypeName_::ModuleType(mident, struct_name) = &type_name.value else {
        return None;
    };
    Some(format!(
        "{}::{}",
        mident.value.module.value(),
        struct_name.value()
    ))
}

fn collect_transferred_types(prog: &T::Program) -> std::collections::BTreeSet<String> {
    let mut types = std::collections::BTreeSet::new();
    const TRANSFER_FUNCTIONS: &[(&str, &str)] =
        &[("transfer", "transfer"), ("transfer", "public_transfer")];

    for (_mident, mdef) in prog.modules.key_cloned_iter() {
        match mdef.target_kind {
            TargetKind::Source {
                is_root_package: true,
            } => {}
            _ => continue,
        }

        for (_fname, fdef) in mdef.functions.key_cloned_iter() {
            let T::FunctionBody_::Defined((_use_funs, seq_items)) = &fdef.body.value else {
                continue;
            };
            for item in seq_items.iter() {
                collect_transferred_types_in_seq_item(item, TRANSFER_FUNCTIONS, &mut types);
            }
        }
    }

    types
}

fn collect_transferred_types_in_seq_item(
    item: &T::SequenceItem,
    transfer_fns: &[(&str, &str)],
    out: &mut std::collections::BTreeSet<String>,
) {
    match &item.value {
        T::SequenceItem_::Seq(exp) => {
            collect_transferred_types_in_exp(exp, transfer_fns, out);
        }
        T::SequenceItem_::Bind(_, _, exp) => {
            collect_transferred_types_in_exp(exp, transfer_fns, out);
        }
        _ => {}
    }
}

fn collect_transferred_types_in_exp(
    exp: &T::Exp,
    transfer_fns: &[(&str, &str)],
    out: &mut std::collections::BTreeSet<String>,
) {
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
            && let Some(type_key) = module_type_key(strip_refs(&type_arg.value))
        {
            out.insert(type_key);
        }
    }

    match &exp.exp.value {
        T::UnannotatedExp_::ModuleCall(call) => {
            collect_transferred_types_in_exp(&call.arguments, transfer_fns, out);
        }
        T::UnannotatedExp_::Block((_, seq_items)) => {
            for item in seq_items.iter() {
                collect_transferred_types_in_seq_item(item, transfer_fns, out);
            }
        }
        T::UnannotatedExp_::IfElse(cond, if_body, else_body) => {
            collect_transferred_types_in_exp(cond, transfer_fns, out);
            collect_transferred_types_in_exp(if_body, transfer_fns, out);
            if let Some(else_e) = else_body {
                collect_transferred_types_in_exp(else_e, transfer_fns, out);
            }
        }
        T::UnannotatedExp_::While(_, cond, body) => {
            collect_transferred_types_in_exp(cond, transfer_fns, out);
            collect_transferred_types_in_exp(body, transfer_fns, out);
        }
        T::UnannotatedExp_::Loop { body, .. } => {
            collect_transferred_types_in_exp(body, transfer_fns, out);
        }
        _ => {}
    }
}

pub(crate) fn lint_copyable_fungible_type(
    out: &mut Vec<Diagnostic>,
    settings: &LintSettings,
    file_map: &MappedFiles,
    prog: &T::Program,
    info: &TypingProgramInfo,
) -> Result<()> {
    use crate::type_classifier::{has_copy_ability, has_key_ability, has_store_ability};

    let transferred_types = collect_transferred_types(prog);

    for (mident, minfo) in info.modules.key_cloned_iter() {
        match minfo.target_kind {
            TargetKind::Source {
                is_root_package: true,
            } => {}
            _ => continue,
        }

        for (sname, sdef) in minfo.structs.key_cloned_iter() {
            let abilities = &sdef.abilities;
            if !has_copy_ability(abilities) {
                continue;
            }

            let has_key = has_key_ability(abilities);
            let has_store = has_store_ability(abilities);
            let type_key = format!("{}::{}", mident.value.module.value(), sname.value());
            let is_transferred = transferred_types.contains(&type_key);

            if !has_key && !(has_store && is_transferred) {
                continue;
            }

            let sym = sname.value();
            let name_str = sym.as_str();
            let loc = sname.loc();
            let Some((file, span, contents)) = diag_from_loc(file_map, &loc) else {
                continue;
            };
            let anchor = loc.start() as usize;
            let reason = if has_key {
                "has `key`"
            } else {
                "is used in transfer operations"
            };

            push_diag(
                out,
                settings,
                &COPYABLE_FUNGIBLE_TYPE,
                file,
                span,
                contents.as_ref(),
                anchor,
                format!(
                    "Struct `{name_str}` is `copy` and {reason}, making it duplicable as a fungible value. \
                     Remove `copy` or redesign the type to prevent infinite duplication."
                ),
            );
        }
    }

    Ok(())
}
