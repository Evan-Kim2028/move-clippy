//! Core Move Clippy engine and lint registry.
//!
//! The crate exposes a tree-sitter based `LintEngine` for fast mode and
//! optional semantic helpers when built with the `full` feature.

pub mod cli;
pub mod config;
pub mod diagnostics;
pub mod error;
pub mod fix;
pub mod fixer;
pub mod level;
pub mod lint;
pub mod parser;
pub mod rules;
pub mod semantic;
pub mod suppression;
pub mod telemetry;
pub mod triage;
pub mod visitor;

use anyhow::Result;
use tree_sitter::Tree;

use crate::diagnostics::Diagnostic;
use crate::lint::{LintContext, LintRegistry, LintSettings};
use crate::parser::parse_source;
use crate::visitor::walk_tree;

/// Engine orchestrates linting by parsing source and running registered rules.
pub struct LintEngine {
    registry: LintRegistry,
    settings: LintSettings,
}

impl LintEngine {
    /// Create a new engine with default lint settings.
    pub fn new(registry: LintRegistry) -> Self {
        Self {
            registry,
            settings: LintSettings::default(),
        }
    }

    /// Create a new engine with explicit lint settings (e.g. from config).
    pub fn new_with_settings(registry: LintRegistry, settings: LintSettings) -> Self {
        Self { registry, settings }
    }

    /// Lint a single in-memory source string and return diagnostics.
    pub fn lint_source(&self, source: &str) -> Result<Vec<Diagnostic>> {
        let tree = parse_source(source)?;
        self.run_rules(source, &tree)
    }

    fn run_rules(&self, source: &str, tree: &Tree) -> Result<Vec<Diagnostic>> {
        let mut ctx = LintContext::new(source, self.settings.clone());
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

/// Construct a `LintEngine` with all built-in fast lints enabled.
pub fn create_default_engine() -> LintEngine {
    LintEngine::new(LintRegistry::default_rules())
}
