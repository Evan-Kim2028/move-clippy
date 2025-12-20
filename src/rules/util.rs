use crate::diagnostics::Position;
use tree_sitter::Node;

pub(crate) fn walk(node: Node, f: &mut impl FnMut(Node)) {
    f(node);
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk(child, f);
    }
}

pub(crate) fn slice<'a>(source: &'a str, node: Node) -> &'a str {
    let start = node.start_byte();
    let end = node.end_byte();
    // tree-sitter byte offsets always refer to UTF-8 byte indices; slicing may
    // panic if offsets don't align to char boundaries, so we fall back to a safe
    // empty string in that (unexpected) case.
    source.get(start..end).unwrap_or("")
}

// Unused function - kept for potential future use
#[allow(dead_code)]
pub(crate) fn extract_braced_items(text: &str) -> Option<&str> {
    let open = text.find('{')?;
    let close = text[open + 1..].find('}')? + (open + 1);
    text.get(open + 1..close)
}

pub(crate) fn compact_ws(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

pub(crate) fn split_call(text: &str) -> Option<(&str, &str)> {
    let open = text.find('(')?;
    let close = find_matching_paren(text, open)?;
    if text[close + 1..].trim().is_empty() {
        let callee = text[..open].trim();
        let args = &text[open + 1..close];
        Some((callee, args))
    } else {
        None
    }
}

fn find_matching_paren(text: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (i, c) in text.char_indices().skip(open) {
        match c {
            '(' => depth += 1,
            ')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

pub(crate) fn split_args(args: &str) -> Option<Vec<&str>> {
    // Conservative parser: if we see generic angle brackets, bail to avoid mis-parsing.
    if args.contains('<') || args.contains('>') {
        return None;
    }

    let mut out = Vec::new();
    let mut depth_paren = 0usize;
    let mut depth_brace = 0usize;
    let mut depth_bracket = 0usize;
    let mut in_string = false;

    let mut start = 0usize;
    let bytes = args.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        match b {
            b'"' => {
                let escaped = i > 0 && bytes[i - 1] == b'\\';
                if !escaped {
                    in_string = !in_string;
                }
            }
            b'(' if !in_string => depth_paren += 1,
            b')' if !in_string => depth_paren = depth_paren.checked_sub(1)?,
            b'{' if !in_string => depth_brace += 1,
            b'}' if !in_string => depth_brace = depth_brace.checked_sub(1)?,
            b'[' if !in_string => depth_bracket += 1,
            b']' if !in_string => depth_bracket = depth_bracket.checked_sub(1)?,
            b',' if !in_string && depth_paren == 0 && depth_brace == 0 && depth_bracket == 0 => {
                out.push(args.get(start..i)?.trim());
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }

    let tail = args.get(start..)?.trim();
    if !tail.is_empty() {
        out.push(tail);
    }

    Some(out)
}

pub(crate) fn parse_ref_mut_ident(arg: &str) -> Option<&str> {
    let t = arg.trim();
    let rest = t.strip_prefix("&mut")?.trim();
    if is_simple_ident(rest) {
        Some(rest)
    } else {
        None
    }
}

pub(crate) fn parse_ref_ident(arg: &str) -> Option<&str> {
    let t = arg.trim();
    if t.starts_with("&mut") {
        return None;
    }
    let rest = t.strip_prefix('&')?.trim();
    if is_simple_ident(rest) {
        Some(rest)
    } else {
        None
    }
}

pub(crate) fn is_simple_ident(text: &str) -> bool {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return false;
    }
    chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

pub(crate) fn is_exact_test_attr(text: &str) -> bool {
    text.trim() == "#[test]"
}

pub(crate) fn is_expected_failure_attr(text: &str) -> bool {
    let t = text.trim();
    t.starts_with("#[expected_failure") && t.ends_with(']') && !t.contains(',')
}

pub(crate) fn is_only_whitespace_between(source: &str, start: usize, end: usize) -> bool {
    source
        .get(start..end)
        .unwrap_or("")
        .chars()
        .all(|c| c.is_whitespace())
}

pub(crate) fn position_from_byte_offset(source: &str, byte_offset: usize) -> Position {
    let mut row = 1usize;
    let mut col = 1usize;

    let end = byte_offset.min(source.len());
    for b in source.as_bytes().iter().take(end) {
        if *b == b'\n' {
            row += 1;
            col = 1;
        } else {
            col += 1;
        }
    }

    Position { row, column: col }
}

/// Generate method call fix for function-to-method syntax transformations.
/// Shared by prefer_vector_methods and modern_method_syntax.
///
/// Example: `vector::push_back(&mut v, x)` → `v.push_back(x)`
pub(crate) fn generate_method_call_fix(
    receiver: &str,
    method: &str,
    remaining_args: Vec<&str>,
) -> String {
    let args_str = if remaining_args.is_empty() {
        String::new()
    } else {
        remaining_args.join(", ")
    };

    if args_str.is_empty() {
        format!("{}.{}()", receiver, method)
    } else {
        format!("{}.{}({})", receiver, method, args_str)
    }
}

/// Check if a receiver expression is safe for method call transformation.
/// Only apply auto-fixes to simple identifiers to avoid breaking complex expressions.
pub(crate) fn is_simple_receiver(receiver: &str) -> bool {
    is_simple_ident(receiver)
}

/// Check if this is a test-only module based on attributes and naming conventions.
///
/// Returns true if:
/// - Module has `#[test_only]` attribute
/// - Module name contains `_tests` or `_test`
pub(crate) fn is_test_only_module(root: tree_sitter::Node, source: &str) -> bool {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        // Check for #[test_only] attribute
        if child.kind() == "attribute" {
            let text = slice(source, child);
            if text.contains("test_only") {
                return true;
            }
        }
        // Also check annotations (different grammar node type)
        if child.kind() == "annotation" {
            let text = slice(source, child);
            if text.contains("test_only") {
                return true;
            }
        }
        // Check the module definition name for test naming patterns
        if child.kind() == "module_definition" {
            let name = slice(source, child);
            if name.contains("_tests") || name.contains("_test") {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_source(source: &str) -> tree_sitter::Tree {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(tree_sitter_move::language())
            .expect("Error loading Move grammar");
        parser.parse(source, None).expect("Error parsing source")
    }

    #[test]
    fn test_is_test_only_module_with_attribute() {
        let source = r#"
            #[test_only]
            module test_pkg::my_module {
                fun helper() {}
            }
        "#;
        let tree = parse_source(source);
        assert!(is_test_only_module(tree.root_node(), source));
    }

    #[test]
    fn test_is_test_only_module_with_test_suffix() {
        let source = r#"
            module test_pkg::my_module_tests {
                fun test_something() {}
            }
        "#;
        let tree = parse_source(source);
        assert!(is_test_only_module(tree.root_node(), source));

        let source2 = r#"
            module test_pkg::my_module_test {
                fun test_something() {}
            }
        "#;
        let tree2 = parse_source(source2);
        assert!(is_test_only_module(tree2.root_node(), source2));
    }

    #[test]
    fn test_is_test_only_module_regular_module() {
        let source = r#"
            module my_pkg::my_module {
                public fun do_something(): u64 { 42 }
            }
        "#;
        let tree = parse_source(source);
        assert!(!is_test_only_module(tree.root_node(), source));
    }

    #[test]
    fn test_is_test_only_module_contest_not_test() {
        // "contest" contains "test" but shouldn't match
        let source = r#"
            module my_pkg::contest {
                public fun participate() {}
            }
        "#;
        let tree = parse_source(source);
        // Note: current implementation uses contains("_test") so this correctly returns false
        assert!(!is_test_only_module(tree.root_node(), source));
    }
}
