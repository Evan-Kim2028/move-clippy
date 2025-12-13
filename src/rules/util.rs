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
