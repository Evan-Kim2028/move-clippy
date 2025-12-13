use crate::lint::{LintCategory, LintContext, LintDescriptor, LintRule};
use tree_sitter::Node;

use super::util::{
    compact_ws, is_simple_ident, parse_ref_ident, parse_ref_mut_ident, slice, split_args,
    split_call, walk,
};

pub struct ModernModuleSyntaxLint;

static MODERN_MODULE_SYNTAX: LintDescriptor = LintDescriptor {
    name: "modern_module_syntax",
    category: LintCategory::Modernization,
    description: "Prefer Move 2024 module label syntax (module x::y;) over block form (module x::y { ... })",
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
};

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
            let method = match callee.as_str() {
                "tx_context::sender" => "sender",
                "object::delete" => "delete",
                "coin::into_balance" => "into_balance",
                _ => return,
            };

            let Some(args) = split_args(args_str) else {
                return;
            };
            if args.len() != 1 {
                return;
            }
            let receiver = args[0].trim();
            if !is_simple_ident(receiver) {
                return;
            }

            ctx.report_node(
                self.descriptor(),
                node,
                format!("Prefer method syntax: `{receiver}.{method}()`"),
            );
        });
    }
}
