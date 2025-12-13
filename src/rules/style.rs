use crate::diagnostics::Span;
use crate::lint::{LintCategory, LintContext, LintDescriptor, LintRule};
use tree_sitter::Node;

use super::util::{compact_ws, extract_braced_items, slice, walk};

pub struct RedundantSelfImportLint;

static REDUNDANT_SELF_IMPORT: LintDescriptor = LintDescriptor {
    name: "redundant_self_import",
    category: LintCategory::Style,
    description: "Avoid `use pkg::mod::{Self};`; prefer `use pkg::mod;`",
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

            // Zero-FP strategy: only flag the exact single-item brace form `{Self}`.
            // If there are multiple items, aliases, or other syntax, we do nothing.
            let text = slice(source, node);
            let Some(braced) = extract_braced_items(text) else {
                return;
            };

            let items: Vec<&str> = braced
                .split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();

            if items.len() == 1 && items[0] == "Self" {
                ctx.report(
                    self.descriptor(),
                    Span::from_range(node.range()),
                    "Redundant `{Self}` import; prefer `use pkg::mod;`",
                );
            }
        });
    }
}

pub struct PreferToStringLint;

static PREFER_TO_STRING: LintDescriptor = LintDescriptor {
    name: "prefer_to_string",
    category: LintCategory::Style,
    description: "Prefer b\"...\".to_string() over std::string::utf8(b\"...\") (import-only check)",
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
                ctx.report(
                    self.descriptor(),
                    Span::from_range(node.range()),
                    "Prefer `b\"...\".to_string()` over `std::string::utf8(b\"...\")`",
                );
            }
        });
    }
}
