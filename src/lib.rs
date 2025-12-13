pub mod cli;
pub mod diagnostics;
pub mod lint;
pub mod parser;
pub mod rules;
pub mod visitor;

use anyhow::Result;
use tree_sitter::Tree;

use crate::diagnostics::Diagnostic;
use crate::lint::{LintContext, LintRegistry};
use crate::parser::parse_source;
use crate::visitor::walk_tree;

/// Engine orchestrates linting by parsing source and running registered rules.
pub struct LintEngine {
    registry: LintRegistry,
}

impl LintEngine {
    pub fn new(registry: LintRegistry) -> Self {
        Self { registry }
    }

    pub fn lint_source(&self, source: &str) -> Result<Vec<Diagnostic>> {
        let tree = parse_source(source)?;
        self.run_rules(source, &tree)
    }

    fn run_rules(&self, source: &str, tree: &Tree) -> Result<Vec<Diagnostic>> {
        let mut ctx = LintContext::new(source);
        let root = tree.root_node();

        for rule in self.registry.rules() {
            rule.check(root, source, &mut ctx);
        }

        // Walk tree to allow visitor-style rules later.
        // This keeps traversal centralized and extendable.
        walk_tree(root, source, &mut ctx);

        Ok(ctx.into_diagnostics())
    }
}

pub fn create_default_engine() -> LintEngine {
    LintEngine::new(LintRegistry::default_rules())
}
