use tree_sitter::Node;

use crate::annotations;

/// Helpers for honoring `#[allow(lint::name)]` and `#![allow(lint::name)]` directives in Move source.
fn is_item_kind(kind: &str) -> bool {
    // Avoid substring matches like `function_identifier` (which would incorrectly anchor directives
    // *inside* an item rather than at the item boundary). Only treat definition/declaration nodes
    // as suppression anchors.
    if !(kind.ends_with("_definition") || kind.ends_with("_declaration")) {
        return false;
    }

    // Be resilient to minor grammar naming differences (e.g., `datatype_definition`).
    kind.contains("module")
        || kind.contains("use")
        || kind.contains("function")
        || kind.contains("struct")
        || kind.contains("datatype")
        || kind.contains("enum")
        || kind.contains("constant")
}

/// Return the byte offset for the enclosing item used as a suppression anchor.
pub fn anchor_item_start_byte(node: Node) -> usize {
    if is_item_kind(node.kind()) {
        return node.start_byte();
    }

    let mut cur = node;
    while let Some(parent) = cur.parent() {
        if is_item_kind(parent.kind()) {
            return parent.start_byte();
        }
        cur = parent;
    }

    node.start_byte()
}

/// Check whether the item starting at `item_start_byte` is suppressed for `lint_name`.
///
/// This checks the attribute/doc block *immediately preceding* the item.
pub fn is_suppressed_at(source: &str, item_start_byte: usize, lint_name: &str) -> bool {
    let scope = annotations::item_scope(source, item_start_byte);
    if scope.is_denied(lint_name) || scope.is_expected(lint_name) {
        return false;
    }
    scope.is_suppressed(lint_name)
}

/// Check whether the file contains a module-level `#![allow(lint::name)]` directive.
///
/// This is intended to support file-level suppression while the tree-sitter Move grammar
/// does not support `#![...]` forms.
pub fn is_module_level_suppressed(source: &str, lint_name: &str) -> bool {
    let scope = annotations::module_scope(source);
    if scope.is_denied(lint_name) || scope.is_expected(lint_name) {
        return false;
    }
    scope.is_suppressed(lint_name)
}

/// Check whether `node` is suppressed for `lint_name`.
///
/// This applies both:
/// - item-level `#[allow(lint::...)]` anchored at the enclosing item, and
/// - module/file-level `#![allow(lint::...)]` at the file header.
pub fn is_suppressed(node: Node, source: &str, lint_name: &str) -> bool {
    let anchor = anchor_item_start_byte(node);
    is_suppressed_at(source, anchor, lint_name) || is_module_level_suppressed(source, lint_name)
}

/// Check suppression using both item-level and module-level rules.
pub fn is_suppressed_at_any(source: &str, item_start_byte: usize, lint_name: &str) -> bool {
    is_suppressed_at(source, item_start_byte, lint_name)
        || is_module_level_suppressed(source, lint_name)
}
