//! Shared helpers for parsing common tree-sitter string patterns.

use super::util::is_simple_ident;

fn trim_trailing_semicolon(s: &str) -> &str {
    s.trim_end_matches(|c: char| c == ';' || c.is_whitespace())
}

fn strip_balanced_parens(s: &str) -> &str {
    let mut slice = s.trim();
    loop {
        let bytes = slice.as_bytes();
        if bytes.len() >= 2 && bytes[0] == b'(' && bytes[bytes.len() - 1] == b')' {
            let inner = &slice[1..slice.len() - 1];
            if is_parentheses_balanced(inner) {
                slice = inner.trim();
                continue;
            }
        }
        return slice;
    }
}

fn is_parentheses_balanced(s: &str) -> bool {
    let mut depth = 0i32;
    for c in s.chars() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0
}

/// Extract the condition expression from `assert!(cond)` forms.
pub fn extract_assert_condition(text: &str) -> Option<&str> {
    let start = text.find("assert!")? + 7;
    let rest = trim_trailing_semicolon(text.get(start..)?.trim_start());
    if !rest.starts_with('(') {
        return None;
    }

    let mut depth = 0usize;
    let mut open_idx = None;
    for (idx, ch) in rest.char_indices() {
        match ch {
            '(' => {
                depth += 1;
                if depth == 1 {
                    open_idx = Some(idx + 1);
                }
            }
            ')' => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
                if depth == 0 {
                    let start_idx = open_idx?;
                    let inner = rest.get(start_idx..idx)?.trim();
                    let mut depth_inner = 0usize;
                    for (i, c) in inner.char_indices() {
                        match c {
                            '(' | '[' | '{' => depth_inner += 1,
                            ')' | ']' | '}' => depth_inner = depth_inner.saturating_sub(1),
                            ',' if depth_inner == 0 => {
                                let cond = inner.get(..i)?.trim();
                                return Some(strip_balanced_parens(cond));
                            }
                            _ => {}
                        }
                    }
                    return Some(strip_balanced_parens(inner));
                }
            }
            _ => {}
        }
    }
    None
}

/// Return true if the expression is a simple equality comparison.
pub fn is_simple_equality_comparison(expr: &str) -> bool {
    let parts: Vec<&str> = expr.split("==").collect();
    if parts.len() != 2 {
        return false;
    }

    let left = parts[0].trim();
    let right = parts[1].trim();
    is_simple_expression(left) && is_simple_expression(right)
}

/// Conservative check for simple expressions (identifiers, literals, field access).
pub fn is_simple_expression(expr: &str) -> bool {
    let trimmed = expr.trim();
    if trimmed.is_empty() {
        return false;
    }

    if is_simple_ident(trimmed) {
        return true;
    }

    if trimmed.contains('.') && !trimmed.contains('(') {
        return trimmed
            .split('.')
            .all(|part| is_simple_ident(part.trim()) && !part.trim().is_empty());
    }

    if trimmed.contains("::") && !trimmed.contains('(') {
        return trimmed
            .split("::")
            .all(|part| is_simple_ident(part.trim()) && !part.trim().is_empty());
    }

    if trimmed.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }

    if let Some(rest) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        return !rest.is_empty() && rest.chars().all(|c| c.is_ascii_hexdigit());
    }

    matches!(trimmed, "true" | "false")
}

/// Extract the receiver from `foo.is_some()` style expressions.
pub fn extract_is_some_receiver(condition: &str) -> Option<&str> {
    let trimmed = strip_balanced_parens(trim_trailing_semicolon(condition));
    let receiver = trimmed.strip_suffix(".is_some()")?.trim();
    if is_simple_ident(receiver) {
        Some(receiver)
    } else {
        None
    }
}

/// Parse comparisons like `i < vec.length()` into iterator + collection names.
pub fn parse_length_comparison(condition: &str) -> Option<(&str, &str)> {
    let trimmed = strip_balanced_parens(trim_trailing_semicolon(condition));

    let parts: Vec<&str> = trimmed.split('<').collect();
    if parts.len() != 2 {
        return None;
    }

    let iter_var = parts[0].trim();
    let length_call = parts[1].trim();
    let vec_var = length_call.strip_suffix(".length()")?.trim();

    if is_simple_ident(iter_var) && is_simple_ident(vec_var) {
        Some((iter_var, vec_var))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_assert_condition() {
        let src = "assert!(foo == bar, 0);";
        assert_eq!(extract_assert_condition(src), Some("foo == bar"));
        let nested = "assert!(   (vector::length(v) == 0), \"\" );";
        assert_eq!(
            extract_assert_condition(nested),
            Some("vector::length(v) == 0")
        );
    }

    #[test]
    fn test_simple_expression_checks() {
        assert!(is_simple_expression("foo"));
        assert!(is_simple_expression("sui::coin::Coin"));
        assert!(is_simple_expression("foo.bar"));
        assert!(!is_simple_expression("foo(bar)"));
        assert!(!is_simple_expression("(foo, bar)"));
    }

    #[test]
    fn test_is_some_receiver() {
        assert_eq!(extract_is_some_receiver("foo.is_some()"), Some("foo"));
        assert_eq!(extract_is_some_receiver("(foo.is_some());"), Some("foo"));
        assert!(extract_is_some_receiver("foo.bar.is_some()").is_none());
    }

    #[test]
    fn test_parse_length_comparison() {
        assert_eq!(
            parse_length_comparison("i < vec.length()"),
            Some(("i", "vec"))
        );
        assert_eq!(
            parse_length_comparison("( idx   < data.length() );"),
            Some(("idx", "data"))
        );
        assert!(parse_length_comparison("i <= vec.length()").is_none());
    }
}
