use crate::diagnostics::Span;
use crate::lint::{
    AnalysisKind, FixDescriptor, LintCategory, LintContext, LintDescriptor, LintRule, RuleGroup,
};
use tree_sitter::Node;

use super::patterns::{
    extract_assert_condition, extract_is_some_receiver, is_simple_equality_comparison,
    parse_length_comparison,
};
use super::util::{
    compact_ws, generate_method_call_fix, is_simple_ident, is_simple_receiver, parse_ref_ident,
    parse_ref_mut_ident, slice, split_args, split_call, walk,
};
use crate::diagnostics::{Applicability, Suggestion};

// ============================================================================
// EqualityInAssertLint - P1 (Near-Zero FP)
// ============================================================================

/// Generate assert_eq! fix from assert!(a == b, ...) pattern
fn generate_assert_eq_fix(assert_text: &str, condition: &str) -> Option<Suggestion> {
    // Split condition on == to get left and right operands
    let parts: Vec<&str> = condition.split("==").collect();
    if parts.len() != 2 {
        return None;
    }

    let left = parts[0].trim();
    let right = parts[1].trim();

    // Extract everything after the condition (error code, message, etc.)
    // Pattern: assert!(condition, error_code, message)
    let start = assert_text.find("assert!")? + 7; // Skip "assert!"
    let rest = assert_text.get(start..)?.trim_start();
    let inner_start = rest.find('(')? + 1;
    let inner_end = rest.rfind(')')?;
    let full_args = rest.get(inner_start..inner_end)?;

    // Find first comma at depth 0 (after condition, before error args)
    let mut depth: i32 = 0;
    let mut comma_pos = None;
    for (i, c) in full_args.char_indices() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                comma_pos = Some(i);
                break;
            }
            _ => {}
        }
    }

    // Build replacement: assert_eq!(left, right, ...remaining_args)
    let error_part = if let Some(pos) = comma_pos {
        let after_comma = full_args.get(pos + 1..)?.trim();
        format!(", {}", after_comma)
    } else {
        String::new()
    };

    let replacement = format!("assert_eq!({}, {}{})", left, right, error_part);

    Some(Suggestion {
        message: "Replace with assert_eq!".to_string(),
        replacement,
        applicability: Applicability::MachineApplicable,
    })
}

pub struct EqualityInAssertLint;

static EQUALITY_IN_ASSERT: LintDescriptor = LintDescriptor {
    name: "equality_in_assert",
    category: LintCategory::Style,
    description: "Prefer `assert_eq!(a, b)` over `assert!(a == b)` for clearer failure messages",
    group: RuleGroup::Stable,
    fix: FixDescriptor::safe("Replace `assert!(a == b)` with `assert_eq!(a, b)`"),
    analysis: AnalysisKind::Syntactic,
    gap: None,
};

impl LintRule for EqualityInAssertLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &EQUALITY_IN_ASSERT
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        walk(root, &mut |node| {
            if node.kind() != "macro_call_expression" {
                return;
            }

            let text = slice(source, node).trim();

            if !text.starts_with("assert!") {
                return;
            }

            // Don't flag assert_eq!, assert_ne!, etc.
            if text.starts_with("assert_eq!") || text.starts_with("assert_ne!") {
                return;
            }

            if let Some(condition) = extract_assert_condition(text) {
                // Check if it's a simple equality comparison
                if is_simple_equality_comparison(condition) {
                    // Generate auto-fix: assert!(a == b, ...) -> assert_eq!(a, b, ...)
                    let suggestion = generate_assert_eq_fix(text, condition);

                    let diagnostic = crate::diagnostics::Diagnostic {
                        lint: self.descriptor(),
                        level: ctx.settings().level_for(self.descriptor().name),
                        file: None,
                        span: Span::from_range(node.range()),
                        message: "Prefer `assert_eq!(a, b)` for clearer failure messages"
                            .to_string(),
                        help: Some("Use assert_eq! for better error messages".to_string()),
                        suggestion,
                    };
                    ctx.report_diagnostic(diagnostic);
                }
            }
        });
    }
}

// ============================================================================
// ManualOptionCheckLint - P1 (Near-Zero FP)
// ============================================================================

/// Generate do! macro fix from manual is_some() + destroy_some() pattern
fn generate_manual_option_fix(
    _if_text: &str,
    body_text: &str,
    var_name: &str,
) -> Option<Suggestion> {
    // Find the destroy_some line to extract binding name and remove it
    // Pattern: let binding_name = var_name.destroy_some();

    let destroy_pattern = format!("{}.destroy_some()", var_name);

    // Split body into lines
    let body_lines: Vec<&str> = body_text.lines().collect();

    // Find the line with destroy_some and extract binding name
    let mut binding_name = "value".to_string(); // default
    let mut filtered_lines = Vec::new();
    let mut found_destroy = false;

    for line in body_lines {
        let trimmed = line.trim();

        if trimmed.contains(&destroy_pattern) {
            found_destroy = true;
            // Try to extract binding name from: let binding_name = opt.destroy_some();
            if let Some(let_pos) = trimmed.find("let ") {
                let after_let = &trimmed[let_pos + 4..];
                if let Some(eq_pos) = after_let.find('=') {
                    binding_name = after_let[..eq_pos].trim().to_string();
                }
            }
            // Skip this line (don't add to filtered_lines)
            continue;
        }

        filtered_lines.push(line);
    }

    if !found_destroy {
        return None;
    }

    // Reconstruct body without the destroy_some line
    let new_body = filtered_lines.join("\n");

    // Extract just the statements part (remove opening/closing braces)
    let body_inner = new_body
        .trim()
        .strip_prefix('{')
        .and_then(|s| s.strip_suffix('}'))
        .unwrap_or(&new_body)
        .trim();

    // Build the do! macro call
    let replacement = if body_inner.is_empty() {
        format!("{}.do!(|{}| {{}})", var_name, binding_name)
    } else {
        format!(
            "{}.do!(|{}| {{\n{}\n}})",
            var_name, binding_name, body_inner
        )
    };

    Some(Suggestion {
        message: format!("Replace with {}.do! macro", var_name),
        replacement,
        applicability: Applicability::MaybeIncorrect,
    })
}

pub struct ManualOptionCheckLint;

static MANUAL_OPTION_CHECK: LintDescriptor = LintDescriptor {
    name: "manual_option_check",
    category: LintCategory::Modernization,
    description: "Prefer option macros (`do!`, `destroy_or!`) over manual `is_some()` + `destroy_some()` patterns",
    group: RuleGroup::Stable,
    fix: FixDescriptor::unsafe_fix("Replace with do! macro"),
    analysis: AnalysisKind::Syntactic,
    gap: None,
};

impl LintRule for ManualOptionCheckLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &MANUAL_OPTION_CHECK
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        walk(root, &mut |node| {
            if node.kind() != "if_expression" {
                return;
            }

            // Extract condition and body by walking children
            // Structure: if ( condition ) block [else block]
            let mut condition_node = None;
            let mut body_node = None;

            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                match child.kind() {
                    "dot_expression" | "binary_expression" | "call_expression"
                    | "name_expression" => {
                        if condition_node.is_none() {
                            condition_node = Some(child);
                        }
                    }
                    "block" => {
                        if body_node.is_none() {
                            body_node = Some(child);
                        }
                    }
                    _ => {}
                }
            }

            let Some(condition_node) = condition_node else {
                return;
            };
            let Some(body_node) = body_node else { return };

            let condition = slice(source, condition_node).trim();
            let body = slice(source, body_node);

            // Pattern: x.is_some() condition with destroy_some in body
            if let Some(var_name) = extract_is_some_receiver(condition) {
                let destroy_pattern = format!("{}.destroy_some()", var_name);
                if body.contains(&destroy_pattern) {
                    // Generate auto-fix
                    let if_text = slice(source, node);
                    let suggestion = generate_manual_option_fix(if_text, body, var_name);

                    let diagnostic = crate::diagnostics::Diagnostic {
                        lint: self.descriptor(),
                        level: ctx.settings().level_for(self.descriptor().name),
                        file: None,
                        span: Span::from_range(node.range()),
                        message: format!(
                            "Consider `{}.do!(|v| ...)` instead of manual `is_some()` + `destroy_some()`",
                            var_name
                        ),
                        help: Some("Use do! macro for cleaner option handling".to_string()),
                        suggestion,
                    };
                    ctx.report_diagnostic(diagnostic);
                }
            }
        });
    }
}

// ============================================================================
// ManualLoopIterationLint - P1 (Near-Zero FP)
// ============================================================================

/// Generate do_ref! macro fix from manual while loop with index
fn generate_manual_loop_fix(body_text: &str, iter_var: &str, vec_var: &str) -> Option<Suggestion> {
    // Find the borrow line to extract binding name
    // Pattern: let binding_name = vec_var.borrow(iter_var);

    let borrow_pattern = format!("{}.borrow({})", vec_var, iter_var);

    // Split body into lines
    let body_lines: Vec<&str> = body_text.lines().collect();

    // Find the line with borrow and extract binding name
    let mut binding_name = "elem".to_string(); // default
    let mut filtered_lines = Vec::new();

    for line in body_lines {
        let trimmed = line.trim();

        // Skip increment lines
        let increment_patterns = [
            format!("{} = {} + 1", iter_var, iter_var),
            format!("{} = 1 + {}", iter_var, iter_var),
            format!("{}={} + 1", iter_var, iter_var),
            format!("{}={}+1", iter_var, iter_var),
        ];

        if increment_patterns.iter().any(|p| trimmed.contains(p)) {
            continue; // Skip increment line
        }

        // Extract binding from borrow line
        if trimmed.contains(&borrow_pattern) {
            if let Some(let_pos) = trimmed.find("let ") {
                let after_let = &trimmed[let_pos + 4..];
                if let Some(eq_pos) = after_let.find('=') {
                    binding_name = after_let[..eq_pos].trim().to_string();
                }
            }
            // Keep this line but will need to transform it
            filtered_lines.push(line);
            continue;
        }

        filtered_lines.push(line);
    }

    // Reconstruct body without increment
    let new_body = filtered_lines.join("\n");

    // Extract just the statements (remove braces)
    let body_inner = new_body
        .trim()
        .strip_prefix('{')
        .and_then(|s| s.strip_suffix('}'))
        .unwrap_or(&new_body)
        .trim();

    // Remove the borrow line from body_inner since it becomes the parameter
    let body_final: Vec<&str> = body_inner
        .lines()
        .filter(|line| !line.trim().contains(&borrow_pattern))
        .collect();

    let body_final_str = body_final.join("\n").trim().to_string();

    // Build the do_ref! macro call
    let replacement = if body_final_str.is_empty() {
        format!("{}.do_ref!(|{}| {{}})", vec_var, binding_name)
    } else {
        format!(
            "{}.do_ref!(|{}| {{\n{}\n}})",
            vec_var, binding_name, body_final_str
        )
    };

    Some(Suggestion {
        message: format!(
            "Replace with {}.do_ref! macro (note: manually remove `let mut {} = 0;` above)",
            vec_var, iter_var
        ),
        replacement,
        applicability: Applicability::MaybeIncorrect,
    })
}

pub struct ManualLoopIterationLint;

static MANUAL_LOOP_ITERATION: LintDescriptor = LintDescriptor {
    name: "manual_loop_iteration",
    category: LintCategory::Modernization,
    description: "Prefer loop macros (`do_ref!`, `fold!`) over manual while loops with index",
    group: RuleGroup::Stable,
    fix: FixDescriptor::unsafe_fix("Replace with do_ref! macro"),
    analysis: AnalysisKind::Syntactic,
    gap: None,
};

impl LintRule for ManualLoopIterationLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &MANUAL_LOOP_ITERATION
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        walk(root, &mut |node| {
            if node.kind() != "while_expression" {
                return;
            }

            // Extract condition and body
            let condition_node = node
                .child_by_field_name("condition")
                .or_else(|| node.child_by_field_name("eb"));
            let body_node = node
                .child_by_field_name("body")
                .or_else(|| node.child_by_field_name("e"));

            let Some(condition_node) = condition_node else {
                return;
            };
            let Some(body_node) = body_node else { return };

            let condition = slice(source, condition_node).trim();
            let body = slice(source, body_node);

            // Pattern: i < vec.length()
            if let Some((iter_var, vec_var)) = parse_length_comparison(condition) {
                // Check body has increment: i = i + 1
                let increment_patterns = [
                    format!("{} = {} + 1", iter_var, iter_var),
                    format!("{} = 1 + {}", iter_var, iter_var),
                    format!("{}={} + 1", iter_var, iter_var),
                    format!("{}={}+1", iter_var, iter_var),
                ];

                let has_increment = increment_patterns.iter().any(|p| body.contains(p));

                // CRITICAL: Also check for borrow pattern to avoid false positives
                let borrow_pattern = format!("{}.borrow({})", vec_var, iter_var);
                let has_borrow = body.contains(&borrow_pattern);

                if has_increment && has_borrow {
                    // Generate auto-fix
                    let suggestion = generate_manual_loop_fix(body, iter_var, vec_var);

                    let diagnostic = crate::diagnostics::Diagnostic {
                        lint: self.descriptor(),
                        level: ctx.settings().level_for(self.descriptor().name),
                        file: None,
                        span: Span::from_range(node.range()),
                        message: format!(
                            "Consider `{}.do_ref!(|e| ...)` instead of manual while loop with index",
                            vec_var
                        ),
                        help: Some(format!(
                            "Use do_ref! macro for cleaner iteration. Note: You must manually remove `let mut {} = 0;` above this loop.",
                            iter_var
                        )),
                        suggestion,
                    };
                    ctx.report_diagnostic(diagnostic);
                }
            }
        });
    }
}

// ============================================================================
// Existing lints below (with extended modern_method_syntax)
// ============================================================================

pub struct ModernModuleSyntaxLint;

static MODERN_MODULE_SYNTAX: LintDescriptor = LintDescriptor {
    name: "modern_module_syntax",
    category: LintCategory::Modernization,
    description: "Prefer Move 2024 module label syntax (module x::y;) over block form (module x::y { ... })",
    group: RuleGroup::Stable,
    fix: FixDescriptor::safe("Convert to Move 2024 module label syntax"),
    analysis: AnalysisKind::Syntactic,
    gap: None,
};

impl LintRule for ModernModuleSyntaxLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &MODERN_MODULE_SYNTAX
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        walk(root, &mut |node| {
            if node.kind() != "module_definition" {
                return;
            }

            // Zero/near-zero FP strategy: distinguish based on the module header.
            //
            // * Legacy: `module pkg::m {`  (brace on the module header line)
            // * Modern:  `module pkg::m;`  (semicolon on the module header line)
            let text = slice(source, node);
            let header = text.lines().next().unwrap_or("");

            let brace = header.find('{');
            let semi = header.find(';');

            let is_legacy_block = match (brace, semi) {
                (Some(b), Some(s)) => b < s,
                (Some(_), None) => true,
                _ => false,
            };

            if is_legacy_block {
                // Extract module identity for the fix
                // Find the module identity node to get the module path
                let module_identity = node
                    .children(&mut node.walk())
                    .find(|child| child.kind() == "module_identity")
                    .map(|id_node| slice(source, id_node).trim());

                if let Some(module_path) = module_identity {
                    // Create the replacement: "module path;"
                    let replacement = format!("module {};", module_path);

                    // Create diagnostic with auto-fix suggestion
                    let diagnostic = crate::diagnostics::Diagnostic {
                        lint: self.descriptor(),
                        level: ctx.settings().level_for(self.descriptor().name),
                        file: None,
                        span: Span::from_range(node.range()),
                        message: "Use Move 2024 module label syntax: `module pkg::mod;`"
                            .to_string(),
                        help: Some("Convert to label syntax".to_string()),
                        suggestion: Some(Suggestion {
                            message: format!(
                                "Convert `module {} {{ ... }}` to `{}`",
                                module_path, replacement
                            ),
                            replacement,
                            applicability: Applicability::MachineApplicable,
                        }),
                    };

                    // Check for suppression
                    let node_start = node.start_byte();
                    if crate::suppression::is_suppressed_at(
                        source,
                        node_start,
                        self.descriptor().name,
                    ) {
                        return;
                    }

                    ctx.report_diagnostic(diagnostic);
                } else {
                    // Fallback if we can't extract module identity
                    ctx.report_node(
                        self.descriptor(),
                        node,
                        "Use Move 2024 module label syntax: `module pkg::mod;`",
                    );
                }
            }
        });
    }
}

pub struct PreferVectorMethodsLint;

static PREFER_VECTOR_METHODS: LintDescriptor = LintDescriptor {
    name: "prefer_vector_methods",
    category: LintCategory::Modernization,
    description: "Prefer method syntax on vectors (e.g., v.push_back(x), v.length())",
    group: RuleGroup::Stable,
    fix: FixDescriptor::safe("Convert to method syntax"),
    analysis: AnalysisKind::Syntactic,
    gap: None,
};

impl LintRule for PreferVectorMethodsLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &PREFER_VECTOR_METHODS
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        walk(root, &mut |node| {
            if node.kind() != "call_expression" {
                return;
            }

            let text = slice(source, node).trim();
            let Some((callee, args_str)) = split_call(text) else {
                return;
            };

            let callee = compact_ws(callee);
            if callee == "vector::push_back" {
                let Some(args) = split_args(args_str) else {
                    return;
                };
                if args.len() != 2 {
                    return;
                }
                let Some(receiver) = parse_ref_mut_ident(args[0]) else {
                    return;
                };

                // Generate auto-fix
                let suggestion = if is_simple_receiver(receiver) {
                    let replacement =
                        generate_method_call_fix(receiver, "push_back", vec![args[1]]);
                    Some(Suggestion {
                        message: format!("Use method syntax: {}", replacement),
                        replacement,
                        applicability: Applicability::MachineApplicable,
                    })
                } else {
                    None
                };

                let diagnostic = crate::diagnostics::Diagnostic {
                    lint: self.descriptor(),
                    level: ctx.settings().level_for(self.descriptor().name),
                    file: None,
                    span: Span::from_range(node.range()),
                    message: format!("Prefer method syntax: `{receiver}.push_back(...)`"),
                    help: Some("Use method call syntax for cleaner code".to_string()),
                    suggestion,
                };
                ctx.report_diagnostic(diagnostic);
            } else if callee == "vector::length" {
                let Some(args) = split_args(args_str) else {
                    return;
                };
                if args.len() != 1 {
                    return;
                }
                let Some(receiver) = parse_ref_ident(args[0]) else {
                    return;
                };

                // Generate auto-fix
                let suggestion = if is_simple_receiver(receiver) {
                    let replacement = generate_method_call_fix(receiver, "length", vec![]);
                    Some(Suggestion {
                        message: format!("Use method syntax: {}", replacement),
                        replacement,
                        applicability: Applicability::MachineApplicable,
                    })
                } else {
                    None
                };

                let diagnostic = crate::diagnostics::Diagnostic {
                    lint: self.descriptor(),
                    level: ctx.settings().level_for(self.descriptor().name),
                    file: None,
                    span: Span::from_range(node.range()),
                    message: format!("Prefer method syntax: `{receiver}.length()`"),
                    help: Some("Use method call syntax for cleaner code".to_string()),
                    suggestion,
                };
                ctx.report_diagnostic(diagnostic);
            }
        });
    }
}

pub struct ModernMethodSyntaxLint;

static MODERN_METHOD_SYNTAX: LintDescriptor = LintDescriptor {
    name: "modern_method_syntax",
    category: LintCategory::Modernization,
    description: "Prefer Move 2024 method call syntax for common allowlisted functions",
    group: RuleGroup::Stable,
    fix: FixDescriptor::safe("Convert to method syntax"),
    analysis: AnalysisKind::Syntactic,
    gap: None,
};

/// Extended allowlist of known-safe method syntax transformations
/// Format: (module, function, method_name, arg_count)
const KNOWN_METHOD_TRANSFORMS: &[(&str, &str, &str, usize)] = &[
    // === Core TxContext ===
    ("tx_context", "sender", "sender", 1),
    ("tx_context", "epoch", "epoch", 1),
    ("tx_context", "epoch_timestamp_ms", "epoch_timestamp_ms", 1),
    // === Object Operations ===
    ("object", "delete", "delete", 1),
    ("object", "id", "id", 1),
    ("object", "borrow_id", "borrow_id", 1),
    ("object", "uid_to_inner", "uid_to_inner", 1),
    ("object", "uid_as_inner", "uid_as_inner", 1),
    // === Coin Operations ===
    ("coin", "value", "value", 1),
    ("coin", "into_balance", "into_balance", 1),
    ("coin", "destroy_zero", "destroy_zero", 1),
    ("coin", "from_balance", "from_balance", 2),
    ("coin", "split", "split", 3),
    ("coin", "join", "join", 2),
    // === Balance Operations ===
    ("balance", "value", "value", 1),
    ("balance", "destroy_zero", "destroy_zero", 1),
    ("balance", "split", "split", 2),
    ("balance", "join", "join", 2),
    // === Option Operations ===
    ("option", "is_some", "is_some", 1),
    ("option", "is_none", "is_none", 1),
    ("option", "borrow", "borrow", 1),
    ("option", "borrow_mut", "borrow_mut", 1),
    ("option", "destroy_some", "destroy_some", 1),
    ("option", "destroy_none", "destroy_none", 1),
    ("option", "extract", "extract", 1),
    ("option", "get_with_default", "get_with_default", 2),
    ("option", "swap", "swap", 2),
    ("option", "fill", "fill", 2),
    ("option", "contains", "contains", 2),
    // === String Operations ===
    ("string", "length", "length", 1),
    ("string", "is_empty", "is_empty", 1),
    ("string", "as_bytes", "as_bytes", 1),
    ("string", "into_bytes", "into_bytes", 1),
    ("string", "append", "append", 2),
    ("string", "sub_string", "sub_string", 3),
    // === ASCII String Operations ===
    ("ascii", "length", "length", 1),
    ("ascii", "is_empty", "is_empty", 1),
    ("ascii", "as_bytes", "as_bytes", 1),
    ("ascii", "into_bytes", "into_bytes", 1),
    // === Table Operations ===
    ("table", "length", "length", 1),
    ("table", "is_empty", "is_empty", 1),
    ("table", "contains", "contains", 2),
    ("table", "borrow", "borrow", 2),
    ("table", "borrow_mut", "borrow_mut", 2),
    ("table", "add", "add", 3),
    ("table", "remove", "remove", 2),
    // === Vector Operations (additional to prefer_vector_methods) ===
    ("vector", "is_empty", "is_empty", 1),
    ("vector", "borrow", "borrow", 2),
    ("vector", "borrow_mut", "borrow_mut", 2),
    ("vector", "pop_back", "pop_back", 1),
    ("vector", "swap_remove", "swap_remove", 2),
    ("vector", "contains", "contains", 2),
    // === Transfer Operations ===
    ("transfer", "transfer", "transfer", 2),
    ("transfer", "public_transfer", "public_transfer", 2),
    ("transfer", "share_object", "share_object", 1),
    ("transfer", "public_share_object", "public_share_object", 1),
    ("transfer", "freeze_object", "freeze_object", 1),
    (
        "transfer",
        "public_freeze_object",
        "public_freeze_object",
        1,
    ),
    // === Event Operations ===
    ("event", "emit", "emit", 1),
    // === BCS Operations ===
    ("bcs", "to_bytes", "to_bytes", 1),
];

impl LintRule for ModernMethodSyntaxLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &MODERN_METHOD_SYNTAX
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        walk(root, &mut |node| {
            if node.kind() != "call_expression" {
                return;
            }

            let text = slice(source, node).trim();
            let Some((callee, args_str)) = split_call(text) else {
                return;
            };

            let callee = compact_ws(callee);

            // Try to match against the extended allowlist
            for (module, func, method, expected_args) in KNOWN_METHOD_TRANSFORMS {
                let pattern = format!("{}::{}", module, func);
                if callee != pattern {
                    continue;
                }

                let Some(args) = split_args(args_str) else {
                    continue;
                };

                if args.len() != *expected_args {
                    continue;
                }

                // First arg is the receiver
                let receiver = args[0].trim();

                // Handle &receiver or &mut receiver
                let clean_receiver = receiver
                    .strip_prefix("&mut ")
                    .or_else(|| receiver.strip_prefix("&"))
                    .unwrap_or(receiver)
                    .trim();

                if !is_simple_ident(clean_receiver) {
                    continue;
                }

                // Generate auto-fix
                let suggestion = if is_simple_receiver(clean_receiver) {
                    // Remaining args (skip first arg which is receiver)
                    let remaining_args: Vec<&str> = args.iter().skip(1).copied().collect();
                    let replacement =
                        generate_method_call_fix(clean_receiver, method, remaining_args);
                    Some(Suggestion {
                        message: format!("Use method syntax: {}", replacement),
                        replacement,
                        applicability: Applicability::MachineApplicable,
                    })
                } else {
                    None
                };

                let diagnostic = crate::diagnostics::Diagnostic {
                    lint: self.descriptor(),
                    level: ctx.settings().level_for(self.descriptor().name),
                    file: None,
                    span: Span::from_range(node.range()),
                    message: format!("Prefer method syntax: `{}.{}(...)`", clean_receiver, method),
                    help: Some("Use method call syntax for cleaner code".to_string()),
                    suggestion,
                };
                ctx.report_diagnostic(diagnostic);
                return;
            }
        });
    }
}

pub struct UnnecessaryPublicEntryLint;

pub(crate) static UNNECESSARY_PUBLIC_ENTRY: LintDescriptor = LintDescriptor {
    name: "unnecessary_public_entry",
    category: LintCategory::Modernization,
    description: "Use either `public` or `entry`, but not both on the same function",
    group: RuleGroup::Stable,
    fix: FixDescriptor::safe("Remove redundant `public` modifier"),
    analysis: AnalysisKind::Syntactic,
    gap: None,
};

impl LintRule for UnnecessaryPublicEntryLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &UNNECESSARY_PUBLIC_ENTRY
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        walk(root, &mut |node| {
            if node.kind() != "function_definition" {
                return;
            }

            let (has_public, has_entry) = function_modifiers(node, source);
            if has_public && has_entry {
                // Generate auto-fix to remove public
                let suggestion = generate_remove_public_fix(node, source);

                let diagnostic = crate::diagnostics::Diagnostic {
                    lint: self.descriptor(),
                    level: ctx.settings().level_for(self.descriptor().name),
                    file: None,
                    span: Span::from_range(node.range()),
                    message: "Functions should not be both `public` and `entry`; remove one of the modifiers".to_string(),
                    help: Some("Remove `public` modifier - `entry` functions are implicitly public".to_string()),
                    suggestion,
                };
                ctx.report_diagnostic(diagnostic);
            }
        });
    }
}

pub struct PublicMutTxContextLint;

pub(crate) static PUBLIC_MUT_TX_CONTEXT: LintDescriptor = LintDescriptor {
    name: "public_mut_tx_context",
    category: LintCategory::Modernization,
    description: "TxContext parameters should be `&mut TxContext`, not `&TxContext`",
    group: RuleGroup::Stable,
    fix: FixDescriptor::safe("Add `mut` to TxContext parameter"),
    analysis: AnalysisKind::Syntactic,
    gap: None,
};

impl LintRule for PublicMutTxContextLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &PUBLIC_MUT_TX_CONTEXT
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        walk(root, &mut |node| {
            if node.kind() != "function_definition" {
                return;
            }

            // Only check public or entry functions
            let (has_public, has_entry) = function_modifiers(node, source);
            if !has_public && !has_entry {
                return;
            }

            if let Some(params) = node.child_by_field_name("parameters") {
                let mut cursor = params.walk();
                for param in params.children(&mut cursor) {
                    if param.kind() != "function_parameter"
                        && param.kind() != "mut_function_parameter"
                    {
                        continue;
                    }
                    let Some(ty) = param.child_by_field_name("type") else {
                        continue;
                    };
                    let type_text = slice(source, ty);
                    if let Some(message) = needs_mut_tx_context(type_text) {
                        // Generate auto-fix
                        let suggestion = generate_mut_tx_context_fix(type_text);

                        let diagnostic = crate::diagnostics::Diagnostic {
                            lint: self.descriptor(),
                            level: ctx.settings().level_for(self.descriptor().name),
                            file: None,
                            span: Span::from_range(ty.range()),
                            message,
                            help: Some("Add `mut` to make TxContext mutable".to_string()),
                            suggestion,
                        };
                        ctx.report_diagnostic(diagnostic);
                    }
                }
            }
        });
    }
}

pub struct WhileTrueToLoopLint;

static WHILE_TRUE_TO_LOOP: LintDescriptor = LintDescriptor {
    name: "while_true_to_loop",
    category: LintCategory::Modernization,
    description: "Prefer `loop { ... }` over `while (true) { ... }`",
    group: RuleGroup::Stable,
    fix: FixDescriptor::safe("Replace `while (true)` with `loop`"),
    analysis: AnalysisKind::Syntactic,
    gap: None,
};

impl LintRule for WhileTrueToLoopLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &WHILE_TRUE_TO_LOOP
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        walk(root, &mut |node| {
            if node.kind() != "while_expression" {
                return;
            }
            let Some(cond) = node.child_by_field_name("eb") else {
                return;
            };
            if slice(source, cond).trim() == "true" {
                // Find the "while" keyword and condition part to replace with "loop"
                // The while_expression should start with "while" and have a condition
                let node_text = slice(source, node);

                // Find where the condition ends (look for opening brace or body)
                let Some(body_start) = node_text.find('{') else {
                    ctx.report_node(
                        self.descriptor(),
                        node,
                        "Use `loop { ... }` for infinite loops instead of `while (true)`"
                            .to_string(),
                    );
                    return;
                };

                // Replace "while (true)" part with "loop"
                // Find the byte offsets
                let node_start = node.start_byte();
                let _replacement_end = node_start + body_start;

                // Create a diagnostic with suggestion
                let diagnostic = crate::diagnostics::Diagnostic {
                    lint: self.descriptor(),
                    level: ctx.settings().level_for(self.descriptor().name),
                    file: None,
                    span: Span::from_range(node.range()),
                    message: "Use `loop { ... }` for infinite loops instead of `while (true)`"
                        .to_string(),
                    help: Some("Replace with `loop`".to_string()),
                    suggestion: Some(Suggestion {
                        message: "Replace `while (true)` with `loop`".to_string(),
                        replacement: format!("loop {}", &node_text[body_start..]),
                        applicability: Applicability::MachineApplicable,
                    }),
                };

                // Check for suppression
                if crate::suppression::is_suppressed_at(source, node_start, self.descriptor().name)
                {
                    return;
                }

                ctx.report_diagnostic(diagnostic);
            }
        });
    }
}

fn function_modifiers(node: Node, source: &str) -> (bool, bool) {
    let mut cursor = node.walk();
    let mut has_public = false;
    let mut has_entry = false;
    let mut seen_fun = false;

    for child in node.children(&mut cursor) {
        if seen_fun {
            break;
        }
        match child.kind() {
            "modifier" => {
                let text = slice(source, child);
                if text.starts_with("public") {
                    has_public = true;
                } else if text.trim() == "entry" {
                    has_entry = true;
                }
            }
            "fun" => {
                seen_fun = true;
            }
            _ => {}
        }
    }

    (has_public, has_entry)
}

/// Generate fix to remove `public` modifier from `public entry` functions
fn generate_remove_public_fix(node: Node, source: &str) -> Option<Suggestion> {
    let function_text = slice(source, node);

    // Find the public modifier node
    let mut cursor = node.walk();
    let mut public_node = None;
    let mut seen_fun = false;

    for child in node.children(&mut cursor) {
        if seen_fun {
            break;
        }
        if child.kind() == "modifier" {
            let text = slice(source, child);
            if text.starts_with("public") {
                public_node = Some(child);
            }
        } else if child.kind() == "fun" {
            seen_fun = true;
        }
    }

    let public_node = public_node?;

    // Get the text of the public modifier (might be "public" or "public(...)")
    let public_text = slice(source, public_node);

    // Simple approach: replace "public " with empty string
    // Handle various spacing: "public entry", "public  entry", "public\nentry"
    let replacement = if function_text.contains("public entry") {
        function_text.replace("public entry", "entry")
    } else if function_text.contains("public  entry") {
        function_text.replace("public  entry", "entry")
    } else {
        // General case: remove "public" and any trailing whitespace before "entry"
        // Find where "public" appears and remove it along with trailing spaces
        let public_start = function_text.find(public_text)?;
        let before = &function_text[..public_start];
        let after_public = public_start + public_text.len();
        let after = &function_text[after_public..];

        // Skip whitespace after "public" until we find "entry"
        let after_trimmed = after.trim_start();
        format!("{}{}", before, after_trimmed)
    };

    Some(Suggestion {
        message: "Remove redundant `public` modifier".to_string(),
        replacement,
        applicability: Applicability::MachineApplicable,
    })
}

fn needs_mut_tx_context(type_text: &str) -> Option<String> {
    let trimmed = type_text.trim_start();
    if !trimmed.starts_with('&') {
        return None;
    }
    let mut rest = trimmed.trim_start_matches('&').trim_start();
    if rest.starts_with("mut") {
        return None;
    }
    if rest.starts_with('(') {
        // References to tuples etc are irrelevant here.
        return None;
    }
    // Strip type arguments or additional qualifiers.
    if let Some(idx) = rest.find('<') {
        rest = &rest[..idx];
    }
    let base = rest.trim_end_matches(|c: char| c == ';' || c.is_whitespace());
    if base.ends_with("TxContext") {
        Some("TxContext parameters should use `&mut TxContext`".to_string())
    } else {
        None
    }
}

/// Generate fix to add `mut` to TxContext reference
fn generate_mut_tx_context_fix(type_text: &str) -> Option<Suggestion> {
    let trimmed = type_text.trim_start();

    // Pattern: &TxContext or & TxContext
    if !trimmed.starts_with('&') {
        return None;
    }

    let after_ref = trimmed[1..].trim_start();

    // Already has mut?
    if after_ref.starts_with("mut") {
        return None;
    }

    // Check it's actually TxContext (including module-qualified)
    let base = after_ref.trim_end_matches(|c: char| c == ';' || c.is_whitespace());
    if let Some(idx) = base.find('<') {
        // Strip type arguments
        if !base[..idx].ends_with("TxContext") {
            return None;
        }
    } else if !base.ends_with("TxContext") {
        return None;
    }

    // Insert "mut " after the "&"
    let replacement = format!("&mut {}", after_ref);

    Some(Suggestion {
        message: "Add `mut` to TxContext parameter".to_string(),
        replacement,
        applicability: Applicability::MachineApplicable,
    })
}

// ============================================================================
// PureFunctionTransferLint - Experimental (Medium-High FP Risk)
// ============================================================================

pub struct PureFunctionTransferLint;

static PURE_FUNCTION_TRANSFER: LintDescriptor = LintDescriptor {
    name: "pure_function_transfer",
    category: LintCategory::Suspicious,
    description: "Non-entry functions should not call transfer internally; return the object instead (experimental - many legitimate patterns)",
    group: RuleGroup::Experimental,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::Syntactic,
    gap: None,
};

impl LintRule for PureFunctionTransferLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &PURE_FUNCTION_TRANSFER
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        walk(root, &mut |node| {
            if node.kind() != "function_definition" {
                return;
            }

            // Check if this function is NOT an entry function
            let (has_public, has_entry) = function_modifiers(node, source);

            // Entry functions are allowed to transfer
            if has_entry {
                return;
            }

            // Private functions are also often allowed to transfer (internal helpers)
            // Only flag public non-entry functions
            if !has_public {
                return;
            }

            // Look for transfer calls in the function body
            let Some(body) = node.child_by_field_name("body") else {
                return;
            };

            // Walk the body looking for transfer calls
            check_for_transfer_calls(body, source, ctx, &PURE_FUNCTION_TRANSFER);
        });
    }
}

/// Check if the body contains transfer::* calls
fn check_for_transfer_calls(
    body: Node,
    source: &str,
    ctx: &mut LintContext<'_>,
    lint: &'static LintDescriptor,
) {
    walk(body, &mut |node| {
        if node.kind() != "call_expression" {
            return;
        }

        let text = slice(source, node).trim();
        let compact = compact_ws(text);

        // Check for transfer:: calls
        if compact.starts_with("transfer::transfer(")
            || compact.starts_with("transfer::public_transfer(")
            || compact.starts_with("transfer::share_object(")
            || compact.starts_with("transfer::public_share_object(")
            || compact.starts_with("transfer::freeze_object(")
            || compact.starts_with("transfer::public_freeze_object(")
        {
            ctx.report_node(
                lint,
                node,
                "Non-entry public functions should return objects instead of transferring internally. \
                 This makes the function more composable.",
            );
        }
    });
}

// ============================================================================
// UnsafeArithmeticLint - Preview (Medium FP Risk)
// ============================================================================

pub struct UnsafeArithmeticLint;

static UNSAFE_ARITHMETIC: LintDescriptor = LintDescriptor {
    name: "unsafe_arithmetic",
    category: LintCategory::Suspicious,
    description: "Detect potentially unsafe arithmetic operations (experimental, requires dataflow analysis)",
    group: RuleGroup::Experimental,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::Syntactic,
    gap: None,
};

/// Variable name patterns that suggest financial/balance operations
const BALANCE_PATTERNS: &[&str] = &[
    "balance",
    "reserve",
    "supply",
    "total",
    "amount",
    "value",
    "funds",
    "liquidity",
    "deposit",
    "withdraw",
    "stake",
    "reward",
    "fee",
];

impl LintRule for UnsafeArithmeticLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &UNSAFE_ARITHMETIC
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        walk(root, &mut |node| {
            if node.kind() != "binary_expression" {
                return;
            }

            let Some(op_node) = node.child_by_field_name("operator") else {
                return;
            };
            let Some(left_node) = node.child_by_field_name("left") else {
                return;
            };
            let Some(right_node) = node.child_by_field_name("right") else {
                return;
            };

            let op = slice(source, op_node).trim();
            let left = slice(source, left_node).to_lowercase();
            let right = slice(source, right_node).to_lowercase();

            // Check for subtraction with balance-like operands
            if op == "-" {
                let left_is_balance = BALANCE_PATTERNS.iter().any(|p| left.contains(p));
                let right_is_amount = right.contains("amount")
                    || right.contains("value")
                    || right.contains("fee")
                    || right.contains("withdraw");

                if left_is_balance && right_is_amount {
                    ctx.report_node(
                        self.descriptor(),
                        node,
                        "Potential underflow: ensure right operand <= left operand before subtraction",
                    );
                }
            }

            // Check for multiplication that could overflow
            if op == "*" {
                let involves_balance = BALANCE_PATTERNS
                    .iter()
                    .any(|p| left.contains(p) || right.contains(p));

                // Only flag if both operands look like they could be large values
                let left_looks_large = left.contains("price")
                    || left.contains("rate")
                    || left.contains("amount")
                    || left.contains("balance");
                let right_looks_large = right.contains("price")
                    || right.contains("rate")
                    || right.contains("amount")
                    || right.contains("balance");

                if involves_balance && (left_looks_large || right_looks_large) {
                    ctx.report_node(
                        self.descriptor(),
                        node,
                        "Potential overflow: consider bounds checking before multiplication",
                    );
                }
            }
        });
    }
}
