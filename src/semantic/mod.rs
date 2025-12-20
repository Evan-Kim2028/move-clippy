// Allow patterns that are intentional in semantic analysis
// - unused_variables: Move compiler iterators yield (key, value) pairs but we often only need value
// - unreachable_patterns: Match arms for exhaustiveness that may not be reached in practice
#![allow(unused_variables)]
#![allow(unreachable_patterns)]

//! Semantic lints that rely on Move compiler typing information.
//!
//! These lints are only available when `move-clippy` is built with the
//! `full` feature and run in `--mode full` against a Move package.

use crate::diagnostics::Diagnostic;
use crate::error::{Error, Result as ClippyResult};
use crate::lint::LintSettings;
use std::path::Path;

mod descriptors;
pub use descriptors::*;

#[cfg(feature = "full")]
mod lints;
#[cfg(feature = "full")]
mod util;

#[cfg(feature = "full")]
mod full {
    use super::lints::*;
    use super::util::{convert_compiler_diagnostic, position_from_byte_offset};
    use super::*;
    use crate::absint_lints;
    use crate::cross_module_lints;
    use crate::diagnostics::Span;
    use crate::instrument_block;
    use crate::level::LintLevel;
    use crate::lint::{LintDescriptor, RuleGroup};
    type Result<T> = ClippyResult<T>;
    use move_compiler::command_line::compiler::Visitor;
    use move_compiler::editions::Flavor;
    use move_compiler::parser::ast::TargetKind;
    use move_compiler::shared::{SaveFlag, SaveHook};
    use move_compiler::shared::{files::MappedFiles, program_info::TypingProgramInfo};
    use move_compiler::sui_mode::linters;
    use move_compiler::typing::ast as T;
    use move_package::BuildConfig;
    use move_package::compilation::build_plan::BuildPlan;

    fn descriptor_for_absint_diag(
        info: &move_compiler::diagnostics::codes::DiagnosticInfo,
    ) -> Option<&'static LintDescriptor> {
        // Only treat warnings emitted by our Phase II visitors as Phase II lints.
        //
        // AbsInt lints emit `custom("Lint", ..., category=50, code=...)` (see `absint_lints.rs`),
        // which renders as `warning[LintW5000X] ...`. The compiler also emits many unrelated
        // warnings with small numeric `code()` values (e.g., UnusedItem::Alias), so filtering
        // only on `code()` will misclassify those as Phase II lints.
        if info.external_prefix() != Some("Lint") || info.category() != 50 {
            return None;
        }

        crate::absint_lints::descriptor_for_diag_code(info.code())
    }

    /// Run all semantic lints against the package rooted at `package_path`.
    pub fn lint_package(
        package_path: &Path,
        settings: &LintSettings,
        preview: bool,
        experimental: bool,
    ) -> ClippyResult<Vec<Diagnostic>> {
        instrument_block!("semantic::lint_package", {
            let package_root = std::fs::canonicalize(package_path)?;
            let mut writer = Vec::<u8>::new();
            let mut build_config = BuildConfig::default();
            build_config.default_flavor = Some(Flavor::Sui);
            // Isolate build artifacts per invocation so tests (and parallel runs) don't race by
            // writing into the fixture/package directory.
            let install_dir = tempfile::tempdir()?;
            build_config.install_dir = Some(install_dir.path().to_path_buf());
            let resolved_graph =
                build_config.resolution_graph_for_package(&package_root, None, &mut writer)?;
            let build_plan = BuildPlan::create(&resolved_graph)?;

            let hook = SaveHook::new([SaveFlag::Typing, SaveFlag::TypingInfo]);

            // Get Phase II visitors (SimpleAbsInt-based lints)
            let phase2_visitors: Vec<Visitor> =
                absint_lints::create_visitors(preview, experimental)
                    .into_iter()
                    .map(Visitor::AbsIntVisitor)
                    .collect();

            // IMPORTANT: avoid `compile_no_exit` here; it prints compiler diagnostics to stdout,
            // which corrupts `--format json` output for ecosystem validation. Instead, capture
            // warnings and convert them into JSON diagnostics.
            let collected_phase2 = std::cell::RefCell::new(Vec::new());
            let deps = build_plan.compute_dependencies();
            let compiled =
                build_plan.compile_with_driver_and_deps(deps, &mut writer, |compiler| {
                    use move_compiler::diagnostics::report_diagnostics_to_buffer_with_env_color;

                    let (attr, filters) = linters::known_filters();
                    let compiler = compiler
                        .add_save_hook(&hook)
                        .add_custom_known_filters(attr, filters)
                        .add_visitors(phase2_visitors);

                    let (files, res) = compiler.build()?;
                    match res {
                        Ok((units, warnings)) => {
                            collected_phase2
                                .borrow_mut()
                                .push((files.clone(), warnings));
                            Ok((files, units))
                        }
                        Err(errors) => {
                            let rendered =
                                report_diagnostics_to_buffer_with_env_color(&files, errors);
                            Err(Error::semantic(format!(
                                "Move compilation failed while running Phase II visitors:\n{}",
                                String::from_utf8_lossy(&rendered)
                            ))
                            .into())
                        }
                    }
                })?;

            let typing_ast: T::Program = hook.take_typing_ast();
            let typing_info: std::sync::Arc<TypingProgramInfo> = hook.take_typing_info();
            let file_map: MappedFiles = compiled.file_map.clone();

            let mut out = Vec::new();

            // Phase II: convert AbsInt visitor diagnostics into our JSON diagnostics.
            for (_files, warnings) in collected_phase2.into_inner() {
                for compiler_diag in warnings.into_vec() {
                    let Some(descriptor) = descriptor_for_absint_diag(compiler_diag.info()) else {
                        continue;
                    };
                    if let Some(diag) =
                        convert_compiler_diagnostic(&compiler_diag, settings, &file_map, descriptor)
                    {
                        out.push(diag);
                    }
                }
            }

            // Type-based naming lints
            // Type-based security lints
            lint_entry_function_returns_value(&mut out, settings, &file_map, &typing_ast)?;
            lint_private_entry_function(&mut out, settings, &file_map, &typing_ast)?;
            lint_event_emit_type_sanity(&mut out, settings, &file_map, &typing_ast)?;
            lint_event_past_tense(&mut out, settings, &file_map, &typing_ast)?;
            lint_copyable_capability(&mut out, settings, &file_map, &typing_info)?;
            lint_droppable_capability(&mut out, settings, &file_map, &typing_info)?;
            lint_capability_antipatterns(&mut out, settings, &file_map, &typing_info, &typing_ast)?;
            lint_non_transferable_fungible_object(&mut out, settings, &file_map, &typing_info)?;
            lint_public_random_access_v2(&mut out, settings, &file_map, &typing_ast)?;
            lint_missing_witness_drop_v2(&mut out, settings, &file_map, &typing_info)?;
            lint_invalid_otw(&mut out, settings, &file_map, &typing_info)?;
            lint_witness_antipatterns(&mut out, settings, &file_map, &typing_info, &typing_ast)?;
            lint_stale_oracle_price_v2(&mut out, settings, &file_map, &typing_ast)?;
            // Phase 4 security lints (type-based, preview)
            if preview {
                lint_shared_capability_object(&mut out, settings, &file_map, &typing_ast)?;
                lint_capability_transfer_literal_address(
                    &mut out,
                    settings,
                    &file_map,
                    &typing_ast,
                )?;
                lint_mut_key_param_missing_authority(&mut out, settings, &file_map, &typing_ast)?;
                lint_unbounded_iteration_over_param_vector(
                    &mut out,
                    settings,
                    &file_map,
                    &typing_ast,
                )?;
            }
            // Phase 4 security lints (type-based, experimental)
            if experimental {
                lint_unchecked_division(&mut out, settings, &file_map, &typing_ast)?;
                lint_unused_return_value(&mut out, settings, &file_map, &typing_ast)?;
                lint_share_owned_authority(&mut out, settings, &file_map, &typing_ast)?;
                lint_droppable_hot_potato_v2(&mut out, settings, &file_map, &typing_info)?;
                lint_droppable_flash_loan_receipt(&mut out, settings, &file_map, &typing_ast)?;
                lint_receipt_missing_phantom_type(&mut out, settings, &file_map, &typing_ast)?;
                lint_copyable_fungible_type(
                    &mut out,
                    settings,
                    &file_map,
                    &typing_ast,
                    &typing_info,
                )?;
                lint_capability_transfer_v2(&mut out, settings, &file_map, &typing_ast)?;
                lint_generic_type_witness_unused(&mut out, settings, &file_map, &typing_ast)?;
            }
            // Note: phantom_capability is implemented in absint_lints.rs (CFG-aware)

            // Phase III: Cross-module analysis lints (type-based)
            if experimental {
                lint_cross_module_lints(&mut out, settings, &file_map, &typing_ast, &typing_info)?;
            }

            // Sui-delegated lints (type-based, production)
            lint_sui_visitors(&mut out, settings, &build_plan, &package_root)?;

            // Filter Preview-group diagnostics when preview is disabled
            if !preview {
                out.retain(|d| d.lint.group != RuleGroup::Preview);
            }

            // Filter Experimental-group diagnostics when experimental is disabled
            if !experimental {
                out.retain(|d| d.lint.group != RuleGroup::Experimental);
            }

            append_unfulfilled_expectations(&mut out, &typing_ast, &file_map);

            Ok(out)
        })
    }

    /// Run cross-module analysis lints (Phase III)
    fn lint_cross_module_lints(
        out: &mut Vec<Diagnostic>,
        settings: &LintSettings,
        file_map: &MappedFiles,
        prog: &T::Program,
        info: &TypingProgramInfo,
    ) -> Result<()> {
        // Run transitive capability leak detection
        let cap_leak_diags = cross_module_lints::lint_transitive_capability_leak(prog, info);
        for compiler_diag in cap_leak_diags {
            if let Some(diag) = convert_compiler_diagnostic(
                &compiler_diag,
                settings,
                file_map,
                &cross_module_lints::TRANSITIVE_CAPABILITY_LEAK,
            ) {
                out.push(diag);
            }
        }

        // Run flashloan repayment analysis
        let flashloan_diags = cross_module_lints::lint_flashloan_without_repay(prog, info);
        for compiler_diag in flashloan_diags {
            if let Some(diag) = convert_compiler_diagnostic(
                &compiler_diag,
                settings,
                file_map,
                &cross_module_lints::FLASHLOAN_WITHOUT_REPAY,
            ) {
                out.push(diag);
            }
        }

        // NOTE: lint_price_manipulation_window removed - used name-based heuristics

        Ok(())
    }

    fn append_unfulfilled_expectations(
        out: &mut Vec<Diagnostic>,
        prog: &T::Program,
        file_map: &MappedFiles,
    ) {
        use std::collections::{BTreeMap, BTreeSet};

        let mut fired: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for d in out.iter() {
            let Some(file) = d.file.as_deref() else {
                continue;
            };
            let entry = fired.entry(file.to_string()).or_default();
            entry.insert(d.lint.name.to_string());
            entry.insert(d.lint.category.as_str().to_string());
        }

        let mut module_expected: BTreeMap<String, (std::sync::Arc<str>, BTreeSet<String>)> =
            BTreeMap::new();
        let mut item_expected: BTreeMap<String, BTreeMap<usize, BTreeSet<String>>> =
            BTreeMap::new();

        for (_mident, mdef) in prog.modules.key_cloned_iter() {
            match mdef.target_kind {
                TargetKind::Source {
                    is_root_package: true,
                } => {}
                _ => continue,
            }

            // Collect module-level expectations once per file.
            let loc = mdef.loc;
            let Some((fname, contents)) = file_map.get(&loc.file_hash()) else {
                continue;
            };
            let file = fname.as_str().to_string();
            module_expected.entry(file.clone()).or_insert_with(|| {
                let scope = crate::annotations::module_scope(contents.as_ref());
                let expected: BTreeSet<String> = scope
                    .unfired_expectations()
                    .cloned()
                    .collect::<BTreeSet<_>>();
                (contents.clone(), expected)
            });

            // Collect item-level expectations for each function anchor.
            for (_fname, fdef) in mdef.functions.key_cloned_iter() {
                let anchor = fdef.loc.start() as usize;
                let scope = crate::annotations::item_scope(contents.as_ref(), anchor);
                let expected: BTreeSet<String> = scope
                    .unfired_expectations()
                    .cloned()
                    .collect::<BTreeSet<_>>();
                if expected.is_empty() {
                    continue;
                }
                item_expected
                    .entry(file.clone())
                    .or_default()
                    .entry(anchor)
                    .or_default()
                    .extend(expected);
            }
        }

        // Module-level unfulfilled expectations: require any matching lint or category in file.
        for (file, (contents, expected)) in module_expected {
            let fired_set = fired.get(&file);
            for name in expected {
                let fired_any = fired_set.is_some_and(|s| s.contains(&name));
                if fired_any {
                    continue;
                }

                out.push(Diagnostic {
                    lint: &crate::lint::UNFULFILLED_EXPECTATION,
                    level: LintLevel::Error,
                    file: Some(file.clone()),
                    span: Span {
                        start: crate::diagnostics::Position { row: 1, column: 1 },
                        end: crate::diagnostics::Position { row: 1, column: 1 },
                    },
                    message: format!(
                        "Expected `lint::{}` to produce a diagnostic in this file, but it did not",
                        name
                    ),
                    help: Some(
                        "Remove the `#![expect(...)]` directive or adjust the code/lint so it triggers."
                            .to_string(),
                    ),
                    suggestion: None,
                });
            }

            // Item-level unfulfilled expectations: approximate by file-level fired set.
            if let Some(anchors) = item_expected.get(&file) {
                let fired_set = fired.get(&file);
                for (&anchor, names) in anchors {
                    for name in names {
                        let fired_any = fired_set.is_some_and(|s| s.contains(name));
                        if fired_any {
                            continue;
                        }

                        let pos = position_from_byte_offset(contents.as_ref(), anchor);
                        out.push(Diagnostic {
                            lint: &crate::lint::UNFULFILLED_EXPECTATION,
                            level: LintLevel::Error,
                            file: Some(file.clone()),
                            span: Span { start: pos, end: pos },
                            message: format!(
                                "Expected `lint::{}` to produce a diagnostic in this scope, but it did not",
                                name
                            ),
                            help: Some(
                                "Remove the `#[expect(...)]` directive or adjust the code/lint so it triggers."
                                    .to_string(),
                            ),
                            suggestion: None,
                        });
                    }
                }
            }
        }
    }
}

#[cfg(feature = "full")]
pub use full::lint_package;

#[cfg(not(feature = "full"))]
pub fn lint_package(
    _package_path: &Path,
    _settings: &LintSettings,
    _preview: bool,
    _experimental: bool,
) -> ClippyResult<Vec<Diagnostic>> {
    Err(Error::semantic(
        "full mode requires building with --features full",
    ))
}
