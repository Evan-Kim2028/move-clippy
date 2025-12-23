use crate::diagnostics::Diagnostic;
use crate::error::Result as ClippyResult;
use crate::lint::LintSettings;
use move_compiler::parser::ast::TargetKind;
use move_compiler::shared::Identifier;
use move_compiler::shared::files::MappedFiles;
use move_compiler::typing::ast as T;

use super::super::util::{diag_from_loc, push_diag};
use super::super::{UNCHECKED_DIVISION, UNUSED_RETURN_VALUE};

type Result<T> = ClippyResult<T>;

/// Lint for division operations without zero-divisor checks.
///
/// Division by zero will abort the transaction. This lint detects divisions
/// where the divisor hasn't been validated as non-zero.
#[allow(dead_code)]
pub(crate) fn lint_unchecked_division(
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

            // Track variables that have been validated as non-zero
            let mut validated_vars: std::collections::HashSet<u16> =
                std::collections::HashSet::new();

            for item in seq_items.iter() {
                check_division_in_seq_item(
                    item,
                    &mut validated_vars,
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

/// Check for division operations in a sequence item.
fn check_division_in_seq_item(
    item: &T::SequenceItem,
    validated_vars: &mut std::collections::HashSet<u16>,
    out: &mut Vec<Diagnostic>,
    settings: &LintSettings,
    file_map: &MappedFiles,
    func_name: &str,
) {
    match &item.value {
        T::SequenceItem_::Seq(exp) => {
            // Check for assert statements that validate non-zero
            check_for_nonzero_assertion(exp, validated_vars);
            check_division_in_exp(exp, validated_vars, out, settings, file_map, func_name);
        }
        T::SequenceItem_::Bind(_, _, exp) => {
            check_division_in_exp(exp, validated_vars, out, settings, file_map, func_name);
        }
        _ => {}
    }
}

/// Check if an expression is an assertion that validates a variable is non-zero.
fn check_for_nonzero_assertion(exp: &T::Exp, validated_vars: &mut std::collections::HashSet<u16>) {
    // Look for assert!(var != 0, ...) or assert!(var > 0, ...)
    if let T::UnannotatedExp_::Builtin(builtin, args) = &exp.exp.value {
        let builtin_str = format!("{:?}", builtin);
        if builtin_str.contains("Assert") {
            // args is Box<Exp> - extract first argument from ExpList if present
            let first_arg = if let T::UnannotatedExp_::ExpList(items) = &args.exp.value {
                items.first().and_then(|item| match item {
                    T::ExpListItem::Single(e, _) => Some(e),
                    _ => None,
                })
            } else {
                Some(args.as_ref())
            };

            if let Some(first_arg) = first_arg
                && let T::UnannotatedExp_::BinopExp(left, op, _, right) = &first_arg.exp.value
            {
                let op_str = format!("{:?}", op);
                // Check for != 0 or > 0
                if op_str.contains("Neq") || op_str.contains("Gt") {
                    // Check if comparing with 0
                    if is_zero_value(right)
                        && let Some(var_id) = extract_var_id(left)
                    {
                        validated_vars.insert(var_id);
                    }
                    if is_zero_value(left)
                        && let Some(var_id) = extract_var_id(right)
                    {
                        validated_vars.insert(var_id);
                    }
                }
            }
        }
    }
}

/// Check if an expression is a zero value.
fn is_zero_value(exp: &T::Exp) -> bool {
    if let T::UnannotatedExp_::Value(val) = &exp.exp.value {
        let val_str = format!("{:?}", val);
        val_str.contains("0") && !val_str.contains("0x")
    } else {
        false
    }
}

/// Extract variable ID from an expression if it's a simple variable reference.
fn extract_var_id(exp: &T::Exp) -> Option<u16> {
    match &exp.exp.value {
        T::UnannotatedExp_::Use(v) => Some(v.value.id),
        T::UnannotatedExp_::Copy { var, .. } => Some(var.value.id),
        T::UnannotatedExp_::Move { var, .. } => Some(var.value.id),
        _ => None,
    }
}

/// Check for division operations in an expression.
fn check_division_in_exp(
    exp: &T::Exp,
    validated_vars: &std::collections::HashSet<u16>,
    out: &mut Vec<Diagnostic>,
    settings: &LintSettings,
    file_map: &MappedFiles,
    func_name: &str,
) {
    match &exp.exp.value {
        T::UnannotatedExp_::BinopExp(left, op, _, right) => {
            let op_str = format!("{:?}", op);
            if op_str.contains("Div") || op_str.contains("Mod") {
                // Check if the divisor (right) is a validated variable
                let divisor_validated = if let Some(var_id) = extract_var_id(right) {
                    validated_vars.contains(&var_id)
                } else {
                    // If it's a constant or complex expression, assume it might be safe
                    // (conservative approach to reduce FPs)
                    matches!(
                        &right.exp.value,
                        T::UnannotatedExp_::Value(_) | T::UnannotatedExp_::Constant(_, _)
                    )
                };

                if !divisor_validated {
                    let loc = exp.exp.loc;
                    let Some((file, span, contents)) = diag_from_loc(file_map, &loc) else {
                        return;
                    };
                    let anchor = loc.start() as usize;

                    push_diag(
                        out,
                        settings,
                        &UNCHECKED_DIVISION,
                        file,
                        span,
                        contents.as_ref(),
                        anchor,
                        format!(
                            "Division in function `{func_name}` may divide by zero. \
                             Consider adding `assert!(divisor != 0, E_DIVISION_BY_ZERO)` before this operation."
                        ),
                    );
                }
            }

            // Recurse
            check_division_in_exp(left, validated_vars, out, settings, file_map, func_name);
            check_division_in_exp(right, validated_vars, out, settings, file_map, func_name);
        }
        T::UnannotatedExp_::ModuleCall(call) => {
            check_division_in_exp(
                &call.arguments,
                validated_vars,
                out,
                settings,
                file_map,
                func_name,
            );
        }
        T::UnannotatedExp_::Block((_, seq)) => {
            let mut local_validated = validated_vars.clone();
            for item in seq.iter() {
                check_division_in_seq_item(
                    item,
                    &mut local_validated,
                    out,
                    settings,
                    file_map,
                    func_name,
                );
            }
        }
        T::UnannotatedExp_::IfElse(cond, if_body, else_body) => {
            check_division_in_exp(cond, validated_vars, out, settings, file_map, func_name);
            check_division_in_exp(if_body, validated_vars, out, settings, file_map, func_name);
            if let Some(else_e) = else_body {
                check_division_in_exp(else_e, validated_vars, out, settings, file_map, func_name);
            }
        }
        T::UnannotatedExp_::While(_, cond, body) => {
            check_division_in_exp(cond, validated_vars, out, settings, file_map, func_name);
            check_division_in_exp(body, validated_vars, out, settings, file_map, func_name);
        }
        T::UnannotatedExp_::Loop { body, .. } => {
            check_division_in_exp(body, validated_vars, out, settings, file_map, func_name);
        }
        _ => {}
    }
}

// =========================================================================
// Unused Return Value Lint
// =========================================================================

/// Lint for important return values that are ignored.
///
/// This lint detects when function calls that return non-unit values
/// have their return values discarded.
pub(crate) fn lint_unused_return_value(
    out: &mut Vec<Diagnostic>,
    settings: &LintSettings,
    file_map: &MappedFiles,
    prog: &T::Program,
) -> Result<()> {
    // TODO(infra): Move to crate::framework_catalog and match on fully-qualified IDs.
    // Functions whose return values should not be ignored
    const IMPORTANT_FUNCTIONS: &[(&str, &str)] = &[
        ("coin", "split"),
        ("coin", "take"),
        ("balance", "split"),
        ("balance", "withdraw_all"),
        ("option", "extract"),
        ("option", "destroy_some"),
        ("vector", "pop_back"),
        ("table", "remove"),
        ("bag", "remove"),
    ];

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
                check_unused_return_in_seq_item(
                    item,
                    IMPORTANT_FUNCTIONS,
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

/// Check for unused return values in a sequence item.
fn check_unused_return_in_seq_item(
    item: &T::SequenceItem,
    important_fns: &[(&str, &str)],
    out: &mut Vec<Diagnostic>,
    settings: &LintSettings,
    file_map: &MappedFiles,
    func_name: &str,
) {
    match &item.value {
        T::SequenceItem_::Seq(exp) => {
            // If a Seq item is a function call, its return value is discarded
            if let T::UnannotatedExp_::ModuleCall(call) = &exp.exp.value {
                let module_sym = call.module.value.module.value();
                let module_name = module_sym.as_str();
                let call_sym = call.name.value();
                let call_name = call_sym.as_str();

                for (mod_pattern, fn_pattern) in important_fns {
                    if module_name == *mod_pattern && call_name == *fn_pattern {
                        let loc = exp.exp.loc;
                        let Some((file, span, contents)) = diag_from_loc(file_map, &loc) else {
                            continue;
                        };
                        let anchor = loc.start() as usize;

                        push_diag(
                            out,
                            settings,
                            &UNUSED_RETURN_VALUE,
                            file,
                            span,
                            contents.as_ref(),
                            anchor,
                            format!(
                                "Return value of `{module_name}::{call_name}` in function `{func_name}` is ignored. \
                                 This may indicate a bug - the returned value (often a Coin or extracted value) should be used."
                            ),
                        );
                    }
                }
            }
        }
        T::SequenceItem_::Bind(_, _, exp) => {
            // Bound expressions are using their return value, so recurse into nested calls
            check_unused_return_in_exp(exp, important_fns, out, settings, file_map, func_name);
        }
        _ => {}
    }
}

/// Recursively check for unused return values in expressions.
fn check_unused_return_in_exp(
    exp: &T::Exp,
    important_fns: &[(&str, &str)],
    out: &mut Vec<Diagnostic>,
    settings: &LintSettings,
    file_map: &MappedFiles,
    func_name: &str,
) {
    match &exp.exp.value {
        T::UnannotatedExp_::Block((_, seq)) => {
            for item in seq.iter() {
                check_unused_return_in_seq_item(
                    item,
                    important_fns,
                    out,
                    settings,
                    file_map,
                    func_name,
                );
            }
        }
        T::UnannotatedExp_::IfElse(cond, t, e_opt) => {
            check_unused_return_in_exp(cond, important_fns, out, settings, file_map, func_name);
            check_unused_return_in_exp(t, important_fns, out, settings, file_map, func_name);
            if let Some(e) = e_opt {
                check_unused_return_in_exp(e, important_fns, out, settings, file_map, func_name);
            }
        }
        T::UnannotatedExp_::While(_, cond, body) => {
            check_unused_return_in_exp(cond, important_fns, out, settings, file_map, func_name);
            check_unused_return_in_exp(body, important_fns, out, settings, file_map, func_name);
        }
        T::UnannotatedExp_::Loop { body, .. } => {
            check_unused_return_in_exp(body, important_fns, out, settings, file_map, func_name);
        }
        T::UnannotatedExp_::BinopExp(l, _op, _ty, r) => {
            check_unused_return_in_exp(l, important_fns, out, settings, file_map, func_name);
            check_unused_return_in_exp(r, important_fns, out, settings, file_map, func_name);
        }
        T::UnannotatedExp_::UnaryExp(_, inner) => {
            check_unused_return_in_exp(inner, important_fns, out, settings, file_map, func_name);
        }
        T::UnannotatedExp_::Borrow(_, inner, _) => {
            check_unused_return_in_exp(inner, important_fns, out, settings, file_map, func_name);
        }
        T::UnannotatedExp_::TempBorrow(_, inner) => {
            check_unused_return_in_exp(inner, important_fns, out, settings, file_map, func_name);
        }
        T::UnannotatedExp_::Dereference(inner) => {
            check_unused_return_in_exp(inner, important_fns, out, settings, file_map, func_name);
        }
        T::UnannotatedExp_::Vector(_, _, _, args) => {
            check_unused_return_in_exp(args, important_fns, out, settings, file_map, func_name);
        }
        T::UnannotatedExp_::Builtin(_, args) => {
            check_unused_return_in_exp(args, important_fns, out, settings, file_map, func_name);
        }
        T::UnannotatedExp_::ExpList(items) => {
            for item in items.iter() {
                match item {
                    T::ExpListItem::Single(e, _) => {
                        check_unused_return_in_exp(
                            e,
                            important_fns,
                            out,
                            settings,
                            file_map,
                            func_name,
                        );
                    }
                    T::ExpListItem::Splat(_, e, _) => {
                        check_unused_return_in_exp(
                            e,
                            important_fns,
                            out,
                            settings,
                            file_map,
                            func_name,
                        );
                    }
                }
            }
        }
        T::UnannotatedExp_::ModuleCall(call) => {
            check_unused_return_in_exp(
                &call.arguments,
                important_fns,
                out,
                settings,
                file_map,
                func_name,
            );
        }
        T::UnannotatedExp_::Assign(_lvalues, _expected_types, rhs) => {
            check_unused_return_in_exp(rhs, important_fns, out, settings, file_map, func_name);
        }
        T::UnannotatedExp_::Return(exp) => {
            check_unused_return_in_exp(exp, important_fns, out, settings, file_map, func_name);
        }
        T::UnannotatedExp_::Abort(exp) => {
            check_unused_return_in_exp(exp, important_fns, out, settings, file_map, func_name);
        }
        T::UnannotatedExp_::Give(_, exp) => {
            check_unused_return_in_exp(exp, important_fns, out, settings, file_map, func_name);
        }
        T::UnannotatedExp_::Block((_, seq_items)) => {
            for item in seq_items.iter() {
                match &item.value {
                    T::SequenceItem_::Seq(exp) => {
                        check_unused_return_in_exp(
                            exp,
                            important_fns,
                            out,
                            settings,
                            file_map,
                            func_name,
                        );
                    }
                    T::SequenceItem_::Bind(_, _, exp) => {
                        check_unused_return_in_exp(
                            exp,
                            important_fns,
                            out,
                            settings,
                            file_map,
                            func_name,
                        );
                    }
                    _ => {}
                }
            }
        }
        T::UnannotatedExp_::IfElse(cond, if_body, else_body) => {
            check_unused_return_in_exp(cond, important_fns, out, settings, file_map, func_name);
            check_unused_return_in_exp(if_body, important_fns, out, settings, file_map, func_name);
            if let Some(else_e) = else_body {
                check_unused_return_in_exp(
                    else_e,
                    important_fns,
                    out,
                    settings,
                    file_map,
                    func_name,
                );
            }
        }
        T::UnannotatedExp_::While(_, cond, body) => {
            check_unused_return_in_exp(cond, important_fns, out, settings, file_map, func_name);
            check_unused_return_in_exp(body, important_fns, out, settings, file_map, func_name);
        }
        T::UnannotatedExp_::Loop { body, .. } => {
            check_unused_return_in_exp(body, important_fns, out, settings, file_map, func_name);
        }
        T::UnannotatedExp_::BinopExp(l, _op, _ty, r) => {
            check_unused_return_in_exp(l, important_fns, out, settings, file_map, func_name);
            check_unused_return_in_exp(r, important_fns, out, settings, file_map, func_name);
        }
        T::UnannotatedExp_::UnaryExp(_, inner) => {
            check_unused_return_in_exp(inner, important_fns, out, settings, file_map, func_name);
        }
        T::UnannotatedExp_::Borrow(_, inner, _) => {
            check_unused_return_in_exp(inner, important_fns, out, settings, file_map, func_name);
        }
        T::UnannotatedExp_::TempBorrow(_, inner) => {
            check_unused_return_in_exp(inner, important_fns, out, settings, file_map, func_name);
        }
        T::UnannotatedExp_::Dereference(inner) => {
            check_unused_return_in_exp(inner, important_fns, out, settings, file_map, func_name);
        }
        T::UnannotatedExp_::Vector(_, _, _, args) => {
            check_unused_return_in_exp(args, important_fns, out, settings, file_map, func_name);
        }
        T::UnannotatedExp_::Builtin(_, args) => {
            check_unused_return_in_exp(args, important_fns, out, settings, file_map, func_name);
        }
        T::UnannotatedExp_::ExpList(items) => {
            for item in items.iter() {
                match item {
                    T::ExpListItem::Single(e, _) => {
                        check_unused_return_in_exp(
                            e,
                            important_fns,
                            out,
                            settings,
                            file_map,
                            func_name,
                        );
                    }
                    T::ExpListItem::Splat(_, e, _) => {
                        check_unused_return_in_exp(
                            e,
                            important_fns,
                            out,
                            settings,
                            file_map,
                            func_name,
                        );
                    }
                }
            }
        }
        _ => {}
    }
}

// =========================================================================
// Share Owned Authority Lint (type-grounded)
// =========================================================================

/// DEPRECATED: This lint cannot be implemented with principled detection.
///
/// The ability pattern `key + store + !copy + !drop` matches ALL valuable Sui objects,
/// not just capabilities. This produces ~78% false positive rate on intentional
/// shared state patterns (pools, registries, kiosks, TransferPolicy).
///
/// Sui's built-in `share_owned` lint provides principled detection using dataflow
/// analysis to flag sharing of objects received as parameters (likely already owned).
#[allow(unused_variables)]
pub(crate) fn lint_share_owned_authority(
    _out: &mut Vec<Diagnostic>,
    _settings: &LintSettings,
    _file_map: &MappedFiles,
    _prog: &T::Program,
) -> Result<()> {
    // DEPRECATED: No-op. See docstring for rationale.
    Ok(())
}
