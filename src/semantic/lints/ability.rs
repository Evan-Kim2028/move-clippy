use crate::diagnostics::Diagnostic;
use crate::error::Result as ClippyResult;
use crate::lint::LintSettings;

use move_compiler::parser::ast::TargetKind;
use move_compiler::shared::Identifier;
use move_compiler::shared::{files::MappedFiles, program_info::TypingProgramInfo};

use super::super::util::{diag_from_loc, push_diag};
use super::super::{COPYABLE_CAPABILITY, DROPPABLE_CAPABILITY};

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
/// DEPRECATED: This lint has high false positive rate (~67%).
///
/// The pattern "only drop ability" matches many legitimate types:
/// - Comparator results (SMALLER/EQUAL/GREATER)
/// - Builder patterns (Verifier, Builder)
/// - Transfer policy rule markers (Rule)
/// - Rating/score structs
///
/// Use `droppable_flash_loan_receipt` instead, which detects the actual
/// security-critical pattern: functions returning Coin/Balance with a
/// droppable receipt struct.
#[allow(unused_variables)]
pub(crate) fn lint_droppable_hot_potato_v2(
    _out: &mut Vec<Diagnostic>,
    _settings: &LintSettings,
    _file_map: &MappedFiles,
    _info: &TypingProgramInfo,
) -> Result<()> {
    // DEPRECATED: No-op. See docstring for rationale.
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
