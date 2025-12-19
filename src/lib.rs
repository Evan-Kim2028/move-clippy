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
    #[must_use]
    pub fn new(registry: LintRegistry) -> Self {
        Self {
            registry,
            settings: LintSettings::default(),
        }
    }

    /// Create a new engine with explicit lint settings (e.g. from config).
    #[must_use]
    pub fn new_with_settings(registry: LintRegistry, settings: LintSettings) -> Self {
        Self { registry, settings }
    }

    /// Create a builder for configuring a `LintEngine`.
    ///
    /// This is the preferred way to construct an engine with custom settings.
    ///
    /// # Examples
    ///
    /// ```
    /// use move_clippy::LintEngine;
    ///
    /// let engine = LintEngine::builder()
    ///     .preview(true)
    ///     .build()
    ///     .expect("failed to build engine");
    /// ```
    #[must_use]
    pub fn builder() -> LintEngineBuilder {
        LintEngineBuilder::new()
    }

    /// Lint a single in-memory source string and return diagnostics.
    #[must_use = "diagnostics should be processed or reported"]
    pub fn lint_source(&self, source: &str) -> Result<Vec<Diagnostic>> {
        let tree = parse_source(source)?;
        self.run_rules(source, &tree)
    }

    fn run_rules(&self, source: &str, tree: &Tree) -> Result<Vec<Diagnostic>> {
        let mut ctx = LintContext::new(source, self.settings.clone());
        let root = tree.root_node();

        ctx.precollect_item_directives(root);

        for rule in self.registry.rules() {
            rule.check(root, source, &mut ctx);
        }

        // Walk tree to allow visitor-style rules later.
        // This keeps traversal centralized and extendable.
        walk_tree(root, source, &mut ctx);

        Ok(ctx.into_diagnostics())
    }
}

/// Builder for constructing a configured [`LintEngine`].
///
/// Use [`LintEngine::builder()`] to create a new builder.
///
/// # Examples
///
/// ```
/// use move_clippy::LintEngineBuilder;
///
/// // Build with preview lints enabled
/// let engine = LintEngineBuilder::new()
///     .preview(true)
///     .build()
///     .expect("failed to build engine");
///
/// // Build with specific lints only
/// let engine = LintEngineBuilder::new()
///     .only(["abilities_order".to_string(), "while_true_to_loop".to_string()])
///     .build()
///     .expect("failed to build engine");
/// ```
#[derive(Default)]
pub struct LintEngineBuilder {
    registry: Option<LintRegistry>,
    settings: LintSettings,
    only: Vec<String>,
    skip: Vec<String>,
    disabled: Vec<String>,
    full_mode: bool,
    preview: bool,
    experimental: bool,
}

impl LintEngineBuilder {
    /// Create a new builder with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Use a custom lint registry instead of building one from options.
    ///
    /// When a custom registry is provided, `only`, `skip`, `disabled`,
    /// `preview`, and `experimental` options are ignored.
    #[must_use]
    pub fn registry(mut self, registry: LintRegistry) -> Self {
        self.registry = Some(registry);
        self
    }

    /// Use custom lint settings.
    #[must_use]
    pub fn settings(mut self, settings: LintSettings) -> Self {
        self.settings = settings;
        self
    }

    /// Only run these specific lints.
    ///
    /// Takes any iterator of strings. When set, only lints in this list
    /// will be enabled (after filtering by tier).
    #[must_use]
    pub fn only(mut self, lints: impl IntoIterator<Item = String>) -> Self {
        self.only = lints.into_iter().collect();
        self
    }

    /// Skip these lints.
    ///
    /// Lints in this list will be excluded from the registry.
    #[must_use]
    pub fn skip(mut self, lints: impl IntoIterator<Item = String>) -> Self {
        self.skip = lints.into_iter().collect();
        self
    }

    /// Disable these lints.
    ///
    /// Lints in this list will be disabled (same as skip).
    #[must_use]
    pub fn disabled(mut self, lints: impl IntoIterator<Item = String>) -> Self {
        self.disabled = lints.into_iter().collect();
        self
    }

    /// Enable full mode (requires `--features full` at compile time).
    ///
    /// Full mode filters out syntactic lints that are superseded by
    /// semantic lints in the full analysis.
    #[must_use]
    pub fn full_mode(mut self, enabled: bool) -> Self {
        self.full_mode = enabled;
        self
    }

    /// Enable preview lints.
    ///
    /// Preview lints are newer rules that may have higher false-positive
    /// rates or change behavior between versions.
    #[must_use]
    pub fn preview(mut self, enabled: bool) -> Self {
        self.preview = enabled;
        self
    }

    /// Enable experimental lints (implies preview).
    ///
    /// Experimental lints have high false-positive risk and are useful
    /// for research but not recommended for CI.
    #[must_use]
    pub fn experimental(mut self, enabled: bool) -> Self {
        self.experimental = enabled;
        self
    }

    /// Build the configured [`LintEngine`].
    ///
    /// # Errors
    ///
    /// Returns an error if any lint name in `only`, `skip`, or `disabled`
    /// is not a known lint name.
    pub fn build(self) -> anyhow::Result<LintEngine> {
        let registry = match self.registry {
            Some(r) => r,
            None => LintRegistry::default_rules_filtered_with_experimental(
                &self.only,
                &self.skip,
                &self.disabled,
                self.full_mode,
                self.preview,
                self.experimental,
            )?,
        };

        Ok(LintEngine::new_with_settings(registry, self.settings))
    }
}

/// Construct a `LintEngine` with all built-in fast lints enabled.
#[must_use = "engine should be used for linting"]
pub fn create_default_engine() -> LintEngine {
    // Use filtered registry to respect tier system (Stable only by default)
    let registry = LintRegistry::default_rules_filtered_with_experimental(
        &[],   // only
        &[],   // skip
        &[],   // disabled
        false, // full_mode
        false, // preview
        false, // experimental
    )
    .expect("Failed to create default registry");

    LintEngine::new(registry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_default() {
        let engine = LintEngineBuilder::new().build().expect("build failed");
        // Should create a working engine with default stable lints
        let result = engine.lint_source("module test::m {}");
        assert!(result.is_ok());
    }

    #[test]
    fn test_builder_with_preview() {
        let engine = LintEngineBuilder::new()
            .preview(true)
            .build()
            .expect("build failed");
        let result = engine.lint_source("module test::m {}");
        assert!(result.is_ok());
    }

    #[test]
    fn test_builder_unknown_lint_error() {
        let result = LintEngineBuilder::new()
            .only(["not_a_real_lint".to_string()])
            .build();
        match result {
            Ok(_) => panic!("expected error for unknown lint"),
            Err(e) => assert!(e.to_string().contains("unknown lint")),
        }
    }

    #[test]
    fn test_engine_builder_method() {
        let engine = LintEngine::builder()
            .preview(false)
            .build()
            .expect("build failed");
        let result = engine.lint_source("module test::m {}");
        assert!(result.is_ok());
    }
}
