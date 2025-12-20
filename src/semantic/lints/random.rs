use crate::diagnostics::Diagnostic;
use crate::error::Result as ClippyResult;
use crate::lint::LintSettings;
use move_compiler::naming::ast as N;
use move_compiler::parser::ast::TargetKind;
use move_compiler::shared::Identifier;
use move_compiler::shared::files::MappedFiles;
use move_compiler::typing::ast as T;

use super::super::PUBLIC_RANDOM_ACCESS_V2;
use super::super::util::{diag_from_loc, push_diag};

type Result<T> = ClippyResult<T>;

// =========================================================================
// Public Random Access V2 Lint (type-based)
// =========================================================================

/// Lint for public (non-entry) functions that expose sui::random::Random objects.
///
/// Random objects should only be accessible in entry functions to prevent
/// front-running attacks where validators can see random values before
/// including transactions.
pub(crate) fn lint_public_random_access_v2(
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
            // Only check public non-entry functions
            // Entry functions are allowed to take Random
            if fdef.entry.is_some() {
                continue;
            }

            // Check if function is public
            let is_public = matches!(
                fdef.visibility,
                move_compiler::expansion::ast::Visibility::Public(_)
            );

            if !is_public {
                continue;
            }

            // Check if any parameter is sui::random::Random
            for (_, _, param_ty) in fdef.signature.parameters.iter() {
                if is_random_type(&param_ty.value) {
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
                        &PUBLIC_RANDOM_ACCESS_V2,
                        file,
                        span,
                        contents.as_ref(),
                        anchor,
                        format!(
                            "Public function `{fn_name}` exposes `sui::random::Random` object. \
                             This enables front-running attacks where validators can see random \
                             values before including transactions. Use `entry` visibility instead, \
                             or make the function private/package-internal."
                        ),
                    );
                    break; // Only report once per function
                }
            }
        }
    }

    Ok(())
}

/// Check if a type is sui::random::Random (including references).
fn is_random_type(ty: &N::Type_) -> bool {
    match ty {
        N::Type_::Apply(_, type_name, _) => {
            if let N::TypeName_::ModuleType(mident, struct_name) = &type_name.value {
                let addr = &mident.value.address;
                let module_sym = mident.value.module.value();
                let struct_sym = struct_name.value();

                // Check for 0x2::random::Random
                // The address should be the Sui framework address (0x2)
                let is_sui_addr = match addr {
                    move_compiler::expansion::ast::Address::Numerical {
                        value: addr_value, ..
                    } => {
                        // Check if address bytes end with 0x02
                        let bytes = addr_value.value.into_bytes();
                        bytes.iter().take(31).all(|&b| b == 0) && bytes[31] == 2
                    }
                    move_compiler::expansion::ast::Address::NamedUnassigned(name) => {
                        name.value.as_str() == "sui" || name.value.as_str() == "0x2"
                    }
                };

                is_sui_addr && module_sym.as_str() == "random" && struct_sym.as_str() == "Random"
            } else {
                false
            }
        }
        N::Type_::Ref(_, inner) => is_random_type(&inner.value),
        _ => false,
    }
}
