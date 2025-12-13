use crate::diagnostics::Diagnostic;
use crate::lint::{LintCategory, LintDescriptor, LintSettings};
use crate::rules::modernization::{PUBLIC_MUT_TX_CONTEXT, UNNECESSARY_PUBLIC_ENTRY};
use anyhow::Result;
use std::path::Path;

/// Semantic lints that rely on Move compiler typing information.
///
/// These lints are only available when `move-clippy` is built with the
/// `full` feature and run in `--mode full` against a Move package.
pub static CAPABILITY_NAMING: LintDescriptor = LintDescriptor {
    name: "capability_naming",
    category: LintCategory::Naming,
    description: "Capability structs (key+store) should be suffixed with _cap (semantic, requires --mode full)",
};

pub static EVENT_NAMING: LintDescriptor = LintDescriptor {
    name: "event_naming",
    category: LintCategory::Naming,
    description: "Event structs (copy+drop) should be named <past_tense>_<noun>_event (semantic, requires --mode full)",
};

pub static GETTER_NAMING: LintDescriptor = LintDescriptor {
    name: "getter_naming",
    category: LintCategory::Naming,
    description: "Avoid get_ prefix for simple field getters taking &Self (semantic, requires --mode full)",
};

pub static SHARE_OWNED: LintDescriptor = LintDescriptor {
    name: "share_owned",
    category: LintCategory::Suspicious,
    description: "Possible owned object share (Sui lint, requires --mode full)",
};

pub static SELF_TRANSFER: LintDescriptor = LintDescriptor {
    name: "self_transfer",
    category: LintCategory::Suspicious,
    description: "Transferring or sharing objects back to the sender (Sui lint, requires --mode full)",
};

pub static CUSTOM_STATE_CHANGE: LintDescriptor = LintDescriptor {
    name: "custom_state_change",
    category: LintCategory::Suspicious,
    description: "Custom transfer/share/freeze functions must call private variants (Sui lint, requires --mode full)",
};

pub static COIN_FIELD: LintDescriptor = LintDescriptor {
    name: "coin_field",
    category: LintCategory::Suspicious,
    description: "Avoid storing sui::coin::Coin fields inside structs (Sui lint, requires --mode full)",
};

pub static FREEZE_WRAPPED: LintDescriptor = LintDescriptor {
    name: "freeze_wrapped",
    category: LintCategory::Suspicious,
    description: "Do not wrap shared objects before freezing (Sui lint, requires --mode full)",
};

pub static COLLECTION_EQUALITY: LintDescriptor = LintDescriptor {
    name: "collection_equality",
    category: LintCategory::Suspicious,
    description: "Avoid equality checks over bags/tables/collections (Sui lint, requires --mode full)",
};

pub static PUBLIC_RANDOM: LintDescriptor = LintDescriptor {
    name: "public_random",
    category: LintCategory::Suspicious,
    description: "Random state should remain private and uncopyable (Sui lint, requires --mode full)",
};

pub static MISSING_KEY: LintDescriptor = LintDescriptor {
    name: "missing_key",
    category: LintCategory::Suspicious,
    description: "Warn when shared/transferred structs lack the key ability (Sui lint, requires --mode full)",
};

pub static FREEZING_CAPABILITY: LintDescriptor = LintDescriptor {
    name: "freezing_capability",
    category: LintCategory::Suspicious,
    description: "Avoid storing freeze capabilities (Sui lint, requires --mode full)",
};

static DESCRIPTORS: &[&LintDescriptor] = &[
    &CAPABILITY_NAMING,
    &EVENT_NAMING,
    &GETTER_NAMING,
    &SHARE_OWNED,
    &SELF_TRANSFER,
    &CUSTOM_STATE_CHANGE,
    &COIN_FIELD,
    &FREEZE_WRAPPED,
    &COLLECTION_EQUALITY,
    &PUBLIC_RANDOM,
    &MISSING_KEY,
    &FREEZING_CAPABILITY,
];

/// Return descriptors for all semantic lints.
pub fn descriptors() -> &'static [&'static LintDescriptor] {
    &DESCRIPTORS
}

/// Look up a semantic lint descriptor by name.
pub fn find_descriptor(name: &str) -> Option<&'static LintDescriptor> {
    descriptors().iter().copied().find(|d| d.name == name)
}

#[cfg(feature = "full")]
mod full {
    use super::*;
    use crate::diagnostics::Span;
    use crate::level::LintLevel;
    use crate::suppression;
    use move_compiler::editions::Flavor;
    use move_compiler::parser::ast::{Ability_, TargetKind};
    use move_compiler::shared::{Identifier, files::MappedFiles, program_info::TypingProgramInfo};
    use move_compiler::shared::{SaveFlag, SaveHook};
    use move_compiler::sui_mode::linters;
    use move_compiler::{naming::ast as N, typing::ast as T};
    use move_ir_types::location::Loc;
    use move_package::BuildConfig;
    use move_package::compilation::build_plan::BuildPlan;

    /// Run all semantic lints against the package rooted at `package_path`.
    pub fn lint_package(package_path: &Path, settings: &LintSettings) -> Result<Vec<Diagnostic>> {
        let package_root = std::fs::canonicalize(package_path)?;
        let mut writer = Vec::<u8>::new();
        let mut build_config = BuildConfig::default();
        build_config.default_flavor = Some(Flavor::Sui);
        let resolved_graph =
            build_config.resolution_graph_for_package(&package_root, None, &mut writer)?;
        let build_plan = BuildPlan::create(&resolved_graph)?;

        let hook = SaveHook::new([SaveFlag::Typing, SaveFlag::TypingInfo]);
        let compiled = build_plan.compile_no_exit(&mut writer, |compiler| {
            let (attr, filters) = linters::known_filters();
            compiler
                .add_save_hook(&hook)
                .add_custom_known_filters(attr, filters)
        })?;

        let typing_ast: T::Program = hook.take_typing_ast();
        let typing_info: std::sync::Arc<TypingProgramInfo> = hook.take_typing_info();
        let file_map: MappedFiles = compiled.file_map.clone();

        let mut out = Vec::new();
        lint_capability_naming(&mut out, settings, &file_map, &typing_info)?;
        lint_event_naming(&mut out, settings, &file_map, &typing_info)?;
        lint_getter_naming(&mut out, settings, &file_map, &typing_ast)?;
        lint_sui_visitors(&mut out, settings, &build_plan, &package_root)?;
        Ok(out)
    }

    fn diag_from_loc(
        file_map: &MappedFiles,
        loc: &Loc,
    ) -> Option<(String, Span, std::sync::Arc<str>)> {
        let (fname, contents) = file_map.get(&loc.file_hash())?;
        let p = file_map.position_opt(loc)?;

        let file = fname.as_str().to_string();
        let span = Span {
            start: crate::diagnostics::Position {
                row: p.start.line_offset() + 1,
                column: p.start.column_offset() + 1,
            },
            end: crate::diagnostics::Position {
                row: p.end.line_offset() + 1,
                column: p.end.column_offset() + 1,
            },
        };

        Some((file, span, contents))
    }

    fn push_diag(
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        lint: &'static LintDescriptor,
        file: String,
        span: Span,
        source: &str,
        anchor_start: usize,
        message: String,
    ) {
        let level = settings.level_for(lint.name);
        if level == LintLevel::Allow {
            return;
        }
        if suppression::is_suppressed_at(source, anchor_start, lint.name) {
            return;
        }

        out.push(Diagnostic {
            lint,
            level,
            file: Some(file),
            span,
            message,
            help: None,
            suggestion: None,
        });
    }

    fn lint_capability_naming(
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        info: &TypingProgramInfo,
    ) -> Result<()> {
        for (_mident, minfo) in info.modules.key_cloned_iter() {
            match minfo.target_kind {
                TargetKind::Source {
                    is_root_package: true,
                } => {}
                _ => continue,
            }

            for (sname, sdef) in minfo.structs.key_cloned_iter() {
                let abilities = &sdef.abilities;
                let is_cap = abilities.has_ability_(Ability_::Key)
                    && abilities.has_ability_(Ability_::Store)
                    && !abilities.has_ability_(Ability_::Copy)
                    && !abilities.has_ability_(Ability_::Drop);
                if !is_cap {
                    continue;
                }

                let sym = sname.value();
                let name_str = sym.as_str();
                if name_str.ends_with("_cap") {
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
                    &CAPABILITY_NAMING,
                    file,
                    span,
                    contents.as_ref(),
                    anchor,
                    format!("Capability struct should be suffixed with `_cap`: `{name_str}_cap`"),
                );
            }
        }

        Ok(())
    }

    fn lint_event_naming(
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        info: &TypingProgramInfo,
    ) -> Result<()> {
        for (_mident, minfo) in info.modules.key_cloned_iter() {
            match minfo.target_kind {
                TargetKind::Source {
                    is_root_package: true,
                } => {}
                _ => continue,
            }

            for (sname, sdef) in minfo.structs.key_cloned_iter() {
                let abilities = &sdef.abilities;
                let is_event = abilities.has_ability_(Ability_::Copy)
                    && abilities.has_ability_(Ability_::Drop)
                    && !abilities.has_ability_(Ability_::Key)
                    && !abilities.has_ability_(Ability_::Store);
                if !is_event {
                    continue;
                }

                let sym = sname.value();
                let name_str = sym.as_str();
                let loc = sname.loc();
                let Some((file, span, contents)) = diag_from_loc(file_map, &loc) else {
                    continue;
                };
                let anchor = loc.start() as usize;

                if !name_str.ends_with("_event") {
                    push_diag(
                        out,
                        settings,
                        &EVENT_NAMING,
                        file,
                        span,
                        contents.as_ref(),
                        anchor,
                        format!("Event struct should end with `_event`: `{name_str}_event`"),
                    );
                    continue;
                }

                let first = name_str.split('_').next().unwrap_or("");
                if !first.ends_with("ed") {
                    push_diag(
                        out,
                        settings,
                        &EVENT_NAMING,
                        file,
                        span,
                        contents.as_ref(),
                        anchor,
                        "Event struct should use a past-tense verb prefix (e.g. `transferred_..._event`)".to_string(),
                    );
                }
            }
        }

        Ok(())
    }

    fn lint_getter_naming(
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        prog: &T::Program,
    ) -> Result<()> {
        for (mident, mdef) in prog.modules.key_cloned_iter() {
            match mdef.target_kind {
                TargetKind::Source {
                    is_root_package: true,
                } => {}
                _ => continue,
            }

            for (fname, fdef) in mdef.functions.key_cloned_iter() {
                let sym = fname.value();
                let name = sym.as_str();
                if !name.starts_with("get_") {
                    continue;
                }

                let Some((_, self_var, self_ty)) = fdef.signature.parameters.first() else {
                    continue;
                };

                if !is_ref_to_module_type(self_ty, &mident) {
                    continue;
                }

                let T::FunctionBody_::Defined((_use_funs, seq_items)) = &fdef.body.value else {
                    continue;
                };

                if seq_items.len() != 1 {
                    continue;
                }

                let Some(T::SequenceItem_::Seq(exp)) = seq_items.front().map(|s| &s.value) else {
                    continue;
                };
                if !is_simple_self_field_get(exp, self_var) {
                    continue;
                }

                let loc = fname.loc();
                let Some((file, span, contents)) = diag_from_loc(file_map, &loc) else {
                    continue;
                };
                let anchor = fdef.loc.start() as usize;

                let suggested = &name[4..];
                push_diag(
                    out,
                    settings,
                    &GETTER_NAMING,
                    file,
                    span,
                    contents.as_ref(),
                    anchor,
                    format!("Prefer `{suggested}` over `{name}` for simple getters"),
                );
            }
        }

        Ok(())
    }

    fn lint_sui_visitors(
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
                    anyhow::bail!(
                        "Move compilation failed while running Sui lints:\n{}",
                        String::from_utf8_lossy(&rendered)
                    );
                }
            }
        })?;

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

                let Some((file, span, contents)) = diag_from_loc(&file_map, &diag.primary_loc())
                else {
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
            x if x == PreferMutableTxContext as u8 => Some(&PUBLIC_MUT_TX_CONTEXT),
            x if x == UnnecessaryPublicEntry as u8 => Some(&UNNECESSARY_PUBLIC_ENTRY),
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

    fn is_self_local(base: &T::Exp, self_var: &N::Var) -> bool {
        match &base.exp.value {
            T::UnannotatedExp_::BorrowLocal(_mut_, v) => v.value.id == self_var.value.id,
            T::UnannotatedExp_::TempBorrow(_mut_, inner) => is_self_local(inner, self_var),
            T::UnannotatedExp_::Copy { var, .. } => var.value.id == self_var.value.id,
            T::UnannotatedExp_::Move { var, .. } => var.value.id == self_var.value.id,
            T::UnannotatedExp_::Use(v) => v.value.id == self_var.value.id,
            _ => false,
        }
    }
}

#[cfg(feature = "full")]
pub use full::lint_package;

#[cfg(not(feature = "full"))]
pub fn lint_package(_package_path: &Path, _settings: &LintSettings) -> Result<Vec<Diagnostic>> {
    anyhow::bail!("full mode requires building with --features full")
}
