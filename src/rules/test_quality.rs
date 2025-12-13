use crate::diagnostics::Span;
use crate::lint::{FixDescriptor, LintCategory, LintContext, LintDescriptor, LintRule, RuleGroup};
use crate::suppression;
use tree_sitter::Node;

use super::util::{
    is_exact_test_attr, is_expected_failure_attr, is_only_whitespace_between,
    position_from_byte_offset, slice, walk,
};

// ============================================================================
// TestAbortCodeLint - P0 (Zero FP)
// ============================================================================

pub struct TestAbortCodeLint;

static TEST_ABORT_CODE: LintDescriptor = LintDescriptor {
    name: "test_abort_code",
    category: LintCategory::TestQuality,
    description: "Avoid numeric abort codes in test assertions; they may collide with application error codes",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
};

impl LintRule for TestAbortCodeLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &TEST_ABORT_CODE
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        // Track if we're in a test module or test function
        let in_test_module = is_test_only_module(root, source);

        walk(root, &mut |node| {
            // Only check macro calls
            if node.kind() != "macro_invocation" {
                return;
            }

            let text = slice(source, node).trim();

            // Only check assert! macros
            if !text.starts_with("assert!") {
                return;
            }

            // Don't flag assert_eq!, assert_ne!, etc.
            if text.starts_with("assert_eq!") || text.starts_with("assert_ne!") {
                return;
            }

            // Must be in test context
            let in_test_fn = is_inside_test_function(node, source);
            if !in_test_fn && !in_test_module {
                return;
            }

            // Parse assert!(condition, CODE) - look for numeric second arg
            if let Some(abort_code) = extract_assert_abort_code(text) {
                if is_numeric_literal(abort_code) {
                    ctx.report_node(
                        self.descriptor(),
                        node,
                        "Avoid numeric abort codes in test assertions; use `assert!(cond)` or a named constant",
                    );
                }
            }
        });
    }
}

/// Check if a module has #[test_only] attribute
fn is_test_only_module(root: Node, source: &str) -> bool {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "attributes" || child.kind() == "attribute" {
            let text = slice(source, child);
            if text.contains("test_only") {
                return true;
            }
        }
    }
    false
}

/// Check if a node is inside a #[test] function
fn is_inside_test_function(node: Node, source: &str) -> bool {
    let mut current = node.parent();
    while let Some(parent) = current {
        if parent.kind() == "function_definition" {
            // Check for #[test] attribute
            let mut cursor = parent.walk();
            for child in parent.children(&mut cursor) {
                if child.kind() == "attributes" || child.kind() == "attribute" {
                    let text = slice(source, child);
                    if text.contains("#[test") {
                        return true;
                    }
                }
            }
            return false;
        }
        current = parent.parent();
    }
    false
}

/// Extract the abort code from assert!(cond, code)
fn extract_assert_abort_code(text: &str) -> Option<&str> {
    // Find the opening paren after assert!
    let start = text.find("assert!")? + 7;
    let rest = text.get(start..)?.trim();
    
    if !rest.starts_with('(') {
        return None;
    }
    
    // Find the content inside parens
    let inner_start = 1;
    let inner_end = rest.rfind(')')?;
    let inner = rest.get(inner_start..inner_end)?.trim();
    
    // Split by comma to find the second argument
    // Be careful of nested parens
    let mut depth: usize = 0;
    let mut last_comma = None;
    
    for (i, c) in inner.char_indices() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => last_comma = Some(i),
            _ => {}
        }
    }
    
    if let Some(comma_pos) = last_comma {
        let abort_code = inner.get(comma_pos + 1..)?.trim();
        Some(abort_code)
    } else {
        None
    }
}

/// Check if a string is a numeric literal (decimal or hex)
fn is_numeric_literal(s: &str) -> bool {
    let trimmed = s.trim();
    
    // Decimal number
    if trimmed.chars().all(|c| c.is_ascii_digit()) && !trimmed.is_empty() {
        return true;
    }
    
    // Hex number
    if let Some(hex) = trimmed.strip_prefix("0x") {
        return hex.chars().all(|c| c.is_ascii_hexdigit()) && !hex.is_empty();
    }
    if let Some(hex) = trimmed.strip_prefix("0X") {
        return hex.chars().all(|c| c.is_ascii_hexdigit()) && !hex.is_empty();
    }
    
    false
}

// ============================================================================
// RedundantTestPrefixLint - P0 (Zero FP)
// ============================================================================

pub struct RedundantTestPrefixLint;

static REDUNDANT_TEST_PREFIX: LintDescriptor = LintDescriptor {
    name: "redundant_test_prefix",
    category: LintCategory::TestQuality,
    description: "In `*_tests` modules, omit redundant `test_` prefix from test functions",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
};

impl LintRule for RedundantTestPrefixLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &REDUNDANT_TEST_PREFIX
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        // First, check if this is a *_tests module
        let module_name = extract_module_name(root, source);
        
        // Only apply in modules ending with _tests
        if !module_name.ends_with("_tests") {
            return;
        }

        walk(root, &mut |node| {
            if node.kind() != "function_definition" {
                return;
            }

            // Check if this function has #[test] attribute
            if !has_test_attribute(node, source) {
                return;
            }

            // Get function name
            let Some(name_node) = node.child_by_field_name("name") else {
                return;
            };
            let fn_name = slice(source, name_node).trim();

            if fn_name.starts_with("test_") {
                let suggested_name = &fn_name[5..]; // Remove "test_" prefix
                ctx.report_node(
                    self.descriptor(),
                    name_node,
                    format!(
                        "In `*_tests` modules, omit `test_` prefix. Consider: `{}`",
                        suggested_name
                    ),
                );
            }
        });
    }
}

/// Extract the module name from the AST
fn extract_module_name(root: Node, source: &str) -> String {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "module_identity" {
            // Get the last part of the module path (the actual module name)
            let text = slice(source, child);
            if let Some(name) = text.split("::").last() {
                return name.trim().to_string();
            }
        }
    }
    String::new()
}

/// Check if a function has a #[test] attribute
fn has_test_attribute(node: Node, source: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "attributes" || child.kind() == "attribute" {
            let text = slice(source, child);
            if text.contains("#[test") {
                return true;
            }
        }
    }
    false
}

// ============================================================================
// MergeTestAttributesLint (existing)
// ============================================================================

pub struct MergeTestAttributesLint;

static MERGE_TEST_ATTRIBUTES: LintDescriptor = LintDescriptor {
    name: "merge_test_attributes",
    category: LintCategory::TestQuality,
    description: "Merge stacked #[test] and #[expected_failure] into a single attribute list",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
};

impl LintRule for MergeTestAttributesLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &MERGE_TEST_ATTRIBUTES
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        let mut attrs: Vec<(usize, usize, usize)> = Vec::new();
        walk(root, &mut |node| {
            let t = slice(source, node).trim();
            if t.starts_with("#[") && t.ends_with(']') {
                let anchor = suppression::anchor_item_start_byte(node);
                attrs.push((node.start_byte(), node.end_byte(), anchor));
            }
        });

        attrs.sort_by_key(|(start, _end, _anchor)| *start);
        for pair in attrs.windows(2) {
            let (a_start, a_end, a_anchor) = pair[0];
            let (b_start, b_end, b_anchor) = pair[1];

            if a_anchor != b_anchor {
                continue;
            }

            let a_text = source.get(a_start..a_end).unwrap_or("");
            let b_text = source.get(b_start..b_end).unwrap_or("");

            if !is_exact_test_attr(a_text) {
                continue;
            }
            if !is_expected_failure_attr(b_text) {
                continue;
            }
            if !is_only_whitespace_between(source, a_end, b_start) {
                continue;
            }

            let span = Span {
                start: position_from_byte_offset(source, a_start),
                end: position_from_byte_offset(source, b_end),
            };

            ctx.report_span_with_anchor(
                self.descriptor(),
                a_anchor,
                span,
                "Merge `#[test]` and `#[expected_failure]` into `#[test, expected_failure]`",
            );
        }
    }
}
