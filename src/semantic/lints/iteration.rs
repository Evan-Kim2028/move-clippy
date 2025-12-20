use crate::diagnostics::Diagnostic;
use crate::error::Result as ClippyResult;
use crate::lint::LintSettings;
use move_compiler::naming::ast as N;
use move_compiler::parser::ast::TargetKind;
use move_compiler::shared::Identifier;
use move_compiler::shared::files::MappedFiles;
use move_compiler::typing::ast as T;

use super::super::util::{diag_from_loc, push_diag};
use super::super::{MUT_KEY_PARAM_MISSING_AUTHORITY, UNBOUNDED_ITERATION_OVER_PARAM_VECTOR};
use super::shared::{format_type, is_coin_type, strip_refs};

type Result<T> = ClippyResult<T>;

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

fn is_public_entry_function(fdef: &T::Function) -> bool {
    fdef.entry.is_some()
        && matches!(
            fdef.visibility,
            move_compiler::expansion::ast::Visibility::Public(_)
        )
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
    crate::type_classifier::abilities_of_type(&inner.value)
        .is_some_and(|a| crate::type_classifier::has_key_ability(&a))
}

fn is_capability_like_type(ty: &N::Type_) -> bool {
    let inner = strip_refs(ty);
    !is_coin_type(inner) && crate::type_classifier::is_capability_type_from_ty(inner)
}

/// Detects public entry functions that take `&mut` key objects without any explicit authority parameter.
pub(crate) fn lint_mut_key_param_missing_authority(
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
        T::UnannotatedExp_::BorrowLocal(_mut_, v) => Some(v.value.id),
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
            check_unbounded_iter_in_exp(exp, vector_param_ids, out, settings, file_map, func_name);
        }
        T::SequenceItem_::Bind(_, _, exp) => {
            check_unbounded_iter_in_exp(exp, vector_param_ids, out, settings, file_map, func_name);
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
        T::UnannotatedExp_::IfElse(cond, if_body, e_opt) => {
            check_unbounded_iter_in_exp(cond, vector_param_ids, out, settings, file_map, func_name);
            check_unbounded_iter_in_exp(
                if_body,
                vector_param_ids,
                out,
                settings,
                file_map,
                func_name,
            );
            if let Some(e) = e_opt {
                check_unbounded_iter_in_exp(
                    e,
                    vector_param_ids,
                    out,
                    settings,
                    file_map,
                    func_name,
                );
            }
        }
        T::UnannotatedExp_::Loop { body, .. } => {
            check_unbounded_iter_in_exp(body, vector_param_ids, out, settings, file_map, func_name);
        }
        _ => {}
    }
}

/// Detects unbounded loops over a vector parameter in public entry functions.
pub(crate) fn lint_unbounded_iteration_over_param_vector(
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
