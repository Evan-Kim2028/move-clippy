use anyhow::{Context, Result};
use std::borrow::Cow;
use tree_sitter::{Language, Parser, Tree};

fn move_language() -> Language {
    tree_sitter_move::language()
}

fn should_mask_directive_line(line_trimmed: &[u8]) -> bool {
    // Be tolerant of whitespace within directives (`#![allow( lint::foo )]`), since the
    // tree-sitter grammar tends to turn these into ERROR nodes even when Move accepts them.
    let compact: Vec<u8> = line_trimmed
        .iter()
        .copied()
        .filter(|b| *b != b' ' && *b != b'\t')
        .collect();
    let compact = compact.as_slice();

    compact.starts_with(b"#![allow(lint::")
        || compact.starts_with(b"#![deny(lint::")
        || compact.starts_with(b"#![expect(lint::")
        || compact.starts_with(b"#[ext(move_clippy(allow(")
        || compact.starts_with(b"#[ext(move_clippy(deny(")
        || compact.starts_with(b"#[ext(move_clippy(expect(")
        || compact.starts_with(b"#![ext(move_clippy(allow(")
        || compact.starts_with(b"#![ext(move_clippy(deny(")
        || compact.starts_with(b"#![ext(move_clippy(expect(")
}

/// Mask out move-clippy directives that tree-sitter-move may parse as ERROR nodes.
///
/// We blank out directive lines with spaces while preserving byte length (and newlines) so that:
/// - AST offsets/spans remain aligned with the original source
/// - lints can still scan the original source for suppression directives
fn mask_lint_directive_lines(source: &str) -> Cow<'_, str> {
    let bytes = source.as_bytes();
    if !bytes.windows(3).any(|w| w == b"#![") && !bytes.windows(6).any(|w| w == b"#[ext(") {
        return Cow::Borrowed(source);
    }

    let mut out = bytes.to_vec();

    let mut line_start = 0usize;
    for (idx, &b) in bytes.iter().enumerate() {
        if b == b'\n' {
            let line_end = idx; // exclude '\n'
            let mut trim = line_start;
            while trim < line_end {
                match bytes[trim] {
                    b' ' | b'\t' => trim += 1,
                    _ => break,
                }
            }

            if should_mask_directive_line(&bytes[trim..line_end]) {
                for byte in out.iter_mut().take(line_end).skip(trim) {
                    *byte = b' ';
                }
            }

            line_start = idx + 1;
        }
    }

    // Last line (no trailing newline)
    if line_start < bytes.len() {
        let line_end = bytes.len();
        let mut trim = line_start;
        while trim < line_end {
            match bytes[trim] {
                b' ' | b'\t' => trim += 1,
                _ => break,
            }
        }

        if should_mask_directive_line(&bytes[trim..line_end]) {
            for byte in out.iter_mut().take(line_end).skip(trim) {
                *byte = b' ';
            }
        }
    }

    match String::from_utf8(out) {
        Ok(s) => Cow::Owned(s),
        Err(_) => Cow::Borrowed(source),
    }
}

pub fn parse_source(source: &str) -> Result<Tree> {
    let mut parser = Parser::new();
    parser
        .set_language(move_language())
        .context("failed to load Move grammar")?;

    let masked = mask_lint_directive_lines(source);

    parser
        .parse(masked.as_ref(), None)
        .context("tree-sitter failed to parse source")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contains_error_node(node: tree_sitter::Node) -> bool {
        if node.kind() == "ERROR" {
            return true;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if contains_error_node(child) {
                return true;
            }
        }
        false
    }

    #[test]
    fn parses_module_level_allow_without_error_nodes() {
        let src = r#"#![allow(lint::prefer_vector_methods)]
module my_pkg::m;

use std::vector;

public fun demo() {
    let mut v = vector::empty<u64>();
    vector::push_back(&mut v, 1);
}
"#;

        let tree = parse_source(src).expect("parse should succeed");
        let root = tree.root_node();
        assert!(
            !contains_error_node(root),
            "expected masking to prevent ERROR nodes for #![allow(lint::...)]"
        );
    }

    #[test]
    fn parses_module_level_deny_without_error_nodes() {
        let src = r#"#![deny(lint::prefer_vector_methods)]
module my_pkg::m;

use std::vector;

public fun demo() {
    let mut v = vector::empty<u64>();
    vector::push_back(&mut v, 1);
}
"#;

        let tree = parse_source(src).expect("parse should succeed");
        let root = tree.root_node();
        assert!(
            !contains_error_node(root),
            "expected masking to prevent ERROR nodes for #![deny(lint::...)]"
        );
    }

    #[test]
    fn parses_module_level_expect_without_error_nodes() {
        let src = r#"#![expect(lint::prefer_vector_methods)]
module my_pkg::m;

use std::vector;

public fun demo() {
    let mut v = vector::empty<u64>();
    vector::push_back(&mut v, 1);
}
"#;

        let tree = parse_source(src).expect("parse should succeed");
        let root = tree.root_node();
        assert!(
            !contains_error_node(root),
            "expected masking to prevent ERROR nodes for #![expect(lint::...)]"
        );
    }

    #[test]
    fn masking_preserves_length_and_indentation() {
        let src = "   \t#![allow(lint::prefer_vector_methods)]\nmodule my_pkg::m;\n";
        let masked = mask_lint_directive_lines(src);

        assert_eq!(masked.len(), src.len(), "masking must preserve byte length");
        assert_eq!(
            &masked.as_bytes()[0..4],
            b"   \t",
            "indentation must remain"
        );

        let first_line = masked.lines().next().expect("expected at least one line");
        let replaced = &first_line[4..];
        assert!(
            replaced.as_bytes().iter().all(|b| *b == b' '),
            "expected directive contents to be replaced with spaces"
        );
    }

    #[test]
    fn masking_handles_last_line_without_trailing_newline() {
        let src = "#![expect(lint::prefer_vector_methods)]";
        let masked = mask_lint_directive_lines(src);
        assert_eq!(masked.len(), src.len(), "masking must preserve byte length");
        assert!(
            masked.as_bytes().iter().all(|b| *b == b' '),
            "expected entire directive line to be masked"
        );
    }

    #[test]
    fn parses_ext_directives_without_error_nodes() {
        let src = r#"#[ext(move_clippy(allow(prefer_vector_methods)))]
module my_pkg::m;

use std::vector;

public fun demo() {
    let mut v = vector::empty<u64>();
    vector::push_back(&mut v, 1);
}
"#;

        let tree = parse_source(src).expect("parse should succeed");
        let root = tree.root_node();
        assert!(
            !contains_error_node(root),
            "expected masking to prevent ERROR nodes for #[ext(move_clippy(...))]"
        );
    }

    #[test]
    fn parses_ext_directives_with_deny_and_expect_without_error_nodes() {
        let src = r#"#![ext(move_clippy(deny(prefer_vector_methods)))]
#[ext(move_clippy(expect(prefer_vector_methods)))]
module my_pkg::m;

use std::vector;

public fun demo() {
    let mut v = vector::empty<u64>();
    vector::push_back(&mut v, 1);
}
"#;

        let tree = parse_source(src).expect("parse should succeed");
        let root = tree.root_node();
        assert!(
            !contains_error_node(root),
            "expected masking to prevent ERROR nodes for ext(move_clippy(deny/expect(...)))"
        );
    }

    #[test]
    fn masks_directives_with_extra_whitespace() {
        let src = " \t#![ allow ( lint::prefer_vector_methods ) ]\nmodule my_pkg::m;\n";
        let masked = mask_lint_directive_lines(src);

        assert_eq!(masked.len(), src.len(), "masking must preserve byte length");
        let first_line = masked.lines().next().expect("expected at least one line");
        assert!(
            first_line.trim().is_empty(),
            "expected directive contents to be replaced with spaces"
        );

        let tree = parse_source(src).expect("parse should succeed");
        assert!(
            !contains_error_node(tree.root_node()),
            "expected masking to prevent ERROR nodes for whitespace-heavy directives"
        );
    }
}
