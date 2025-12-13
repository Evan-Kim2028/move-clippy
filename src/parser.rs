use anyhow::{Context, Result};
use tree_sitter::{Language, Parser, Tree};

fn move_language() -> Language {
    tree_sitter_move::language()
}

pub fn parse_source(source: &str) -> Result<Tree> {
    let mut parser = Parser::new();
    parser
        .set_language(move_language())
        .context("failed to load Move grammar")?;

    parser
        .parse(source, None)
        .context("tree-sitter failed to parse source")
}
