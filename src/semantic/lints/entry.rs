use crate::diagnostics::Diagnostic;
use crate::error::Result as ClippyResult;
use crate::lint::LintSettings;
use move_compiler::naming::ast as N;
use move_compiler::parser::ast::TargetKind;
use move_compiler::shared::Identifier;
use move_compiler::shared::files::MappedFiles;
use move_compiler::typing::ast as T;

use super::super::util::{diag_from_loc, push_diag};
use super::super::{ENTRY_FUNCTION_RETURNS_VALUE, PRIVATE_ENTRY_FUNCTION};
use super::shared::format_type;

type Result<T> = ClippyResult<T>;

pub(crate) fn lint_entry_function_returns_value(
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

pub(crate) fn lint_private_entry_function(
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
