use crate::diagnostics::Diagnostic;
use crate::error::Result as ClippyResult;
use crate::lint::LintSettings;
use move_compiler::parser::ast::TargetKind;
use move_compiler::shared::Identifier;
use move_compiler::shared::files::MappedFiles;
use move_compiler::typing::ast as T;

use super::super::EVENT_EMIT_TYPE_SANITY;
use super::super::util::{diag_from_loc, push_diag};
use super::shared::format_type;

type Result<T> = ClippyResult<T>;

// =========================================================================
// Event Emit Type Sanity Lint
// =========================================================================

pub(crate) fn lint_event_emit_type_sanity(
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
            let abilities = crate::type_classifier::abilities_of_type(&type_arg.value);
            let is_event_like = crate::type_classifier::is_event_like_type(&type_arg.value);

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
                check_event_emit_in_seq_item(item, emit_fns, out, settings, file_map, func_name);
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
