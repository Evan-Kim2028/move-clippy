//! Core Move Clippy engine and lint registry.
//!
//! The crate exposes a tree-sitter based `LintEngine` for fast mode and
//! optional semantic helpers when built with the `full` feature.

// Allow patterns that are intentional in this codebase
#![allow(clippy::type_complexity)] // Complex types are used intentionally for Move compiler integration
#![allow(clippy::too_many_arguments)] // Move compiler APIs require many arguments
#![allow(clippy::should_implement_trait)] // from_str methods intentionally return Option, not Result
#![allow(clippy::new_without_default)] // LintRegistry::new() requires explicit construction
#![allow(clippy::field_reassign_with_default)] // Pattern used for clarity in test setup
#![allow(clippy::derivable_impls)] // Some Default impls are explicit for documentation
#![allow(clippy::manual_contains)] // Used in hot paths where iter().any() is clearer
#![allow(clippy::vec_init_then_push)] // Used for clarity in some contexts

pub mod annotations;
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
pub mod unified;
pub mod visitor;

#[cfg(feature = "full")]
pub mod type_classifier;

#[cfg(feature = "full")]
pub mod framework_catalog;

#[cfg(feature = "full")]
pub mod guard_utils;

#[cfg(feature = "full")]
pub mod absint_lints;

#[cfg(feature = "full")]
pub mod cross_module_lints;

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
    // Use filtered registry to respect tier system (Stable only by default)
    let registry = LintRegistry::default_rules_filtered_with_experimental(
        &[],    // only
        &[],    // skip
        &[],    // disabled
        false,  // full_mode
        false,  // preview
        false,  // experimental
    )
    .expect("Failed to create default registry");
    
    LintEngine::new(registry)
}
