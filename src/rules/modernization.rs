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
                    ctx.report_diagnostic_for_node(node, diagnostic);
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
                    ctx.report_diagnostic_for_node(node, diagnostic);
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
                    ctx.report_diagnostic_for_node(node, diagnostic);
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

                    ctx.report_diagnostic_for_node(node, diagnostic);
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
                ctx.report_diagnostic_for_node(node, diagnostic);
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
                ctx.report_diagnostic_for_node(node, diagnostic);
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
                ctx.report_diagnostic_for_node(node, diagnostic);
                return;
            }
        });
    }
}

// ============================================================================
// REMOVED LINTS:
// - UnnecessaryPublicEntryLint - duplicates Sui compiler's built-in lint
// - PublicMutTxContextLint - duplicates Sui compiler's PreferMutableTxContext lint
// - WhileTrueToLoopLint - duplicates Move compiler's WhileTrueToLoop lint
// - PureFunctionTransferLint - experimental, many legitimate patterns
// - UnsafeArithmeticLint - experimental, needs dataflow analysis
// ============================================================================
