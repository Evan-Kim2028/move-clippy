use crate::diagnostics::Diagnostic;
use crate::error::Result as ClippyResult;
use crate::lint::LintSettings;
use move_compiler::naming::ast as N;
use move_compiler::parser::ast::TargetKind;
use move_compiler::shared::Identifier;
use move_compiler::shared::{files::MappedFiles, program_info::TypingProgramInfo};
use move_compiler::typing::ast as T;

use super::super::util::{diag_from_loc, push_diag};
use super::super::{GENERIC_TYPE_WITNESS_UNUSED, MISSING_WITNESS_DROP_V2, WITNESS_ANTIPATTERNS};
// INVALID_OTW removed - duplicates Sui Verifier's one_time_witness_verifier.rs
use super::shared::{format_type, strip_refs};

type Result<T> = ClippyResult<T>;

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
            exp_uses_var(scrut, target) || arms.iter().any(|(_vname, e)| exp_uses_var(e, target))
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
pub(crate) fn lint_generic_type_witness_unused(
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
// Missing Witness Drop V2 Lint (type-based)
// =========================================================================

pub(crate) fn lint_missing_witness_drop_v2(
    out: &mut Vec<Diagnostic>,
    settings: &LintSettings,
    file_map: &MappedFiles,
    info: &TypingProgramInfo,
) -> Result<()> {
    use crate::type_classifier::has_drop_ability;

    for (mident, minfo) in info.modules.key_cloned_iter() {
        match minfo.target_kind {
            TargetKind::Source {
                is_root_package: true,
            } => {}
            _ => continue,
        }

        let module_name = mident.value.module.value();
        let module_name_str = module_name.as_str();
        let expected_otw_name = module_name_str.to_uppercase();

        for (sname, sdef) in minfo.structs.key_cloned_iter() {
            let struct_name_sym = sname.value();
            let struct_name = struct_name_sym.as_str();

            if struct_name != expected_otw_name {
                continue;
            }

            let is_empty = match &sdef.fields {
                N::StructFields::Defined(_, fields) => fields.is_empty(),
                N::StructFields::Native(_) => true,
            };
            if !is_empty {
                continue;
            }

            let abilities = &sdef.abilities;
            if has_drop_ability(abilities) {
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
                &MISSING_WITNESS_DROP_V2,
                file,
                span,
                contents.as_ref(),
                anchor,
                format!(
                    "Struct `{struct_name}` in module `{module_name_str}` appears to be a \
                     one-time witness (OTW) but is missing the `drop` ability. \
                     Add `has drop` to the struct."
                ),
            );
        }
    }

    Ok(())
}

// =========================================================================
// REMOVED: Invalid OTW Lint
// =========================================================================
// This lint duplicates the Sui Verifier's one_time_witness_verifier.rs which
// is authoritative and will reject modules at publish time. The Sui verifier
// checks:
// - OTW must have only `drop` ability
// - OTW must have only one boolean field
// - OTW must not be generic
// - OTW must not be instantiated in the module
// See: sui-execution/v0/sui-verifier/src/one_time_witness_verifier.rs

// =========================================================================
// Witness Antipatterns Lint (type-based)
// =========================================================================

pub(crate) fn lint_witness_antipatterns(
    out: &mut Vec<Diagnostic>,
    settings: &LintSettings,
    file_map: &MappedFiles,
    info: &TypingProgramInfo,
    prog: &T::Program,
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

        let module_name = mident.value.module.value();
        let module_name_str = module_name.as_str();
        let expected_otw_name = module_name_str.to_uppercase();

        for (sname, sdef) in minfo.structs.key_cloned_iter() {
            let struct_name_sym = sname.value();
            let struct_name = struct_name_sym.as_str();

            let abilities = &sdef.abilities;
            let has_drop = has_drop_ability(abilities);
            let is_empty = match &sdef.fields {
                N::StructFields::Defined(_, fields) => fields.is_empty(),
                N::StructFields::Native(_) => true,
            };
            let name_is_witness =
                struct_name.contains("Witness") || struct_name == expected_otw_name;

            if !has_drop || !is_empty {
                continue;
            }

            if !name_is_witness {
                continue;
            }

            if has_copy_ability(abilities) {
                let loc = sname.loc();
                let Some((file, span, contents)) = diag_from_loc(file_map, &loc) else {
                    continue;
                };
                let anchor = loc.start() as usize;

                push_diag(
                    out,
                    settings,
                    &WITNESS_ANTIPATTERNS,
                    file,
                    span,
                    contents.as_ref(),
                    anchor,
                    format!(
                        "Witness struct `{struct_name}` has `copy` ability. \
                         This allows the witness to be duplicated, defeating the proof-of-ownership pattern. \
                         Remove `copy` from the abilities."
                    ),
                );
            }

            if has_store_ability(abilities) {
                let loc = sname.loc();
                let Some((file, span, contents)) = diag_from_loc(file_map, &loc) else {
                    continue;
                };
                let anchor = loc.start() as usize;

                push_diag(
                    out,
                    settings,
                    &WITNESS_ANTIPATTERNS,
                    file,
                    span,
                    contents.as_ref(),
                    anchor,
                    format!(
                        "Witness struct `{struct_name}` has `store` ability. \
                         This allows the witness to be persisted and replayed. \
                         Remove `store` from the abilities."
                    ),
                );
            }

            if has_key_ability(abilities) {
                let loc = sname.loc();
                let Some((file, span, contents)) = diag_from_loc(file_map, &loc) else {
                    continue;
                };
                let anchor = loc.start() as usize;

                push_diag(
                    out,
                    settings,
                    &WITNESS_ANTIPATTERNS,
                    file,
                    span,
                    contents.as_ref(),
                    anchor,
                    format!(
                        "Witness struct `{struct_name}` has `key` ability. \
                         Witnesses are ephemeral proofs and should not be objects. \
                         This conflates the witness pattern with the capability pattern."
                    ),
                );
            }
        }

        if let Some(mdef) = prog.modules.get(&mident) {
            for (fname, fdef) in mdef.functions.key_cloned_iter() {
                let is_public = matches!(
                    fdef.visibility,
                    move_compiler::expansion::ast::Visibility::Public(_)
                );
                if !is_public {
                    continue;
                }

                let ret_ty = &fdef.signature.return_type;
                if let N::Type_::Apply(_, type_name, _) = &ret_ty.value
                    && let N::TypeName_::ModuleType(ret_mident, ret_struct) = &type_name.value
                {
                    if ret_mident.value.module.value() != module_name {
                        continue;
                    }

                    let ret_struct_sym = ret_struct.value();
                    let ret_struct_name = ret_struct_sym.as_str();

                    if let Some(sdef) = minfo.structs.get_(&ret_struct_sym) {
                        let is_empty = match &sdef.fields {
                            N::StructFields::Defined(_, fields) => fields.is_empty(),
                            N::StructFields::Native(_) => true,
                        };
                        let has_drop = has_drop_ability(&sdef.abilities);
                        let is_witness_name = ret_struct_name.contains("Witness")
                            || ret_struct_name == expected_otw_name;

                        if is_empty && has_drop && is_witness_name {
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
                                &WITNESS_ANTIPATTERNS,
                                file,
                                span,
                                contents.as_ref(),
                                anchor,
                                format!(
                                    "Public function `{fn_name}` returns witness type `{ret_struct_name}`. \
                                     Witnesses should only be constructible within their module. \
                                     Make this function private or package-internal."
                                ),
                            );
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
