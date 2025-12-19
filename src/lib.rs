//! Core Move Clippy engine and lint registry.
//!
//! The crate exposes a tree-sitter based `LintEngine` for fast mode and
//! optional semantic helpers when built with the `full` feature.
//!
//! # Quick Start
//!
//! ```
//! use move_clippy::{LintEngine, create_default_engine};
//!
//! // Create engine with default stable lints
//! let engine = create_default_engine();
//!
//! // Or use the builder for more control
//! let engine = LintEngine::builder()
//!     .preview(true)  // Enable preview lints
//!     .build()
//!     .expect("failed to build engine");
//!
//! // Lint some source code
//! let diagnostics = engine.lint_source("module test::m {}")
//!     .expect("failed to lint");
//! ```

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

// ============================================================================
// Public Re-exports
// ============================================================================

// Core types
pub use crate::diagnostics::{Diagnostic, Position, Span, Suggestion};
pub use crate::error::{Error, Result};
pub use crate::level::LintLevel;
pub use crate::lint::{
    AnalysisKind, LintCategory, LintDescriptor, LintName, LintRegistry, LintRule, LintSettings,
    RuleGroup,
};

// Unified registry
pub use crate::unified::{LintPhase, UnifiedLint, UnifiedLintRegistry, unified_registry};

// ============================================================================
// LintEngine
// ============================================================================

use anyhow::Result as AnyhowResult;
use std::fmt;
use tree_sitter::Tree;

use crate::lint::LintContext;
use crate::parser::parse_source;
use crate::visitor::walk_tree;

/// Engine orchestrates linting by parsing source and running registered rules.
///
/// # Creating an Engine
///
/// Use [`LintEngine::builder()`] for full control over configuration:
///
/// ```
/// use move_clippy::LintEngine;
///
/// let engine = LintEngine::builder()
///     .preview(true)
///     .skip(["while_true_to_loop".to_string()])
///     .build()
///     .expect("failed to build engine");
/// ```
///
/// Or use [`create_default_engine()`] for quick setup with stable lints only.
pub struct LintEngine {
    registry: LintRegistry,
    settings: LintSettings,
}

impl fmt::Debug for LintEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LintEngine")
            .field(
                "registry",
                &format!("<{} rules>", self.registry.rules().count()),
            )
            .field("settings", &self.settings)
            .finish()
    }
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
    pub fn lint_source(&self, source: &str) -> AnyhowResult<Vec<Diagnostic>> {
        let tree = parse_source(source)?;
        self.run_rules(source, &tree)
    }

    fn run_rules(&self, source: &str, tree: &Tree) -> AnyhowResult<Vec<Diagnostic>> {
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

// ============================================================================
// LintEngineBuilder
// ============================================================================

/// Builder for constructing a configured [`LintEngine`].
///
/// Use [`LintEngine::builder()`] to create a new builder.
///
/// # Default
///
/// The builder implements [`Default`], which creates a builder with:
/// - No custom registry (will build from default rules)
/// - Default lint settings
/// - No lint filtering (only, skip, disabled are empty)
/// - Full mode disabled
/// - Preview and experimental lints disabled
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

impl fmt::Debug for LintEngineBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LintEngineBuilder")
            .field("registry", &self.registry.as_ref().map(|_| "<custom>"))
            .field("settings", &self.settings)
            .field("only", &self.only)
            .field("skip", &self.skip)
            .field("disabled", &self.disabled)
            .field("full_mode", &self.full_mode)
            .field("preview", &self.preview)
            .field("experimental", &self.experimental)
            .finish()
    }
}

impl LintEngineBuilder {
    /// Create a new builder with default settings.
    ///
    /// This is equivalent to `LintEngineBuilder::default()`.
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
    pub fn build(self) -> crate::error::Result<LintEngine> {
        let registry = match self.registry {
            Some(r) => r,
            None => LintRegistry::default_rules_filtered_with_experimental(
                &self.only,
                &self.skip,
                &self.disabled,
                self.full_mode,
                self.preview,
                self.experimental,
            )
            .map_err(|e| crate::error::Error::other(e.to_string()))?,
        };

        Ok(LintEngine::new_with_settings(registry, self.settings))
    }
}

// ============================================================================
// Convenience Functions
// ============================================================================

/// Construct a `LintEngine` with all built-in fast lints enabled.
///
/// This creates an engine with:
/// - Only stable lints (no preview or experimental)
/// - Default lint settings
/// - Fast mode only (no semantic analysis)
///
/// For more control, use [`LintEngine::builder()`].
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

// ============================================================================
// Tests
// ============================================================================

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

    #[test]
    fn test_engine_debug() {
        let engine = create_default_engine();
        let debug_str = format!("{:?}", engine);
        assert!(debug_str.contains("LintEngine"));
        assert!(debug_str.contains("rules"));
    }

    #[test]
    fn test_builder_debug() {
        let builder = LintEngineBuilder::new().preview(true);
        let debug_str = format!("{:?}", builder);
        assert!(debug_str.contains("LintEngineBuilder"));
        assert!(debug_str.contains("preview: true"));
    }
}
