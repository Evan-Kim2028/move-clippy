use crate::diagnostics::Diagnostic;
use crate::error::{Error, Result as ClippyResult};
use crate::level::LintLevel;
use crate::lint::{LintDescriptor, LintSettings};
// REMOVED: PUBLIC_MUT_TX_CONTEXT, UNNECESSARY_PUBLIC_ENTRY - now handled directly by Sui compiler
use crate::suppression;
use move_compiler::naming::ast as N;
use move_compiler::typing::ast as T;
use move_package::compilation::build_plan::BuildPlan;
use std::path::Path;

use super::super::util::diag_from_loc;
use super::super::{
    COIN_FIELD, COLLECTION_EQUALITY, CUSTOM_STATE_CHANGE, FREEZE_WRAPPED, FREEZING_CAPABILITY,
    MISSING_KEY, PUBLIC_RANDOM, SELF_TRANSFER, SHARE_OWNED,
};

type Result<T> = ClippyResult<T>;

// =========================================================================
// Sui-delegated Lints
// =========================================================================

pub(crate) fn lint_sui_visitors(
    out: &mut Vec<Diagnostic>,
    settings: &LintSettings,
    build_plan: &BuildPlan,
    package_root: &Path,
) -> Result<()> {
    use move_compiler::diagnostics::report_diagnostics_to_buffer_with_env_color;
    use move_compiler::linters::{LintLevel as CompilerLintLevel, LinterDiagnosticCategory};
    use move_compiler::sui_mode::linters;

    let mut writer = Vec::new();
    let deps = build_plan.compute_dependencies();
    let collected = std::cell::RefCell::new(Vec::new());

    build_plan.compile_with_driver_and_deps(deps, &mut writer, |compiler| {
        let (attr, filters) = linters::known_filters();
        let compiler = compiler
            .add_custom_known_filters(attr, filters)
            .add_visitors(linters::linter_visitors(CompilerLintLevel::All));
        let (files, res) = compiler.build()?;
        match res {
            Ok((units, warnings)) => {
                collected.borrow_mut().push((files.clone(), warnings));
                Ok((files, units))
            }
            Err(errors) => {
                let rendered = report_diagnostics_to_buffer_with_env_color(&files, errors);
                Err(Error::semantic(format!(
                    "Move compilation failed while running Sui lints:\n{}",
                    String::from_utf8_lossy(&rendered)
                ))
                .into())
            }
        }
    })?;

    let mut seen: std::collections::BTreeSet<(
        &'static str,
        String,
        usize,
        usize,
        usize,
        usize,
        String,
    )> = std::collections::BTreeSet::new();

    for (file_map, warnings) in collected.into_inner() {
        for diag in warnings.into_vec() {
            if diag.info().category() != LinterDiagnosticCategory::Sui as u8 {
                continue;
            }
            let Some(descriptor) = descriptor_for_sui_code(diag.info().code()) else {
                continue;
            };
            let level = settings.level_for(descriptor.name);
            if level == LintLevel::Allow {
                continue;
            }

            let Some((file, span, contents)) = diag_from_loc(&file_map, &diag.primary_loc()) else {
                continue;
            };

            if !Path::new(&file).starts_with(package_root) {
                continue;
            }

            let anchor = diag.primary_loc().start() as usize;
            if suppression::is_suppressed_at(contents.as_ref(), anchor, descriptor.name) {
                continue;
            }

            let message = compose_sui_message(&diag);
            let key = (
                descriptor.name,
                file.clone(),
                span.start.row,
                span.start.column,
                span.end.row,
                span.end.column,
                message.clone(),
            );
            if !seen.insert(key) {
                continue;
            }
            out.push(Diagnostic {
                lint: descriptor,
                level,
                file: Some(file),
                span,
                message,
                help: None,
                suggestion: None,
            });
        }
    }

    Ok(())
}

fn descriptor_for_sui_code(code: u8) -> Option<&'static LintDescriptor> {
    use move_compiler::sui_mode::linters::LinterDiagnosticCode::*;

    match code {
        x if x == ShareOwned as u8 => Some(&SHARE_OWNED),
        x if x == SelfTransfer as u8 => Some(&SELF_TRANSFER),
        x if x == CustomStateChange as u8 => Some(&CUSTOM_STATE_CHANGE),
        x if x == CoinField as u8 => Some(&COIN_FIELD),
        x if x == FreezeWrapped as u8 => Some(&FREEZE_WRAPPED),
        x if x == CollectionEquality as u8 => Some(&COLLECTION_EQUALITY),
        x if x == PublicRandom as u8 => Some(&PUBLIC_RANDOM),
        x if x == MissingKey as u8 => Some(&MISSING_KEY),
        x if x == FreezingCapability as u8 => Some(&FREEZING_CAPABILITY),
        // PreferMutableTxContext and UnnecessaryPublicEntry removed - handled by Sui compiler
        _ => None,
    }
}

fn compose_sui_message(diag: &move_compiler::diagnostics::Diagnostic) -> String {
    let base = diag.info().message().to_string();
    let label = diag.primary_msg().trim();
    if label.is_empty() || base.contains(label) {
        base
    } else {
        format!("{base}: {label}")
    }
}

#[allow(dead_code)]
fn is_ref_to_module_type(
    ty: &N::Type,
    module: &move_compiler::expansion::ast::ModuleIdent,
) -> bool {
    let N::Type_::Ref(_is_mut, inner) = &ty.value else {
        return false;
    };

    let N::Type_::Apply(_abilities, tname, _tys) = &inner.value else {
        return false;
    };

    match &tname.value {
        N::TypeName_::ModuleType(m, _dtype) => m == module,
        _ => false,
    }
}

#[allow(dead_code)]
fn is_simple_self_field_get(exp: &T::Exp, self_var: &N::Var) -> bool {
    match &exp.exp.value {
        T::UnannotatedExp_::Borrow(_mut_, base, _field) => is_self_local(base, self_var),
        T::UnannotatedExp_::Dereference(inner) => match &inner.exp.value {
            T::UnannotatedExp_::Borrow(_mut_, base, _field) => is_self_local(base, self_var),
            _ => false,
        },
        _ => false,
    }
}

#[allow(dead_code)]
fn is_self_local(base: &T::Exp, self_var: &N::Var) -> bool {
    match &base.exp.value {
        T::UnannotatedExp_::BorrowLocal(_mut_, v) => v.value.id == self_var.value.id,
        T::UnannotatedExp_::TempBorrow(_, inner) => is_self_local(inner, self_var),
        T::UnannotatedExp_::Copy { var, .. } => var.value.id == self_var.value.id,
        T::UnannotatedExp_::Move { var, .. } => var.value.id == self_var.value.id,
        T::UnannotatedExp_::Use(v) => v.value.id == self_var.value.id,
        _ => false,
    }
}
