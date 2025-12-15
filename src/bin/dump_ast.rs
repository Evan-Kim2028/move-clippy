#!/usr/bin/env rust
//! AST dumping tool for debugging tree-sitter parsing

use std::env;
use std::fs;

fn print_tree(node: tree_sitter::Node, source: &str, indent: usize) {
    let indent_str = "  ".repeat(indent);
    let kind = node.kind();

    // Get node text (truncate if too long)
    let text = &source[node.byte_range()];
    let text_display = if text.len() > 50 {
        format!("{}...", &text[..50])
    } else {
        text.to_string()
    };
    let text_display = text_display.replace('\n', "\\n");

    println!("{}{}  \"{}\"", indent_str, kind, text_display);

    // Recursively print children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        print_tree(child, source, indent + 1);
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: dump_ast <file.move>");
        std::process::exit(1);
    }

    let file_path = &args[1];
    let source = fs::read_to_string(file_path).unwrap_or_else(|e| {
        eprintln!("Failed to read {}: {}", file_path, e);
        std::process::exit(1);
    });

    // Parse with tree-sitter-move
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(tree_sitter_move::language())
        .expect("Failed to set language");

    let tree = parser.parse(&source, None).expect("Failed to parse");

    println!("AST for {}:", file_path);
    println!("================");
    print_tree(tree.root_node(), &source, 0);
}
