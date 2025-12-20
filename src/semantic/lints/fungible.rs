use crate::diagnostics::Diagnostic;
use crate::error::Result as ClippyResult;
use crate::lint::LintSettings;
use move_compiler::parser::ast::TargetKind;
use move_compiler::shared::Identifier;
use move_compiler::shared::{files::MappedFiles, program_info::TypingProgramInfo};

use super::super::NON_TRANSFERABLE_FUNGIBLE_OBJECT;
use super::super::util::{diag_from_loc, push_diag};

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
