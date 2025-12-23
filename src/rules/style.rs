use crate::diagnostics::{Applicability, Span, Suggestion};
use crate::lint::{
    AnalysisKind, FixDescriptor, LintCategory, LintContext, LintDescriptor, LintRule, RuleGroup,
};
use tree_sitter::Node;

use super::util::{compact_ws, slice, walk};

// ============================================================================
// AbilitiesOrderLint - P0 (Zero FP)
// ============================================================================

pub struct AbilitiesOrderLint;

static ABILITIES_ORDER: LintDescriptor = LintDescriptor {
    name: "abilities_order",
    category: LintCategory::Style,
    description: "Struct abilities should be ordered: key, copy, drop, store",
    group: RuleGroup::Stable,
    fix: FixDescriptor::safe("Reorder abilities to canonical order"),
    analysis: AnalysisKind::Syntactic,
    gap: None,
};

/// The canonical order of abilities per Sui Move conventions
const ABILITY_ORDER: &[&str] = &["key", "copy", "drop", "store"];

impl LintRule for AbilitiesOrderLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &ABILITIES_ORDER
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        walk(root, &mut |node| {
            // Look for struct definitions with abilities
            if node.kind() != "struct_definition" && node.kind() != "datatype_definition" {
                return;
            }

            // Find the ability_decls child
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "ability_decls" {
                    check_abilities_order(child, source, ctx, &ABILITIES_ORDER);
                }
            }
        });
    }
}

fn check_abilities_order(
    node: Node,
    source: &str,
    ctx: &mut LintContext<'_>,
    lint: &'static LintDescriptor,
) {
    let text = slice(source, node);

    // Extract abilities from the text (e.g., "has key, copy, store" -> ["key", "copy", "store"])
    // Bind the intermediate string to avoid temporary value dropped while borrowed
    let cleaned = text.replace("has", "");
    let abilities: Vec<&str> = cleaned
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && ABILITY_ORDER.contains(s))
        .collect();

    if abilities.len() < 2 {
        return; // Nothing to order
    }

    // Check if abilities are in correct relative order
    let mut last_pos = 0;
    let mut out_of_order = false;

    for ability in &abilities {
        if let Some(pos) = ABILITY_ORDER.iter().position(|&a| a == *ability) {
            if pos < last_pos {
                out_of_order = true;
                break;
            }
            last_pos = pos;
        }
    }

    if out_of_order {
        // Build the correct order
        let mut sorted = abilities.clone();
        sorted.sort_by_key(|a| ABILITY_ORDER.iter().position(|&x| x == *a).unwrap_or(99));

        let replacement = format!("has {}", sorted.join(", "));
        let message = format!(
            "Abilities should be ordered: `has {}`. Found: `has {}`",
            sorted.join(", "),
            abilities.join(", ")
        );

        // Check for suppression before creating diagnostic
        let node_start = node.start_byte();
        if crate::suppression::is_suppressed_at(source, node_start, lint.name) {
            return;
        }

        // Create diagnostic with machine-applicable suggestion
        let diagnostic = crate::diagnostics::Diagnostic {
            lint,
            level: ctx.settings().level_for(lint.name),
            file: None,
            span: Span::from_range(node.range()),
            message,
            help: Some(format!("Reorder to `{}`", replacement)),
            suggestion: Some(Suggestion {
                message: format!("Reorder abilities to `{}`", replacement),
                replacement,
                applicability: Applicability::MachineApplicable,
            }),
        };

        ctx.report_diagnostic_for_node(node, diagnostic);
    }
}

// ============================================================================
// DocCommentStyleLint - P0 (Zero FP)
// ============================================================================

pub struct DocCommentStyleLint;

static DOC_COMMENT_STYLE: LintDescriptor = LintDescriptor {
    name: "doc_comment_style",
    category: LintCategory::Style,
    // Move Book: "Doc Comments Start With `///`"
    // https://move-book.com/guides/code-quality-checklist/#doc-comments-start-with-
    description: "Use `///` for doc comments, not `/** */` or `/* */` (Move Book: code-quality-checklist)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::Syntactic,
    gap: None,
};

impl LintRule for DocCommentStyleLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &DOC_COMMENT_STYLE
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        walk(root, &mut |node| {
            if node.kind() != "block_comment" {
                return;
            }

            let text = slice(source, node).trim();

            // Check if it's a JavaDoc-style or block comment that looks like documentation
            let is_javadoc = text.starts_with("/**");
            let is_block_doc =
                text.starts_with("/*") && !text.starts_with("/**") && looks_like_doc(text);

            if !is_javadoc && !is_block_doc {
                return;
            }

            // Check if this comment precedes a documentable item
            if precedes_documentable_item(node, source) {
                ctx.report_node(
                    self.descriptor(),
                    node,
                    "Use `///` for doc comments instead of block comments",
                );
            }
        });
    }
}

/// Check if the block comment looks like documentation (has multiple lines with asterisks)
fn looks_like_doc(text: &str) -> bool {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() < 2 {
        return false;
    }
    // Check if it follows doc comment patterns (lines starting with * after first line)
    lines
        .iter()
        .skip(1)
        .any(|line| line.trim().starts_with('*'))
}

/// Check if the comment node precedes a documentable item
fn precedes_documentable_item(node: Node, _source: &str) -> bool {
    // Find the next sibling that isn't whitespace
    let mut sibling = node.next_sibling();

    while let Some(s) = sibling {
        let kind = s.kind();

        // Skip whitespace, newlines, and other comments
        if kind == "line_comment" || kind == "block_comment" || kind == "newline" {
            sibling = s.next_sibling();
            continue;
        }

        // Check if it's a documentable item
        return matches!(
            kind,
            "function_definition"
                | "struct_definition"
                | "datatype_definition"
                | "constant"
                | "module_definition"
                | "enum_definition"
                | "spec_block"
                | "use_declaration"
        );
    }

    false
}

// ============================================================================
// ExplicitSelfAssignmentsLint - P0 (Zero FP)
// ============================================================================

pub struct ExplicitSelfAssignmentsLint;

static EXPLICIT_SELF_ASSIGNMENTS: LintDescriptor = LintDescriptor {
    name: "explicit_self_assignments",
    category: LintCategory::Style,
    // Move Book: "Ignored Values In Unpack Can Be Ignored Altogether"
    // https://move-book.com/guides/code-quality-checklist/#ignored-values-in-unpack-can-be-ignored-altogether
    description: "Use `..` to ignore multiple struct fields instead of explicit `: _` bindings (Move Book: code-quality-checklist)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::Syntactic,
    gap: None,
};

impl LintRule for ExplicitSelfAssignmentsLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &EXPLICIT_SELF_ASSIGNMENTS
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        walk(root, &mut |node| {
            // Look for struct unpacking patterns
            let kind = node.kind();
            if kind != "bind_unpack" && kind != "unpack_expression" && kind != "bind_fields" {
                return;
            }

            let text = slice(source, node);

            // Already uses `..` - good!
            if text.contains("..") {
                return;
            }

            // Count `: _` patterns (field being ignored)
            let underscore_count = count_ignored_fields(text);

            // Only flag if 2+ fields are ignored
            if underscore_count >= 2 {
                ctx.report_node(
                    self.descriptor(),
                    node,
                    format!(
                        "Use `..` to ignore {} fields instead of explicit `: _` bindings",
                        underscore_count
                    ),
                );
            }
        });
    }
}

/// Count the number of ignored field patterns (`: _` or just `_` in field position)
fn count_ignored_fields(text: &str) -> usize {
    // Count patterns like `field: _` or `field_name: _`

    // Also count standalone `_` that aren't part of identifiers
    // This is more conservative - we only count `: _` patterns
    text.matches(": _").count()
}

// ============================================================================
// REMOVED: EventSuffixLint
// The Move Book recommends past-tense naming for events (e.g., UserRegistered)
// but does NOT require an "Event" suffix. This lint was not backed by docs.
// ============================================================================

// ============================================================================
// EmptyVectorLiteralLint - Stable (Zero FP)
// ============================================================================

pub struct EmptyVectorLiteralLint;

static EMPTY_VECTOR_LITERAL: LintDescriptor = LintDescriptor {
    name: "empty_vector_literal",
    category: LintCategory::Modernization,
    // Move Book: "Vector Has a Literal. And Associated Functions"
    // https://move-book.com/guides/code-quality-checklist/#vector-has-a-literal-and-associated-functions
    description: "Prefer `vector[]` over `vector::empty()` (Move Book: code-quality-checklist)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::safe("Replace with `vector[]`"),
    analysis: AnalysisKind::Syntactic,
    gap: None,
};

impl LintRule for EmptyVectorLiteralLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &EMPTY_VECTOR_LITERAL
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        walk(root, &mut |node| {
            if node.kind() != "call_expression" {
                return;
            }

            let text = slice(source, node).trim();
            let compact = compact_ws(text);

            // Match vector::empty() or vector::empty<T>()
            if compact.starts_with("vector::empty") && compact.ends_with("()") {
                // Extract type parameter if present
                let type_param = if compact.contains('<') {
                    let start = compact.find('<').unwrap();
                    let end = compact.rfind('>').unwrap();
                    Some(&compact[start..=end])
                } else {
                    None
                };

                let replacement = match type_param {
                    Some(tp) => format!("vector{}", tp),
                    None => "vector[]".to_string(),
                };

                // Check for suppression before creating diagnostic
                let node_start = node.start_byte();
                if crate::suppression::is_suppressed_at(source, node_start, self.descriptor().name)
                {
                    return;
                }

                // Create diagnostic with machine-applicable suggestion
                let diagnostic = crate::diagnostics::Diagnostic {
                    lint: self.descriptor(),
                    level: ctx.settings().level_for(self.descriptor().name),
                    file: None,
                    span: Span::from_range(node.range()),
                    message: format!("Prefer `{}` over `{}`", replacement, text.trim()),
                    help: Some(format!("Replace with `{}`", replacement)),
                    suggestion: Some(Suggestion {
                        message: format!("Replace `{}` with `{}`", text.trim(), replacement),
                        replacement: replacement.clone(),
                        applicability: Applicability::MachineApplicable,
                    }),
                };

                ctx.report_diagnostic_for_node(node, diagnostic);
            }
        });
    }
}

// ============================================================================
// TypedAbortCodeLint - Stable (Low FP)
// ============================================================================

pub struct TypedAbortCodeLint;

static TYPED_ABORT_CODE: LintDescriptor = LintDescriptor {
    name: "typed_abort_code",
    category: LintCategory::Style,
    description: "Prefer named error constants over numeric abort codes",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::Syntactic,
    gap: None,
};

impl LintRule for TypedAbortCodeLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &TYPED_ABORT_CODE
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        // Skip test modules entirely
        if is_test_only_module(root, source) {
            return;
        }

        walk(root, &mut |node| {
            // Skip test functions
            if is_inside_test_function(node, source) {
                return;
            }

            // Check abort statements
            if node.kind() == "abort_expression"
                && let Some(code_node) = node.child_by_field_name("value")
            {
                let code_text = slice(source, code_node).trim();
                if is_numeric_literal(code_text) {
                    ctx.report_node(
                        self.descriptor(),
                        node,
                        format!(
                            "Prefer named error constant over numeric abort code `{}`",
                            code_text
                        ),
                    );
                }
            }

            // Check assert! with numeric abort codes
            if node.kind() == "macro_call_expression" {
                let text = slice(source, node).trim();
                if text.starts_with("assert!")
                    && !text.starts_with("assert_eq!")
                    && !text.starts_with("assert_ne!")
                    && let Some(abort_code) = extract_assert_abort_code_for_typed(text)
                    && is_numeric_literal(abort_code)
                {
                    ctx.report_node(
                        self.descriptor(),
                        node,
                        format!(
                            "Prefer named error constant over numeric abort code `{}`",
                            abort_code
                        ),
                    );
                }
            }
        });
    }
}

/// Extract the abort code from an assert! macro call
fn extract_assert_abort_code_for_typed(text: &str) -> Option<&str> {
    // Find assert!(condition, CODE) pattern
    let rest = text.strip_prefix("assert!")?;
    let inner_start = rest.find('(')?;
    let inner_end = rest.rfind(')')?;
    let inner = rest.get(inner_start + 1..inner_end)?.trim();

    // Find the last comma at depth 0 to get the abort code
    let mut depth: usize = 0;
    let mut last_comma = None;

    for (i, c) in inner.char_indices() {
        match c {
            '(' | '[' | '{' | '<' => depth += 1,
            ')' | ']' | '}' | '>' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => last_comma = Some(i),
            _ => {}
        }
    }

    if let Some(comma_pos) = last_comma {
        let abort_code = inner.get(comma_pos + 1..)?.trim();
        Some(abort_code)
    } else {
        None // No abort code provided
    }
}

/// Check if text is a numeric literal
fn is_numeric_literal(s: &str) -> bool {
    let trimmed = s.trim();
    // Check for decimal literals
    if trimmed.parse::<u64>().is_ok() {
        return true;
    }
    // Check for hex literals
    if let Some(hex) = trimmed.strip_prefix("0x") {
        return u64::from_str_radix(hex, 16).is_ok();
    }
    false
}

/// Check if this is a test-only module
fn is_test_only_module(root: Node, source: &str) -> bool {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "attribute" {
            let text = slice(source, child);
            if text.contains("test_only") {
                return true;
            }
        }
        // Also check the module definition
        if child.kind() == "module_definition" {
            let name = slice(source, child);
            if name.contains("_tests") || name.contains("_test") {
                return true;
            }
        }
    }
    false
}

/// Check if node is inside a test function
fn is_inside_test_function(node: Node, source: &str) -> bool {
    let mut parent = node.parent();
    while let Some(p) = parent {
        if p.kind() == "function_definition" {
            // Check for #[test] attribute
            let mut sibling = p.prev_sibling();
            while let Some(sib) = sibling {
                if sib.kind() == "attribute" {
                    let text = slice(source, sib);
                    if text.contains("test") {
                        return true;
                    }
                }
                sibling = sib.prev_sibling();
            }
            break;
        }
        parent = p.parent();
    }
    false
}

// ============================================================================
// Existing lints below
// ============================================================================

pub struct RedundantSelfImportLint;

static REDUNDANT_SELF_IMPORT: LintDescriptor = LintDescriptor {
    name: "redundant_self_import",
    category: LintCategory::Style,
    // Move Book: "No Single Self in use Statements"
    // https://move-book.com/guides/code-quality-checklist/#no-single-self-in-use-statements
    description: "Use `pkg::mod` instead of `pkg::mod::{Self}` (Move Book: code-quality-checklist)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::safe("Remove redundant `{Self}`"),
    analysis: AnalysisKind::Syntactic,
    gap: None,
};

impl LintRule for RedundantSelfImportLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &REDUNDANT_SELF_IMPORT
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        walk(root, &mut |node| {
            if node.kind() != "use_declaration" {
                return;
            }

            let text = slice(source, node);

            // Check for pattern: ::{Self} at the end with no other members
            // This matches `use pkg::mod::{Self};` but not `use pkg::mod::{Self, Other};`
            if text.contains("::{Self}") || text.contains("::{ Self }") {
                // Count members in the braces
                let brace_content = text
                    .split("::{")
                    .nth(1)
                    .and_then(|s| s.strip_suffix("};"))
                    .or_else(|| {
                        text.split("::{ ")
                            .nth(1)
                            .and_then(|s| s.strip_suffix(" };"))
                    })
                    .unwrap_or("");

                let members: Vec<&str> = brace_content
                    .split(',')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .collect();

                // Only warn if Self is the only member
                if members.len() == 1 && members[0] == "Self" {
                    let module_path = text
                        .trim_start_matches("use ")
                        .split("::{")
                        .next()
                        .unwrap_or("")
                        .trim();

                    let replacement = format!("use {};", module_path);

                    let diagnostic = crate::diagnostics::Diagnostic {
                        lint: &REDUNDANT_SELF_IMPORT,
                        level: ctx.settings().level_for(REDUNDANT_SELF_IMPORT.name),
                        file: None,
                        span: Span::from_range(node.range()),
                        message: format!(
                            "Redundant `{{Self}}` import. Use `{}` instead.",
                            replacement
                        ),
                        help: Some(format!("Simplify to `{}`", replacement)),
                        suggestion: Some(Suggestion {
                            message: "Remove redundant `{Self}`".to_string(),
                            replacement,
                            applicability: Applicability::MachineApplicable,
                        }),
                    };

                    ctx.report_diagnostic_for_node(node, diagnostic);
                }
            }
        });
    }
}

pub struct PreferToStringLint;

static PREFER_TO_STRING: LintDescriptor = LintDescriptor {
    name: "prefer_to_string",
    category: LintCategory::Style,
    // Move Book: "Do Not Import std::string::utf8"
    // https://move-book.com/guides/code-quality-checklist/#do-not-import-stdstringutf8
    description: "Prefer b\"...\".to_string() over std::string::utf8(b\"...\") (Move Book: code-quality-checklist)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::Syntactic,
    gap: None,
};

impl LintRule for PreferToStringLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &PREFER_TO_STRING
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        walk(root, &mut |node| {
            if node.kind() != "use_declaration" {
                return;
            }

            // Near-zero FP: only match the exact import forms.
            let text = slice(source, node);
            let compact = compact_ws(text);
            if compact == "usestd::string::utf8;" || compact == "usestd::string::{utf8};" {
                ctx.report_node(
                    self.descriptor(),
                    node,
                    "Prefer `b\"...\".to_string()` over `std::string::utf8(b\"...\")`",
                );
            }
        });
    }
}

pub struct ConstantNamingLint;

static CONSTANT_NAMING: LintDescriptor = LintDescriptor {
    name: "constant_naming",
    category: LintCategory::Naming,
    // Move Book: "Error Constants are in EPascalCase" + "Regular Constant are ALL_CAPS"
    // https://move-book.com/guides/code-quality-checklist/#error-constants-are-in-epascalcase
    // https://move-book.com/guides/code-quality-checklist/#regular-constant-are-all_caps
    description: "Error constants should use EPascalCase; other constants should be SCREAMING_SNAKE_CASE (Move Book: code-quality-checklist)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::safe("Rename to correct case"),
    analysis: AnalysisKind::Syntactic,
    gap: None,
};

impl LintRule for ConstantNamingLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &CONSTANT_NAMING
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        walk(root, &mut |node| {
            if node.kind() != "constant" {
                return;
            }
            let Some(name_node) = node.child_by_field_name("name") else {
                return;
            };
            let name = slice(source, name_node).trim();
            if name.is_empty() {
                return;
            }

            match classify_constant(name) {
                ConstantKind::Error if !is_valid_error_constant(name) => {
                    let suggested = to_e_pascal_case(name);
                    let message = format!(
                        "Error constants should use EPascalCase (e.g. `ENotAuthorized`), found `{name}`"
                    );

                    // Check for suppression
                    if crate::suppression::is_suppressed_at(
                        source,
                        name_node.start_byte(),
                        self.descriptor().name,
                    ) {
                        return;
                    }

                    let diagnostic = crate::diagnostics::Diagnostic {
                        lint: self.descriptor(),
                        level: ctx.settings().level_for(self.descriptor().name),
                        file: None,
                        span: Span::from_range(name_node.range()),
                        message: message.clone(),
                        help: Some(format!("Consider renaming to `{}`", suggested)),
                        suggestion: Some(Suggestion {
                            message: format!("Rename to `{}`", suggested),
                            replacement: suggested,
                            applicability: Applicability::MaybeIncorrect, // Renaming affects all usages
                        }),
                    };
                    ctx.report_diagnostic_for_node(name_node, diagnostic);
                }
                ConstantKind::Regular if !is_valid_regular_constant(name) => {
                    let suggested = to_screaming_snake_case(name);
                    let message = format!(
                        "Regular constants should be SCREAMING_SNAKE_CASE (e.g. `MAX_SUPPLY`), found `{name}`"
                    );

                    // Check for suppression
                    if crate::suppression::is_suppressed_at(
                        source,
                        name_node.start_byte(),
                        self.descriptor().name,
                    ) {
                        return;
                    }

                    let diagnostic = crate::diagnostics::Diagnostic {
                        lint: self.descriptor(),
                        level: ctx.settings().level_for(self.descriptor().name),
                        file: None,
                        span: Span::from_range(name_node.range()),
                        message: message.clone(),
                        help: Some(format!("Consider renaming to `{}`", suggested)),
                        suggestion: Some(Suggestion {
                            message: format!("Rename to `{}`", suggested),
                            replacement: suggested,
                            applicability: Applicability::MaybeIncorrect, // Renaming affects all usages
                        }),
                    };
                    ctx.report_diagnostic_for_node(name_node, diagnostic);
                }
                _ => {}
            }
        });
    }
}

pub struct UnneededReturnLint;

static UNNEEDED_RETURN: LintDescriptor = LintDescriptor {
    name: "unneeded_return",
    category: LintCategory::Style,
    description: "Avoid trailing `return` statements; let the final expression return implicitly",
    group: RuleGroup::Stable,
    fix: FixDescriptor::safe("Remove `return` keyword"),
    analysis: AnalysisKind::Syntactic,
    gap: None,
};

impl LintRule for UnneededReturnLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &UNNEEDED_RETURN
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        walk(root, &mut |node| {
            if node.kind() != "function_definition" {
                return;
            }

            // Find the function body block
            // Try field name first, then iterate children
            let body = node.child_by_field_name("body").or_else(|| {
                let mut cursor = node.walk();
                node.children(&mut cursor).find(|c| c.kind() == "block")
            });

            let Some(body) = body else {
                return;
            };

            if body.kind() != "block" {
                return;
            }

            if let Some(ret) = trailing_return_expression(body) {
                // Extract the expression after "return"
                let ret_text = slice(source, ret);

                // The return expression looks like "return expr" or "return expr;"
                // We want to extract just "expr"
                let replacement = if let Some(stripped) = ret_text.strip_prefix("return") {
                    let expr = stripped.trim();
                    // Remove trailing semicolon if present (it's part of block_item, not return_expression)
                    expr.trim_end_matches(';').trim().to_string()
                } else {
                    // Fallback: just report without fix
                    ctx.report_node(
                        self.descriptor(),
                        ret,
                        "Remove `return`; the last expression in a block already returns implicitly",
                    );
                    return;
                };

                // Check for suppression
                let node_start = ret.start_byte();
                if crate::suppression::is_suppressed_at(source, node_start, self.descriptor().name)
                {
                    return;
                }

                // Create diagnostic with machine-applicable suggestion
                let diagnostic = crate::diagnostics::Diagnostic {
                    lint: self.descriptor(),
                    level: ctx.settings().level_for(self.descriptor().name),
                    file: None,
                    span: Span::from_range(ret.range()),
                    message:
                        "Remove `return`; the last expression in a block already returns implicitly"
                            .to_string(),
                    help: Some(format!("Replace with `{}`", replacement)),
                    suggestion: Some(Suggestion {
                        message: "Remove `return` keyword".to_string(),
                        replacement,
                        applicability: Applicability::MachineApplicable,
                    }),
                };

                ctx.report_diagnostic_for_node(ret, diagnostic);
            }
        });
    }
}

enum ConstantKind {
    Error,
    Regular,
}

fn classify_constant(name: &str) -> ConstantKind {
    if is_error_like(name) {
        ConstantKind::Error
    } else {
        ConstantKind::Regular
    }
}

fn is_error_like(name: &str) -> bool {
    let mut chars = name.chars();
    match (chars.next(), chars.next()) {
        (Some('E'), Some(second)) if second.is_ascii_uppercase() => {
            name.chars().skip(1).any(|c| c.is_ascii_lowercase())
        }
        _ => false,
    }
}

fn is_valid_error_constant(name: &str) -> bool {
    if !name.starts_with('E') || name.contains('_') {
        return false;
    }
    let mut chars = name.chars();
    chars.next(); // drop leading E
    match chars.next() {
        Some(second) if second.is_ascii_uppercase() => {
            name.chars().skip(1).all(|c| c.is_ascii_alphanumeric())
        }
        _ => false,
    }
}

fn is_valid_regular_constant(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_uppercase() || first == '_') {
        return false;
    }
    name.chars()
        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// Convert a name to EPascalCase (e.g., E_NOT_AUTHORIZED -> ENotAuthorized)
fn to_e_pascal_case(name: &str) -> String {
    // If already starts with E, keep it; otherwise add it
    let without_e = name.strip_prefix('E').unwrap_or(name);

    // Split on underscores and capitalize each word
    let parts: Vec<String> = without_e
        .split('_')
        .filter(|s| !s.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                }
            }
        })
        .collect();

    format!("E{}", parts.join(""))
}

/// Convert a name to SCREAMING_SNAKE_CASE (e.g., maxSupply -> MAX_SUPPLY)
fn to_screaming_snake_case(name: &str) -> String {
    let mut result = String::new();
    let mut prev_was_lowercase = false;

    for (i, ch) in name.chars().enumerate() {
        if ch == '_' {
            result.push('_');
            prev_was_lowercase = false;
        } else if ch.is_ascii_uppercase() {
            // Add underscore before uppercase if previous was lowercase (camelCase boundary)
            if i > 0 && prev_was_lowercase {
                result.push('_');
            }
            result.push(ch);
            prev_was_lowercase = false;
        } else if ch.is_ascii_lowercase() {
            result.push(ch.to_ascii_uppercase());
            prev_was_lowercase = true;
        } else {
            result.push(ch);
            prev_was_lowercase = false;
        }
    }

    result
}

fn trailing_return_expression(block: Node) -> Option<Node> {
    let count = block.named_child_count();
    if count == 0 {
        return None;
    }
    let last = block.named_child(count - 1)?;
    match last.kind() {
        "block_item" => {
            let expr = last.named_child(0)?;
            (expr.kind() == "return_expression").then_some(expr)
        }
        "return_expression" => Some(last),
        _ => None,
    }
}

// ============================================================================
// ErrorConstNamingLint - P0 (Zero FP)
// ============================================================================

/// Detects error constants that don't follow EPascalCase naming.
///
/// From [Move Book Code Quality Checklist](https://move-book.com/guides/code-quality-checklist/):
/// > Error Constants are in `EPascalCase`
///
/// # Example
///
/// ```move
/// // bad! all-caps are used for regular constants
/// const NOT_AUTHORIZED: u64 = 0;
///
/// // good! clear indication it's an error constant
/// const ENotAuthorized: u64 = 0;
/// ```
pub struct ErrorConstNamingLint;

static ERROR_CONST_NAMING: LintDescriptor = LintDescriptor {
    name: "error_const_naming",
    category: LintCategory::Style,
    // Move Book: "Error Constants are in EPascalCase"
    // https://move-book.com/guides/code-quality-checklist/#error-constants-are-in-epascalcase
    description: "Error constants should use EPascalCase (e.g., `ENotAuthorized`) (Move Book: code-quality-checklist)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(), // Renaming requires updating all usages
    analysis: AnalysisKind::Syntactic,
    gap: None,
};

impl LintRule for ErrorConstNamingLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &ERROR_CONST_NAMING
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        // First pass: collect constants used in abort/assert
        let mut error_consts: std::collections::HashSet<String> = std::collections::HashSet::new();

        walk(root, &mut |node| {
            // Look for abort expressions: abort ERROR_CODE
            if node.kind() == "abort_expression"
                && let Some(arg) = node.named_child(0)
            {
                let text = slice(source, arg).trim();
                // If it's a simple identifier (constant reference)
                if arg.kind() == "name_expression" || arg.kind() == "module_access" {
                    error_consts.insert(text.to_string());
                }
            }

            // Look for assert! macro: assert!(condition, ERROR_CODE)
            if node.kind() == "macro_call" {
                let text = slice(source, node);
                if text.starts_with("assert!")
                    && let Some(args) = node.child_by_field_name("arguments")
                {
                    let mut cursor = args.walk();
                    let children: Vec<_> = args.children(&mut cursor).collect();
                    // Find the comma and get the expression after it
                    for (i, child) in children.iter().enumerate() {
                        if slice(source, *child) == ","
                            && let Some(next) = children.get(i + 1)
                        {
                            let err_text = slice(source, *next).trim();
                            error_consts.insert(err_text.to_string());
                        }
                    }
                }
            }
        });

        // Second pass: check constant definitions
        walk(root, &mut |node| {
            if node.kind() != "constant" {
                return;
            }

            let Some(name_node) = node.child_by_field_name("name") else {
                return;
            };

            let name = slice(source, name_node);

            // Check if this constant is used as an error code
            if !error_consts.contains(name) {
                // Also check if name suggests it's an error (starts with E followed by uppercase,
                // or contains ERROR/ERR in screaming case)
                let looks_like_error = name.starts_with('E')
                    && name
                        .chars()
                        .nth(1)
                        .map(|c| c.is_uppercase())
                        .unwrap_or(false)
                    || name.contains("ERROR")
                    || name.contains("ERR_")
                    || name.starts_with("E_");

                if !looks_like_error {
                    return;
                }
            }

            // This is an error constant - check naming
            if !is_valid_error_const_name(name) {
                let suggested = to_e_pascal_case(name);
                ctx.report_node(
                    &ERROR_CONST_NAMING,
                    name_node,
                    format!(
                        "Error constant `{}` should use EPascalCase. Consider renaming to `{}`.",
                        name, suggested
                    ),
                );
            }
        });
    }
}

/// Check if a name follows EPascalCase (starts with E, followed by PascalCase)
fn is_valid_error_const_name(name: &str) -> bool {
    if !name.starts_with('E') {
        return false;
    }

    let rest = &name[1..];
    if rest.is_empty() {
        return false;
    }

    // First char after E should be uppercase
    let first = rest.chars().next().unwrap();
    if !first.is_uppercase() {
        return false;
    }

    // Should be PascalCase (no underscores, not all caps)
    !rest.contains('_') && !rest.chars().all(|c| c.is_uppercase() || c.is_numeric())
}
