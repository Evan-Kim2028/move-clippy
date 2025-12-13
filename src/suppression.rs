use tree_sitter::Node;

fn is_item_kind(kind: &str) -> bool {
    if kind == "module_definition" || kind == "use_declaration" {
        return true;
    }

    // Be resilient to grammar naming differences.
    kind.contains("function")
        || kind.contains("struct")
        || kind.contains("enum")
        || kind.contains("constant")
}

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

pub fn is_suppressed_at(source: &str, item_start_byte: usize, lint_name: &str) -> bool {
    let Some(before_item) = source.get(..item_start_byte) else {
        return false;
    };

    // Keep the scan local to avoid accidentally picking up earlier unrelated allow attributes.
    let mut start = before_item.len().saturating_sub(4096);
    while start > 0 && !before_item.is_char_boundary(start) {
        start -= 1;
    }
    let window = &before_item[start..];
    let mut lines = window.lines().rev();

    let needle = format!("#[allow(lint::{lint_name})]");

    for line in &mut lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Treat doc comments as part of the item's attribute block.
        let is_doc = trimmed.starts_with("///")
            || trimmed.starts_with("/**")
            || trimmed.starts_with('*')
            || trimmed.starts_with("*/");
        if is_doc {
            continue;
        }

        if trimmed.starts_with("#[") {
            let compact: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
            if compact == needle {
                return true;
            }
            continue;
        }

        // Any other line means we've left the attribute/doc block.
        break;
    }

    false
}
