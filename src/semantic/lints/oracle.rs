use crate::diagnostics::Diagnostic;
use crate::error::Result as ClippyResult;
use crate::lint::LintSettings;
use move_compiler::parser::ast::TargetKind;
use move_compiler::shared::Identifier;
use move_compiler::shared::files::MappedFiles;
use move_compiler::typing::ast as T;

use super::super::STALE_ORACLE_PRICE_V2;
use super::super::util::{diag_from_loc, push_diag};

type Result<T> = ClippyResult<T>;

// =========================================================================
// Stale Oracle Price V2 Lint (type-based)
// 
// DEPRECATED: This lint is superseded by stale_oracle_price_v3 in absint_lints.rs
// which uses CFG-aware dataflow analysis for rigorous detection.
// This version is kept for backwards compatibility but will be removed.
// =========================================================================

const ORACLE_MODULES: &[(&str, &[&str])] = &[
    ("pyth", &["get_price_unsafe", "price_unsafe"]),
    ("price_info", &["get_price_unsafe"]),
    ("switchboard", &["get_price_unsafe"]),
    ("supra", &["get_price_unsafe"]),
];

pub(crate) fn lint_stale_oracle_price_v2(
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

            let fn_name_sym = fname.value();
            let fn_name = fn_name_sym.as_str();

            for item in seq_items.iter() {
                check_stale_oracle_in_seq_item(item, out, settings, file_map, fn_name);
            }
        }
    }

    Ok(())
}

fn check_stale_oracle_in_seq_item(
    item: &T::SequenceItem,
    out: &mut Vec<Diagnostic>,
    settings: &LintSettings,
    file_map: &MappedFiles,
    func_name: &str,
) {
    match &item.value {
        T::SequenceItem_::Seq(exp) => {
            check_stale_oracle_in_exp(exp, out, settings, file_map, func_name);
        }
        T::SequenceItem_::Bind(_, _, exp) => {
            check_stale_oracle_in_exp(exp, out, settings, file_map, func_name);
        }
        _ => {}
    }
}

fn check_stale_oracle_in_exp(
    exp: &T::Exp,
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

        let is_unsafe_oracle_call = ORACLE_MODULES.iter().any(|(oracle_mod, unsafe_fns)| {
            module_name == *oracle_mod && unsafe_fns.iter().any(|f| call_name == *f)
        });

        if is_unsafe_oracle_call {
            let loc = exp.exp.loc;
            let Some((file, span, contents)) = diag_from_loc(file_map, &loc) else {
                return;
            };
            let anchor = loc.start() as usize;

            push_diag(
                out,
                settings,
                &STALE_ORACLE_PRICE_V2,
                file,
                span,
                contents.as_ref(),
                anchor,
                format!(
                    "Call to `{module_name}::{call_name}` in `{func_name}` may return stale prices. \
                     Consider using `get_price_no_older_than` with an appropriate max age."
                ),
            );
        }
    }

    match &exp.exp.value {
        T::UnannotatedExp_::ModuleCall(call) => {
            check_stale_oracle_in_exp(&call.arguments, out, settings, file_map, func_name);
        }
        T::UnannotatedExp_::Block((_, seq_items)) => {
            for item in seq_items.iter() {
                check_stale_oracle_in_seq_item(item, out, settings, file_map, func_name);
            }
        }
        T::UnannotatedExp_::IfElse(cond, if_body, else_body) => {
            check_stale_oracle_in_exp(cond, out, settings, file_map, func_name);
            check_stale_oracle_in_exp(if_body, out, settings, file_map, func_name);
            if let Some(else_e) = else_body {
                check_stale_oracle_in_exp(else_e, out, settings, file_map, func_name);
            }
        }
        T::UnannotatedExp_::While(_, cond, body) => {
            check_stale_oracle_in_exp(cond, out, settings, file_map, func_name);
            check_stale_oracle_in_exp(body, out, settings, file_map, func_name);
        }
        T::UnannotatedExp_::Loop { body, .. } => {
            check_stale_oracle_in_exp(body, out, settings, file_map, func_name);
        }
        _ => {}
    }
}
