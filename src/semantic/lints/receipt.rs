use crate::diagnostics::Diagnostic;
use crate::error::Result as ClippyResult;
use crate::lint::LintSettings;
use move_compiler::naming::ast as N;
use move_compiler::parser::ast::TargetKind;
use move_compiler::shared::Identifier;
use move_compiler::shared::files::MappedFiles;
use move_compiler::typing::ast as T;

use super::super::DROPPABLE_FLASH_LOAN_RECEIPT;
use super::super::util::{diag_from_loc, push_diag};
use super::shared::{format_type, is_coin_or_balance_type, strip_refs};

type Result<T> = ClippyResult<T>;

#[allow(dead_code)]
fn flatten_return_types(ret: &N::Type) -> Vec<&N::Type> {
    match &ret.value {
        N::Type_::Apply(_, type_name, type_args)
            if matches!(type_name.value, N::TypeName_::Multiple(_)) =>
        {
            type_args.iter().collect()
        }
        _ => vec![ret],
    }
}

#[allow(dead_code)]
fn is_root_module_type(prog: &T::Program, type_name: &N::TypeName_) -> bool {
    let N::TypeName_::ModuleType(mident, _) = type_name else {
        return false;
    };
    prog.modules.get(mident).is_some_and(|mdef| {
        matches!(
            mdef.target_kind,
            TargetKind::Source {
                is_root_package: true
            }
        )
    })
}

#[allow(dead_code)]
fn type_param_ids_in_type(ty: &N::Type_) -> std::collections::BTreeSet<N::TParamID> {
    use std::collections::BTreeSet;
    match ty {
        N::Type_::Param(tp) => BTreeSet::from([tp.id]),
        N::Type_::Ref(_, inner) => type_param_ids_in_type(&inner.value),
        N::Type_::Apply(_, _name, args) => {
            let mut out = BTreeSet::new();
            for arg in args {
                out.extend(type_param_ids_in_type(&arg.value));
            }
            out
        }
        N::Type_::Fun(args, ret) => {
            let mut out = BTreeSet::new();
            for arg in args {
                out.extend(type_param_ids_in_type(&arg.value));
            }
            out.extend(type_param_ids_in_type(&ret.value));
            out
        }
        _ => BTreeSet::new(),
    }
}

// =========================================================================
// Droppable Flash Loan Receipt Lint (type-based, experimental)
// =========================================================================

pub(crate) fn lint_droppable_flash_loan_receipt(
    out: &mut Vec<Diagnostic>,
    settings: &LintSettings,
    file_map: &MappedFiles,
    prog: &T::Program,
) -> Result<()> {
    use crate::type_classifier::{abilities_of_type, has_drop_ability};

    for (_mident, mdef) in prog.modules.key_cloned_iter() {
        match mdef.target_kind {
            TargetKind::Source {
                is_root_package: true,
            } => {}
            _ => continue,
        }

        for (fname, fdef) in mdef.functions.key_cloned_iter() {
            let return_types = flatten_return_types(&fdef.signature.return_type);
            let mut has_coin_or_balance = false;
            let mut droppable_structs = Vec::new();

            for ret_ty in return_types {
                let stripped = strip_refs(&ret_ty.value);
                if is_coin_or_balance_type(stripped) {
                    has_coin_or_balance = true;
                    continue;
                }

                let N::Type_::Apply(_, type_name, _) = stripped else {
                    continue;
                };
                if !matches!(type_name.value, N::TypeName_::ModuleType(_, _)) {
                    continue;
                }

                if abilities_of_type(stripped).is_some_and(|a| has_drop_ability(&a)) {
                    droppable_structs.push(format_type(stripped));
                }
            }

            if !has_coin_or_balance || droppable_structs.is_empty() {
                continue;
            }

            let loc = fdef.loc;
            let Some((file, span, contents)) = diag_from_loc(file_map, &loc) else {
                continue;
            };
            let anchor = loc.start() as usize;
            let fn_name_sym = fname.value();
            let fn_name = fn_name_sym.as_str();

            for receipt_ty in droppable_structs {
                push_diag(
                    out,
                    settings,
                    &DROPPABLE_FLASH_LOAN_RECEIPT,
                    file.clone(),
                    span,
                    contents.as_ref(),
                    anchor,
                    format!(
                        "Function `{fn_name}` returns a Coin/Balance with droppable `{receipt_ty}`. \
                         If this is a flash loan receipt, it must NOT have `drop`, or borrowers can ignore repayment."
                    ),
                );
            }
        }
    }

    Ok(())
}

// =========================================================================
// Receipt Missing Phantom Type Lint (DEPRECATED)
// =========================================================================

/// DEPRECATED: This lint has high false positive rate.
///
/// The heuristic "function takes Coin<T> and returns something without T"
/// flags many legitimate patterns:
/// - Pool creation functions (create_pool returns Pool, not Pool<T>)
/// - Position creation (create_position returns Position)
/// - Game outcomes (play returns Outcome, not Outcome<T>)
/// - Vesting wallet creation (returns OwnerCap<Witness>)
///
/// The assumption that all such patterns are flash loan receipts is incorrect.
/// Flash loan detection should be done via `droppable_flash_loan_receipt` which
/// specifically checks for droppable receipts returned alongside Coin/Balance.
#[allow(unused_variables)]
pub(crate) fn lint_receipt_missing_phantom_type(
    _out: &mut Vec<Diagnostic>,
    _settings: &LintSettings,
    _file_map: &MappedFiles,
    _prog: &T::Program,
) -> Result<()> {
    // DEPRECATED: No-op. See docstring for rationale.
    Ok(())
}
