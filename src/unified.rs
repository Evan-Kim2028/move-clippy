//! Unified Lint Architecture for move-clippy
//!
//! This module provides a unified interface for all lint systems:
//! - Phase I: Tree-sitter syntactic lints (LintRule trait)
//! - Phase II: Semantic type-based lints (semantic.rs)
//! - Phase III: CFG-aware abstract interpretation lints (absint_lints.rs)
//! - Phase IV: Cross-module call graph lints (cross_module_lints.rs)
//!
//! The unified architecture enables:
//! - Consistent querying by tier, category, analysis kind
//! - Unified diagnostic output across all lint phases
//! - Single point of registration for all lint types

use crate::lint::{AnalysisKind, LintCategory, LintDescriptor, LintRegistry, RuleGroup};
use std::collections::HashMap;
use std::sync::OnceLock;

static UNIFIED_REGISTRY: OnceLock<UnifiedLintRegistry> = OnceLock::new();

pub fn unified_registry() -> &'static UnifiedLintRegistry {
    UNIFIED_REGISTRY.get_or_init(build_unified_registry)
}

pub fn lint_phase(name: &str) -> Option<LintPhase> {
    unified_registry().get(name).map(|l| l.phase)
}

/// Unified lint entry that wraps lint metadata from any phase.
#[derive(Debug, Clone)]
pub struct UnifiedLint {
    /// The lint descriptor containing metadata
    pub descriptor: &'static LintDescriptor,
    /// Phase of the lint system (1-4)
    pub phase: LintPhase,
}

/// Classification of which lint phase a lint belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LintPhase {
    /// Phase I: Tree-sitter syntactic lints (fast, no type info)
    Syntactic,
    /// Phase II: Semantic type-based lints (requires Move compiler)
    Semantic,
    /// Phase III: CFG-aware abstract interpretation lints
    AbstractInterpretation,
    /// Phase IV: Cross-module call graph lints
    CrossModule,
}

impl LintPhase {
    pub fn as_str(&self) -> &'static str {
        match self {
            LintPhase::Syntactic => "syntactic",
            LintPhase::Semantic => "semantic",
            LintPhase::AbstractInterpretation => "absint",
            LintPhase::CrossModule => "cross-module",
        }
    }

    /// Returns the CLI mode required for this phase.
    pub fn required_mode(&self) -> Option<&'static str> {
        match self {
            LintPhase::Syntactic => None, // Works in --mode fast (default)
            LintPhase::Semantic => Some("--mode full"),
            LintPhase::AbstractInterpretation => Some("--mode full"),
            LintPhase::CrossModule => Some("--mode full"),
        }
    }
}

/// Unified registry for all lint types across all phases.
#[derive(Debug, Default)]
pub struct UnifiedLintRegistry {
    /// All registered lints indexed by name
    lints: HashMap<&'static str, UnifiedLint>,
    /// Index by tier for fast filtering
    by_tier: HashMap<RuleGroup, Vec<&'static str>>,
    /// Index by category for fast filtering
    by_category: HashMap<LintCategory, Vec<&'static str>>,
    /// Index by phase for fast filtering
    by_phase: HashMap<LintPhase, Vec<&'static str>>,
    /// Index by analysis kind
    by_analysis: HashMap<AnalysisKind, Vec<&'static str>>,
}

impl UnifiedLintRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a lint from any phase.
    pub fn register(&mut self, descriptor: &'static LintDescriptor, phase: LintPhase) {
        let name = descriptor.name;

        // Store the lint
        self.lints.insert(name, UnifiedLint { descriptor, phase });

        // Update indices
        self.by_tier.entry(descriptor.group).or_default().push(name);
        self.by_category
            .entry(descriptor.category)
            .or_default()
            .push(name);
        self.by_phase.entry(phase).or_default().push(name);
        self.by_analysis
            .entry(descriptor.analysis)
            .or_default()
            .push(name);
    }

    /// Get a lint by name.
    pub fn get(&self, name: &str) -> Option<&UnifiedLint> {
        self.lints.get(name)
    }

    /// Get all lints.
    pub fn all(&self) -> impl Iterator<Item = &UnifiedLint> {
        self.lints.values()
    }

    /// Get all lint descriptors.
    pub fn descriptors(&self) -> impl Iterator<Item = &'static LintDescriptor> {
        self.lints.values().map(|l| l.descriptor)
    }

    /// Get lints by tier.
    pub fn by_tier(&self, tier: RuleGroup) -> Vec<&UnifiedLint> {
        self.by_tier
            .get(&tier)
            .map(|names| names.iter().filter_map(|n| self.lints.get(n)).collect())
            .unwrap_or_default()
    }

    /// Get lints by category.
    pub fn by_category(&self, category: LintCategory) -> Vec<&UnifiedLint> {
        self.by_category
            .get(&category)
            .map(|names| names.iter().filter_map(|n| self.lints.get(n)).collect())
            .unwrap_or_default()
    }

    /// Get lints by phase.
    pub fn by_phase(&self, phase: LintPhase) -> Vec<&UnifiedLint> {
        self.by_phase
            .get(&phase)
            .map(|names| names.iter().filter_map(|n| self.lints.get(n)).collect())
            .unwrap_or_default()
    }

    /// Get lints by analysis kind.
    pub fn by_analysis(&self, analysis: AnalysisKind) -> Vec<&UnifiedLint> {
        self.by_analysis
            .get(&analysis)
            .map(|names| names.iter().filter_map(|n| self.lints.get(n)).collect())
            .unwrap_or_default()
    }

    /// Get all stable lints.
    pub fn stable(&self) -> Vec<&UnifiedLint> {
        self.by_tier(RuleGroup::Stable)
    }

    /// Get all preview lints.
    pub fn preview(&self) -> Vec<&UnifiedLint> {
        self.by_tier(RuleGroup::Preview)
    }

    /// Get all experimental lints.
    pub fn experimental(&self) -> Vec<&UnifiedLint> {
        self.by_tier(RuleGroup::Experimental)
    }

    /// Get all security lints.
    pub fn security(&self) -> Vec<&UnifiedLint> {
        self.by_category(LintCategory::Security)
    }

    /// Get all lints that require --mode full.
    pub fn requiring_full_mode(&self) -> Vec<&UnifiedLint> {
        self.lints
            .values()
            .filter(|l| l.descriptor.analysis.requires_full_mode())
            .collect()
    }

    /// Get all lints available in fast mode (default).
    pub fn fast_mode_lints(&self) -> Vec<&UnifiedLint> {
        self.by_phase(LintPhase::Syntactic)
    }

    /// Count total registered lints.
    pub fn len(&self) -> usize {
        self.lints.len()
    }

    /// Check if registry is empty.
    pub fn is_empty(&self) -> bool {
        self.lints.is_empty()
    }
}

/// ## Extension Point: Adding a fast (syntactic) lint
///
/// Fast-mode lints are tree-sitter based `LintRule` implementations under `src/rules/`.
/// To add a new fast lint:
///
/// 1. Define a `static LintDescriptor` (name/category/group/analysis).
/// 2. Implement `LintRule` and report findings via `ctx.report_node(...)` (or
///    `ctx.report_node_diagnostic(...)` if you need suggestions), so directives always apply.
/// 3. Export the lint type from `src/rules.rs`.
/// 4. Register it here by adding `.with_rule(crate::rules::YourLintType)`.
/// 5. Add executable documentation:
///    - a minimal positive and negative fixture, and
///    - a directive coverage fixture (see `tests/fixtures/README.md`).
///
/// The unified registry (`build_unified_registry`) automatically picks up these descriptors
/// for CLI output (`list-rules`, `explain`) and for generated docs.
pub(crate) fn build_syntactic_registry() -> LintRegistry {
    LintRegistry::new()
        // Existing lints
        .with_rule(crate::rules::ModernModuleSyntaxLint)
        .with_rule(crate::rules::RedundantSelfImportLint)
        .with_rule(crate::rules::PreferToStringLint)
        .with_rule(crate::rules::PreferVectorMethodsLint)
        .with_rule(crate::rules::ModernMethodSyntaxLint)
        .with_rule(crate::rules::MergeTestAttributesLint)
        .with_rule(crate::rules::ConstantNamingLint)
        .with_rule(crate::rules::UnneededReturnLint)
        .with_rule(crate::rules::UnnecessaryPublicEntryLint)
        .with_rule(crate::rules::PublicMutTxContextLint)
        .with_rule(crate::rules::WhileTrueToLoopLint)
        // P0 lints
        .with_rule(crate::rules::AbilitiesOrderLint)
        .with_rule(crate::rules::DocCommentStyleLint)
        .with_rule(crate::rules::ExplicitSelfAssignmentsLint)
        .with_rule(crate::rules::TestAbortCodeLint)
        .with_rule(crate::rules::RedundantTestPrefixLint)
        // P1 lints
        .with_rule(crate::rules::EqualityInAssertLint)
        .with_rule(crate::rules::AdminCapPositionLint)
        .with_rule(crate::rules::ManualOptionCheckLint)
        .with_rule(crate::rules::ManualLoopIterationLint)
        // Additional stable lints
        .with_rule(crate::rules::EventSuffixLint)
        .with_rule(crate::rules::EmptyVectorLiteralLint)
        .with_rule(crate::rules::TypedAbortCodeLint)
        // Security lints (audit-backed)
        .with_rule(crate::rules::StaleOraclePriceLint)
        .with_rule(crate::rules::SingleStepOwnershipTransferLint)
        .with_rule(crate::rules::MissingWitnessDropLint)
        .with_rule(crate::rules::PublicRandomAccessLint)
        .with_rule(crate::rules::SuspiciousOverflowCheckLint)
        .with_rule(crate::rules::IgnoredBooleanReturnLint)
        .with_rule(crate::rules::DivideByZeroLiteralLint)
        // Preview/experimental lints
        .with_rule(crate::rules::PureFunctionTransferLint)
        .with_rule(crate::rules::UnsafeArithmeticLint)
        .with_rule(crate::rules::UncheckedCoinSplitLint)
        .with_rule(crate::rules::UncheckedWithdrawalLint)
        .with_rule(crate::rules::CapabilityLeakLint)
        .with_rule(crate::rules::DestroyZeroUncheckedLint)
        .with_rule(crate::rules::OtwPatternViolationLint)
        .with_rule(crate::rules::DigestAsRandomnessLint)
        .with_rule(crate::rules::FreshAddressReuseLint)
}

/// Build a unified registry from all lint phases.
///
/// This collects lints from:
/// - LintRegistry (Phase I syntactic)
/// - semantic.rs descriptors (Phase II)
/// - absint_lints.rs descriptors (Phase III, feature-gated)
/// - cross_module_lints.rs descriptors (Phase IV, feature-gated)
/// ## Extension Point: Lint inventory and CLI surface
///
/// This function defines the authoritative lint inventory for:
/// - `move-clippy list-rules`
/// - `move-clippy explain <lint>`
/// - generated docs (`docs/LINT_REFERENCE.md`, `docs/LINT_CATALOG_SUMMARY.md`)
///
/// Each phase has exactly one “register here” path:
/// - Phase I (fast): `build_syntactic_registry()` above.
/// - Phase II (semantic): `src/semantic.rs:DESCRIPTORS`.
/// - Phase III (AbsInt): `src/absint_lints.rs:DESCRIPTORS` (feature-gated).
/// - Phase IV (cross-module): `src/cross_module_lints.rs:DESCRIPTORS` (feature-gated).
pub fn build_unified_registry() -> UnifiedLintRegistry {
    let mut registry = UnifiedLintRegistry::new();

    // Phase I: Syntactic lints
    let lint_registry = build_syntactic_registry();
    for descriptor in lint_registry.descriptors() {
        registry.register(descriptor, LintPhase::Syntactic);
    }

    // Phase II: Semantic lints
    for descriptor in crate::semantic::descriptors() {
        registry.register(descriptor, LintPhase::Semantic);
    }

    // Phase III: Abstract interpretation lints (feature-gated)
    #[cfg(feature = "full")]
    for descriptor in crate::absint_lints::descriptors() {
        registry.register(descriptor, LintPhase::AbstractInterpretation);
    }

    // Phase IV: Cross-module lints (feature-gated)
    #[cfg(feature = "full")]
    for descriptor in crate::cross_module_lints::descriptors() {
        registry.register(descriptor, LintPhase::CrossModule);
    }

    registry
}

/// Print a summary of all registered lints.
pub fn print_lint_summary(registry: &UnifiedLintRegistry) {
    println!("=== Unified Lint Registry Summary ===\n");

    // By tier
    println!("By Tier:");
    println!(
        "  Stable:       {}",
        registry.by_tier(RuleGroup::Stable).len()
    );
    println!(
        "  Preview:      {}",
        registry.by_tier(RuleGroup::Preview).len()
    );
    println!(
        "  Experimental: {}",
        registry.by_tier(RuleGroup::Experimental).len()
    );
    println!();

    // By phase
    println!("By Phase:");
    println!(
        "  Syntactic:    {}",
        registry.by_phase(LintPhase::Syntactic).len()
    );
    println!(
        "  Semantic:     {}",
        registry.by_phase(LintPhase::Semantic).len()
    );
    println!(
        "  AbsInt:       {}",
        registry.by_phase(LintPhase::AbstractInterpretation).len()
    );
    println!(
        "  CrossModule:  {}",
        registry.by_phase(LintPhase::CrossModule).len()
    );
    println!();

    // By category
    println!("By Category:");
    println!(
        "  Style:        {}",
        registry.by_category(LintCategory::Style).len()
    );
    println!(
        "  Modernization:{}",
        registry.by_category(LintCategory::Modernization).len()
    );
    println!(
        "  Naming:       {}",
        registry.by_category(LintCategory::Naming).len()
    );
    println!(
        "  Security:     {}",
        registry.by_category(LintCategory::Security).len()
    );
    println!(
        "  Suspicious:   {}",
        registry.by_category(LintCategory::Suspicious).len()
    );
    println!(
        "  TestQuality:  {}",
        registry.by_category(LintCategory::TestQuality).len()
    );
    println!();

    println!("Total: {} lints", registry.len());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unified_registry_creation() {
        let registry = build_unified_registry();
        assert!(!registry.is_empty(), "Registry should have lints");
    }

    #[test]
    fn test_lint_lookup_by_name() {
        let registry = build_unified_registry();

        // Should find syntactic lints
        let lint = registry.get("abilities_order");
        assert!(lint.is_some(), "Should find abilities_order lint");

        let lint = lint.unwrap();
        assert_eq!(lint.phase, LintPhase::Syntactic);
        assert_eq!(lint.descriptor.group, RuleGroup::Stable);
    }

    #[test]
    fn test_tier_filtering() {
        let registry = build_unified_registry();

        let stable = registry.stable();
        assert!(!stable.is_empty(), "Should have stable lints");

        for lint in &stable {
            assert_eq!(
                lint.descriptor.group,
                RuleGroup::Stable,
                "All stable lints should be Stable tier"
            );
        }
    }

    #[test]
    fn test_phase_filtering() {
        let registry = build_unified_registry();

        let syntactic = registry.by_phase(LintPhase::Syntactic);
        assert!(!syntactic.is_empty(), "Should have syntactic lints");

        for lint in &syntactic {
            assert_eq!(
                lint.phase,
                LintPhase::Syntactic,
                "All syntactic lints should be Phase I"
            );
        }
    }

    #[test]
    fn test_security_lints() {
        let registry = build_unified_registry();

        let security = registry.security();
        // May be empty if no security lints are registered
        for lint in &security {
            assert_eq!(
                lint.descriptor.category,
                LintCategory::Security,
                "All security lints should be Security category"
            );
        }
    }

    #[test]
    fn test_fast_mode_lints() {
        let registry = build_unified_registry();

        let fast = registry.fast_mode_lints();
        assert!(!fast.is_empty(), "Should have fast mode lints");

        for lint in &fast {
            assert!(
                !lint.descriptor.analysis.requires_full_mode(),
                "Fast mode lints should not require full mode"
            );
        }
    }
}
