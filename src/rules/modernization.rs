use crate::lint::{FixDescriptor, LintCategory, LintContext, LintDescriptor, LintRule, RuleGroup};
use tree_sitter::Node;

use super::patterns::{
    extract_assert_condition, extract_is_some_receiver, is_simple_equality_comparison,
    parse_length_comparison,
};
use super::util::{
    compact_ws, is_simple_ident, parse_ref_ident, parse_ref_mut_ident, slice, split_args,
    split_call, walk,
};

// ============================================================================
// EqualityInAssertLint - P1 (Near-Zero FP)
// ============================================================================

pub struct EqualityInAssertLint;

static EQUALITY_IN_ASSERT: LintDescriptor = LintDescriptor {
    name: "equality_in_assert",
    category: LintCategory::Style,
    description: "Prefer `assert_eq!(a, b)` over `assert!(a == b)` for clearer failure messages",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
};

impl LintRule for EqualityInAssertLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &EQUALITY_IN_ASSERT
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        walk(root, &mut |node| {
            if node.kind() != "macro_invocation" {
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
                    ctx.report_node(
                        self.descriptor(),
                        node,
                        "Prefer `assert_eq!(a, b)` for clearer failure messages",
                    );
                }
            }
        });
    }
}

// ============================================================================
// ManualOptionCheckLint - P1 (Near-Zero FP)
// ============================================================================

pub struct ManualOptionCheckLint;

static MANUAL_OPTION_CHECK: LintDescriptor = LintDescriptor {
    name: "manual_option_check",
    category: LintCategory::Modernization,
    description: "Prefer option macros (`do!`, `destroy_or!`) over manual `is_some()` + `destroy_some()` patterns",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
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

            // Extract condition
            let condition_node = node
                .child_by_field_name("condition")
                .or_else(|| node.child_by_field_name("eb"));
            let body_node = node
                .child_by_field_name("consequence")
                .or_else(|| node.child_by_field_name("e"));

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
                    ctx.report_node(
                        self.descriptor(),
                        node,
                        format!(
                            "Consider `{}.do!(|v| ...)` instead of manual `is_some()` + `destroy_some()`",
                            var_name
                        ),
                    );
                }
            }
        });
    }
}

// ============================================================================
// ManualLoopIterationLint - P1 (Near-Zero FP)
// ============================================================================

pub struct ManualLoopIterationLint;

static MANUAL_LOOP_ITERATION: LintDescriptor = LintDescriptor {
    name: "manual_loop_iteration",
    category: LintCategory::Modernization,
    description: "Prefer loop macros (`do_ref!`, `fold!`) over manual while loops with index",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
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

                if has_increment {
                    ctx.report_node(
                        self.descriptor(),
                        node,
                        format!(
                            "Consider `{}.do_ref!(|e| ...)` instead of manual while loop with index",
                            vec_var
                        ),
                    );
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
    fix: FixDescriptor::none(),
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
                ctx.report_node(
                    self.descriptor(),
                    node,
                    "Use Move 2024 module label syntax: `module pkg::mod;`",
                );
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
    fix: FixDescriptor::none(),
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

                ctx.report_node(
                    self.descriptor(),
                    node,
                    format!("Prefer method syntax: `{receiver}.push_back(...)`"),
                );
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

                ctx.report_node(
                    self.descriptor(),
                    node,
                    format!("Prefer method syntax: `{receiver}.length()`"),
                );
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
    fix: FixDescriptor::none(),
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

                ctx.report_node(
                    self.descriptor(),
                    node,
                    format!("Prefer method syntax: `{}.{}(...)`", clean_receiver, method),
                );
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
    fix: FixDescriptor::none(),
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
                ctx.report_node(
                    self.descriptor(),
                    node,
                    "Functions should not be both `public` and `entry`; remove one of the modifiers",
                );
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
    fix: FixDescriptor::none(),
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
                    if let Some(message) = needs_mut_tx_context(slice(source, ty)) {
                        ctx.report_node(self.descriptor(), ty, message);
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
    fix: FixDescriptor::none(),
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
                ctx.report_node(
                    self.descriptor(),
                    node,
                    "Use `loop { ... }` for infinite loops instead of `while (true)`",
                );
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

// ============================================================================
// PureFunctionTransferLint - Preview (Medium FP Risk)
// ============================================================================

pub struct PureFunctionTransferLint;

static PURE_FUNCTION_TRANSFER: LintDescriptor = LintDescriptor {
    name: "pure_function_transfer",
    category: LintCategory::Suspicious,
    description: "Non-entry functions should not call transfer internally; return the object instead",
    group: RuleGroup::Preview,
    fix: FixDescriptor::none(),
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
    description: "Potential integer overflow/underflow without bounds check",
    group: RuleGroup::Preview,
    fix: FixDescriptor::none(),
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
