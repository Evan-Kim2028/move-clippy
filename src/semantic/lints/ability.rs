use crate::diagnostics::Diagnostic;
use crate::error::Result as ClippyResult;
use crate::lint::LintSettings;
use move_compiler::naming::ast as N;
use move_compiler::parser::ast::TargetKind;
use move_compiler::shared::Identifier;
use move_compiler::shared::{files::MappedFiles, program_info::TypingProgramInfo};

use super::super::util::{diag_from_loc, push_diag};
use super::super::{COPYABLE_CAPABILITY, DROPPABLE_CAPABILITY, DROPPABLE_HOT_POTATO_V2};

type Result<T> = ClippyResult<T>;

// =========================================================================
// Droppable Hot Potato V2 Lint (type-based, zero FP)
// =========================================================================

/// Detect structs with ONLY the `drop` ability (no other abilities).
///
/// A struct with only `drop` is almost always a bug:
/// 1. If it's a hot potato, it should have NO abilities
/// 2. If it's a witness, it should be empty
///
/// This lint is type-based with zero false positives because:
/// - Structs with `copy + drop` are events (legitimate)
/// - Structs with `key + store` are resources (legitimate)
/// - Structs with no abilities are hot potatoes (correct)
/// - Structs with ONLY `drop` are broken hot potatoes (bug!)
pub(crate) fn lint_droppable_hot_potato_v2(
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

            // Check for "only drop" pattern: has drop, but no copy, no key, no store
            let has_only_drop = has_drop_ability(abilities)
                && !has_copy_ability(abilities)
                && !has_key_ability(abilities)
                && !has_store_ability(abilities);

            if !has_only_drop {
                continue;
            }

            // Skip empty structs (0 fields) - these are witness/marker types
            // Witness types legitimately have only `drop` ability
            let is_empty = match &sdef.fields {
                N::StructFields::Defined(_, fields) => fields.is_empty(),
                N::StructFields::Native(_) => true, // Native structs, skip them
            };
            if is_empty {
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
                &DROPPABLE_HOT_POTATO_V2,
                file,
                span,
                contents.as_ref(),
                anchor,
                format!(
                    "Struct `{name_str}` has only `drop` ability (no copy/key/store). \
                     If this is a hot potato, remove `drop` to enforce consumption. \
                     If this is a witness, ensure it has no fields. \
                     See: https://blog.trailofbits.com/2025/09/10/how-sui-move-rethinks-flash-loan-security/"
                ),
            );
        }
    }

    Ok(())
}

// =========================================================================
// Ability Mistake Lints (type-based, zero FP)
// =========================================================================

pub(crate) fn lint_copyable_capability(
    out: &mut Vec<Diagnostic>,
    settings: &LintSettings,
    file_map: &MappedFiles,
    info: &TypingProgramInfo,
) -> Result<()> {
    use crate::type_classifier::{has_copy_ability, has_key_ability, has_store_ability};

    for (_mident, minfo) in info.modules.key_cloned_iter() {
        match minfo.target_kind {
            TargetKind::Source {
                is_root_package: true,
            } => {}
            _ => continue,
        }

        for (sname, sdef) in minfo.structs.key_cloned_iter() {
            let abilities = &sdef.abilities;
            let is_copyable_transferable = has_key_ability(abilities)
                && has_store_ability(abilities)
                && has_copy_ability(abilities);
            if !is_copyable_transferable {
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
                &COPYABLE_CAPABILITY,
                file,
                span,
                contents.as_ref(),
                anchor,
                format!(
                    "Struct `{name_str}` is `key + store + copy`. This creates a transferable, copyable authority/asset, \
                     which is almost always a severe security bug (privileges or value can be duplicated). Remove `copy`."
                ),
            );
        }
    }

    Ok(())
}

pub(crate) fn lint_droppable_capability(
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
            let is_droppable_transferable = has_key_ability(abilities)
                && has_store_ability(abilities)
                && has_drop_ability(abilities)
                && !has_copy_ability(abilities);
            if !is_droppable_transferable {
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
                &DROPPABLE_CAPABILITY,
                file,
                span,
                contents.as_ref(),
                anchor,
                format!(
                    "Struct `{name_str}` is `key + store + drop` (and not `copy`). This allows a transferable authority/asset to be silently discarded, \
                     which commonly breaks invariants (e.g., obligations can be bypassed). Remove `drop` or redesign the type."
                ),
            );
        }
    }

    Ok(())
}
