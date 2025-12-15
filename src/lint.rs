use crate::diagnostics::{Diagnostic, Span};
use crate::level::LintLevel;
use crate::suppression;
use anyhow::{Result, anyhow};
use std::collections::{HashMap, HashSet};
use tree_sitter::Node;

// ============================================================================
// Rule Groups (Preview vs Stable) - Inspired by Ruff
// ============================================================================

/// Classification of lint rules by stability level.
///
/// New rules start in `Preview` and graduate to `Stable` after meeting
/// promotion criteria (see docs/STABILITY.md).
///
/// Tier hierarchy (most to least stable):
/// 1. Stable - Production-ready, zero/near-zero false positives
/// 2. Preview - Good detection but may have edge case FPs
/// 3. Experimental - High FP risk, requires explicit opt-in
/// 4. Deprecated - Scheduled for removal
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum RuleGroup {
    /// Battle-tested rules with minimal false positives.
    /// Enabled by default based on category.
    #[default]
    Stable,

    /// New rules that need community validation.
    /// Require `--preview` flag or `preview = true` in config.
    Preview,

    /// Experimental rules with high false positive risk.
    /// Require `--experimental` flag or `experimental = true` in config.
    /// These rules are useful for research but not recommended for CI.
    Experimental,

    /// Rules scheduled for removal in the next major version.
    /// Emit a warning when explicitly enabled.
    Deprecated,
}

impl RuleGroup {
    pub fn as_str(&self) -> &'static str {
        match self {
            RuleGroup::Stable => "stable",
            RuleGroup::Preview => "preview",
            RuleGroup::Experimental => "experimental",
            RuleGroup::Deprecated => "deprecated",
        }
    }

    /// Returns true if this tier requires explicit opt-in via CLI flag.
    pub fn requires_opt_in(&self) -> bool {
        matches!(self, RuleGroup::Preview | RuleGroup::Experimental)
    }

    /// Returns the CLI flag needed to enable this tier.
    pub fn required_flag(&self) -> Option<&'static str> {
        match self {
            RuleGroup::Stable => None,
            RuleGroup::Preview => Some("--preview"),
            RuleGroup::Experimental => Some("--experimental"),
            RuleGroup::Deprecated => None, // Always available but warns
        }
    }
}

// ============================================================================
// Analysis Kind Classification
// ============================================================================

/// Analysis kinds determine how a lint examines Move code:
/// - `Syntactic` lints use tree-sitter pattern matching
/// - `TypeBased` lints use the Move compiler's type checker
/// - `TypeBasedCFG` lints use abstract interpretation (control flow)
/// - `CrossModule` lints analyze call graphs across module boundaries
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum AnalysisKind {
    /// Tree-sitter pattern matching (fast, no type info).
    /// Runs in `--mode fast` (default).
    #[default]
    Syntactic,
    /// Move compiler type/ability checking (TypingProgramInfo).
    /// Requires `--mode full`.
    TypeBased,
    /// CFG-aware abstract interpretation (SimpleAbsInt).
    /// Requires `--mode full --preview`.
    TypeBasedCFG,
    /// Cross-module call graph analysis.
    /// Requires `--mode full --preview`.
    CrossModule,
}

impl AnalysisKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            AnalysisKind::Syntactic => "syntactic",
            AnalysisKind::TypeBased => "type-based",
            AnalysisKind::TypeBasedCFG => "type-based-cfg",
            AnalysisKind::CrossModule => "cross-module",
        }
    }

    /// Returns true if this analysis kind requires `--mode full`.
    pub fn requires_full_mode(&self) -> bool {
        !matches!(self, AnalysisKind::Syntactic)
    }

    /// Returns true if this analysis kind requires `--preview`.
    pub fn requires_preview(&self) -> bool {
        matches!(self, AnalysisKind::TypeBasedCFG | AnalysisKind::CrossModule)
    }
}

// ============================================================================
// Fix Safety Classification - Inspired by Ruff
// ============================================================================

/// Safety classification for auto-fixes.
///
/// - `Safe` fixes preserve runtime behavior exactly
/// - `Unsafe` fixes may change runtime behavior and require explicit opt-in
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FixSafety {
    /// Fix is guaranteed to preserve runtime behavior.
    /// Applied by default with `--fix`.
    #[default]
    Safe,

    /// Fix may change runtime behavior (different errors, side effects, etc.).
    /// Requires `--unsafe-fixes` flag to apply.
    Unsafe,
}

impl FixSafety {
    pub fn as_str(&self) -> &'static str {
        match self {
            FixSafety::Safe => "safe",
            FixSafety::Unsafe => "unsafe",
        }
    }
}

/// Descriptor for an auto-fix associated with a lint rule.
#[derive(Debug, Clone)]
pub struct FixDescriptor {
    /// Whether an auto-fix is available for this lint.
    pub available: bool,
    /// Safety classification of the fix.
    pub safety: FixSafety,
    /// Human-readable description of what the fix does.
    pub description: &'static str,
}

impl FixDescriptor {
    /// Create a safe fix descriptor.
    pub const fn safe(description: &'static str) -> Self {
        Self {
            available: true,
            safety: FixSafety::Safe,
            description,
        }
    }

    /// Create an unsafe fix descriptor.
    pub const fn unsafe_fix(description: &'static str) -> Self {
        Self {
            available: true,
            safety: FixSafety::Unsafe,
            description,
        }
    }

    /// Indicate no fix is available.
    pub const fn none() -> Self {
        Self {
            available: false,
            safety: FixSafety::Safe,
            description: "",
        }
    }
}

// ============================================================================
// Lint Categories
// ============================================================================

/// High-level categories used to group lints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LintCategory {
    Style,
    Modernization,
    Naming,
    TestQuality,
    Suspicious,
    /// Security-critical lints that detect potential vulnerabilities.
    /// These are based on real audit findings and published security research.
    Security,
}

impl LintCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            LintCategory::Style => "style",
            LintCategory::Modernization => "modernization",
            LintCategory::Naming => "naming",
            LintCategory::TestQuality => "test_quality",
            LintCategory::Suspicious => "suspicious",
            LintCategory::Security => "security",
        }
    }
}

/// Static metadata describing a lint rule.
#[derive(Debug)]
pub struct LintDescriptor {
    pub name: &'static str,
    pub category: LintCategory,
    pub description: &'static str,
    /// Stability group: Stable, Preview, or Deprecated.
    pub group: RuleGroup,
    /// Auto-fix availability and safety classification.
    pub fix: FixDescriptor,
    /// Detection method used by this lint.
    pub analysis: AnalysisKind,
}

impl LintDescriptor {
    /// Helper to create a stable syntactic lint descriptor with no fix.
    pub const fn stable(
        name: &'static str,
        category: LintCategory,
        description: &'static str,
    ) -> Self {
        Self {
            name,
            category,
            description,
            group: RuleGroup::Stable,
            fix: FixDescriptor::none(),
            analysis: AnalysisKind::Syntactic,
        }
    }

    /// Helper to create a stable syntactic lint descriptor with a safe fix.
    pub const fn stable_with_fix(
        name: &'static str,
        category: LintCategory,
        description: &'static str,
        fix_description: &'static str,
    ) -> Self {
        Self {
            name,
            category,
            description,
            group: RuleGroup::Stable,
            fix: FixDescriptor::safe(fix_description),
            analysis: AnalysisKind::Syntactic,
        }
    }

    /// Helper to create a preview syntactic lint descriptor with no fix.
    pub const fn preview(
        name: &'static str,
        category: LintCategory,
        description: &'static str,
    ) -> Self {
        Self {
            name,
            category,
            description,
            group: RuleGroup::Preview,
            fix: FixDescriptor::none(),
            analysis: AnalysisKind::Syntactic,
        }
    }

    /// Helper to create a preview syntactic lint descriptor with a safe fix.
    pub const fn preview_with_fix(
        name: &'static str,
        category: LintCategory,
        description: &'static str,
        fix_description: &'static str,
    ) -> Self {
        Self {
            name,
            category,
            description,
            group: RuleGroup::Preview,
            fix: FixDescriptor::safe(fix_description),
            analysis: AnalysisKind::Syntactic,
        }
    }

    /// Helper to create a stable type-based lint descriptor (requires --mode full).
    pub const fn stable_type_based(
        name: &'static str,
        category: LintCategory,
        description: &'static str,
    ) -> Self {
        Self {
            name,
            category,
            description,
            group: RuleGroup::Stable,
            fix: FixDescriptor::none(),
            analysis: AnalysisKind::TypeBased,
        }
    }

    /// Helper to create a preview type-based lint descriptor (requires --mode full).
    pub const fn preview_type_based(
        name: &'static str,
        category: LintCategory,
        description: &'static str,
    ) -> Self {
        Self {
            name,
            category,
            description,
            group: RuleGroup::Preview,
            fix: FixDescriptor::none(),
            analysis: AnalysisKind::TypeBased,
        }
    }

    /// Helper to create a preview CFG-aware lint descriptor (requires --mode full --preview).
    pub const fn preview_cfg(
        name: &'static str,
        category: LintCategory,
        description: &'static str,
    ) -> Self {
        Self {
            name,
            category,
            description,
            group: RuleGroup::Preview,
            fix: FixDescriptor::none(),
            analysis: AnalysisKind::TypeBasedCFG,
        }
    }

    /// Helper to create a preview cross-module lint descriptor (requires --mode full --preview).
    pub const fn preview_cross_module(
        name: &'static str,
        category: LintCategory,
        description: &'static str,
    ) -> Self {
        Self {
            name,
            category,
            description,
            group: RuleGroup::Preview,
            fix: FixDescriptor::none(),
            analysis: AnalysisKind::CrossModule,
        }
    }
}

/// A single lint rule that can inspect a syntax tree.
pub trait LintRule: Send + Sync {
    fn descriptor(&self) -> &'static LintDescriptor;
    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>);
}

/// Per-lint configuration derived from `move-clippy.toml`.
#[derive(Debug, Clone, Default)]
pub struct LintSettings {
    levels: HashMap<String, LintLevel>,
}

impl LintSettings {
    pub fn with_config_levels(mut self, levels: HashMap<String, LintLevel>) -> Self {
        // Resolve aliases when storing levels
        for (name, level) in levels {
            let canonical = resolve_lint_alias(&name);
            self.levels.insert(canonical.to_string(), level);
        }
        self
    }

    pub fn disable(mut self, disabled: impl IntoIterator<Item = String>) -> Self {
        for name in disabled {
            // Resolve aliases when disabling
            let canonical = resolve_lint_alias(&name);
            self.levels.insert(canonical.to_string(), LintLevel::Allow);
        }
        self
    }

    pub fn level_for(&self, lint_name: &str) -> LintLevel {
        // First try canonical name, then check if input is an alias
        if let Some(&level) = self.levels.get(lint_name) {
            return level;
        }
        // Try resolving as alias
        let canonical = resolve_lint_alias(lint_name);
        self.levels.get(canonical).copied().unwrap_or_default()
    }
}

/// Mutable context passed to lint rules while traversing a file.
pub struct LintContext<'src> {
    source: &'src str,
    settings: LintSettings,
    diagnostics: Vec<Diagnostic>,
}

impl<'src> LintContext<'src> {
    pub fn new(source: &'src str, settings: LintSettings) -> Self {
        Self {
            source,
            settings,
            diagnostics: Vec::new(),
        }
    }

    pub fn report(
        &mut self,
        lint: &'static LintDescriptor,
        span: Span,
        message: impl Into<String>,
    ) {
        let level = self.settings.level_for(lint.name);
        if level == LintLevel::Allow {
            return;
        }

        self.diagnostics.push(Diagnostic {
            lint,
            level,
            file: None,
            span,
            message: message.into(),
            help: None,
            suggestion: None,
        });
    }

    pub fn report_node(
        &mut self,
        lint: &'static LintDescriptor,
        node: Node,
        message: impl Into<String>,
    ) {
        let level = self.settings.level_for(lint.name);
        if level == LintLevel::Allow {
            return;
        }

        let anchor_start = suppression::anchor_item_start_byte(node);
        if suppression::is_suppressed_at(self.source, anchor_start, lint.name) {
            return;
        }

        self.diagnostics.push(Diagnostic {
            lint,
            level,
            file: None,
            span: Span::from_range(node.range()),
            message: message.into(),
            help: None,
            suggestion: None,
        });
    }

    /// Report a diagnostic directly (for cases that need custom suggestions).
    pub fn report_diagnostic(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub fn report_span_with_anchor(
        &mut self,
        lint: &'static LintDescriptor,
        anchor_start_byte: usize,
        span: Span,
        message: impl Into<String>,
    ) {
        let level = self.settings.level_for(lint.name);
        if level == LintLevel::Allow {
            return;
        }

        if suppression::is_suppressed_at(self.source, anchor_start_byte, lint.name) {
            return;
        }

        self.diagnostics.push(Diagnostic {
            lint,
            level,
            file: None,
            span,
            message: message.into(),
            help: None,
            suggestion: None,
        });
    }

    pub fn source(&self) -> &'src str {
        self.source
    }

    pub fn settings(&self) -> &LintSettings {
        &self.settings
    }

    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

/// Names of all built-in syntax-only lints.
pub const FAST_LINT_NAMES: &[&str] = &[
    // Existing lints
    "modern_module_syntax",
    "redundant_self_import",
    "prefer_to_string",
    "prefer_vector_methods",
    "modern_method_syntax",
    "merge_test_attributes",
    "constant_naming",
    "unneeded_return",
    "unnecessary_public_entry",
    "public_mut_tx_context",
    "while_true_to_loop",
    // P0 lints (Zero FP)
    "abilities_order",
    "doc_comment_style",
    "explicit_self_assignments",
    "test_abort_code",
    "redundant_test_prefix",
    // P1 lints (Near-zero FP)
    "equality_in_assert",
    "admin_cap_position",
    "manual_option_check",
    "manual_loop_iteration",
    // Additional stable lints
    "event_suffix",
    "empty_vector_literal",
    "typed_abort_code",
    // Security lints (audit-backed, see docs/SECURITY_LINTS.md)
    "stale_oracle_price",
    "single_step_ownership_transfer",
    "missing_witness_drop",
    "public_random_access",
    "suspicious_overflow_check", // Promoted to stable
    "ignored_boolean_return",    // Typus hack pattern
    // Deprecated lints (kept for backward compatibility)
    "droppable_hot_potato",      // DEPRECATED: Use droppable_hot_potato_v2
    "shared_capability",         // DEPRECATED: Use share_owned_authority
    "shared_capability_object",  // DEPRECATED: Use share_owned_authority (type-based)
    "unchecked_coin_split",      // DEPRECATED: Sui runtime protects
    "capability_leak",           // DEPRECATED: Name-based, needs type-based rewrite
    "unchecked_withdrawal",      // DEPRECATED: Name-based, needs CFG-based rewrite
    // Preview lints (require --preview flag)
    "pure_function_transfer",
    "unsafe_arithmetic",
];

const FULL_MODE_SUPERSEDED_LINTS: &[&str] = &["public_mut_tx_context", "unnecessary_public_entry"];

/// Names of all built-in semantic lints.
pub const SEMANTIC_LINT_NAMES: &[&str] = &[
    "capability_naming",
    "event_naming",
    "getter_naming",
    "share_owned",
    "self_transfer",
    "custom_state_change",
    "coin_field",
    "freeze_wrapped",
    "collection_equality",
    "public_random",
    "missing_key",
    "freezing_capability",
    // Security semantic lints (audit-backed, see docs/SECURITY_LINTS.md)
    "unfrozen_coin_metadata",
    "unused_capability_param",
    "unchecked_division",
    "oracle_zero_price",
    "unused_return_value",
    "missing_access_control",
    // Phase II (AbsInt) lints (require --mode full --preview)
    "unused_capability_param_v2",
    "unchecked_division_v2",
    // Phase III (cross-module) lints (require --mode full --preview)
    "transitive_capability_leak",
    "flashloan_without_repay",
];

pub fn is_semantic_lint(name: &str) -> bool {
    SEMANTIC_LINT_NAMES.contains(&name)
}

// ============================================================================
// Lint Name Aliases (Backward Compatibility)
// ============================================================================

/// Mapping of old lint names to their current canonical names.
///
/// When renaming a lint, add an entry here to maintain backward compatibility.
/// Users can still reference the old name in config files and CLI arguments.
///
/// Format: (old_name, canonical_name)
pub const LINT_ALIASES: &[(&str, &str)] = &[
    // Example aliases for potential future renames:
    // ("legacy_module_block", "modern_module_syntax"),
    // ("utf8_string_import", "prefer_to_string"),
    // ("get_prefix_getter", "getter_naming"),
];

/// Resolve a lint name to its canonical form.
///
/// If the name is an alias, returns the canonical name.
/// Otherwise, returns the original name unchanged.
pub fn resolve_lint_alias(name: &str) -> &str {
    for (alias, canonical) in LINT_ALIASES {
        if *alias == name {
            return canonical;
        }
    }
    name
}

/// Check if a name is a known alias (not the canonical name).
pub fn is_lint_alias(name: &str) -> bool {
    LINT_ALIASES.iter().any(|(alias, _)| *alias == name)
}

/// Get all known lint names including aliases.
pub fn all_known_lints_with_aliases() -> HashSet<&'static str> {
    let mut known = all_known_lints();
    for (alias, _) in LINT_ALIASES {
        known.insert(alias);
    }
    known
}

pub fn all_known_lints() -> HashSet<&'static str> {
    FAST_LINT_NAMES
        .iter()
        .copied()
        .chain(SEMANTIC_LINT_NAMES.iter().copied())
        .collect()
}

/// Registry of syntax-only lint rules used by the fast-mode engine.
pub struct LintRegistry {
    rules: Vec<Box<dyn LintRule>>,
}

impl Default for LintRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl LintRegistry {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn with_rule(mut self, rule: impl LintRule + 'static) -> Self {
        self.rules.push(Box::new(rule));
        self
    }

    pub fn rules(&self) -> impl Iterator<Item = &Box<dyn LintRule>> {
        self.rules.iter()
    }

    pub fn descriptors(&self) -> impl Iterator<Item = &'static LintDescriptor> + '_ {
        self.rules.iter().map(|r| r.descriptor())
    }

    pub fn find_descriptor(&self, name: &str) -> Option<&'static LintDescriptor> {
        self.descriptors().find(|d| d.name == name)
    }

    pub fn default_rules() -> Self {
        Self::new()
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
            .with_rule(crate::rules::DroppableHotPotatoLint)
            .with_rule(crate::rules::SharedCapabilityLint)
            .with_rule(crate::rules::StaleOraclePriceLint)
            .with_rule(crate::rules::SingleStepOwnershipTransferLint)
            .with_rule(crate::rules::MissingWitnessDropLint)
            .with_rule(crate::rules::PublicRandomAccessLint)
            .with_rule(crate::rules::SuspiciousOverflowCheckLint) // Promoted to stable
            .with_rule(crate::rules::IgnoredBooleanReturnLint) // NEW: Typus hack pattern
            .with_rule(crate::rules::SharedCapabilityObjectLint) // NEW: Typus hack pattern
            // Preview lints (only included when preview mode enabled)
            .with_rule(crate::rules::PureFunctionTransferLint)
            .with_rule(crate::rules::UnsafeArithmeticLint)
            .with_rule(crate::rules::UncheckedCoinSplitLint)
            .with_rule(crate::rules::UncheckedWithdrawalLint) // NEW: Thala hack pattern
            .with_rule(crate::rules::CapabilityLeakLint) // NEW: MoveScanner pattern
    }

    pub fn default_rules_filtered(
        only: &[String],
        skip: &[String],
        disabled: &[String],
        full_mode: bool,
        preview: bool,
    ) -> Result<Self> {
        // Note: experimental flag implies preview
        Self::default_rules_filtered_with_experimental(
            only, skip, disabled, full_mode, preview, false,
        )
    }

    /// Filter rules with full tier support including experimental.
    pub fn default_rules_filtered_with_experimental(
        only: &[String],
        skip: &[String],
        disabled: &[String],
        full_mode: bool,
        preview: bool,
        experimental: bool,
    ) -> Result<Self> {
        // Experimental implies preview
        let effective_preview = preview || experimental;
        // Use the extended set that includes aliases for validation
        let known = all_known_lints_with_aliases();

        for n in only.iter().chain(skip.iter()).chain(disabled.iter()) {
            if !known.contains(n.as_str()) {
                return Err(anyhow!("unknown lint: {n}"));
            }
        }

        // Resolve aliases to canonical names for filtering
        let only_resolved: Vec<&str> = only.iter().map(|s| resolve_lint_alias(s)).collect();
        let skip_resolved: Vec<&str> = skip.iter().map(|s| resolve_lint_alias(s)).collect();
        let disabled_resolved: Vec<&str> = disabled.iter().map(|s| resolve_lint_alias(s)).collect();

        let only_set: Option<HashSet<&str>> = if only_resolved.is_empty() {
            None
        } else {
            Some(only_resolved.into_iter().collect())
        };

        let skip_set: HashSet<&str> = skip_resolved.into_iter().collect();
        let disabled_set: HashSet<&str> = disabled_resolved.into_iter().collect();

        let mut reg = Self::new();
        for name in FAST_LINT_NAMES {
            if full_mode && FULL_MODE_SUPERSEDED_LINTS.iter().any(|l| l == name) {
                continue;
            }
            if let Some(ref only) = only_set
                && !only.contains(name)
            {
                continue;
            }
            if skip_set.contains(name) || disabled_set.contains(name) {
                continue;
            }

            // Get the rule's group and filter based on tier flags
            let group = get_lint_group(name);
            match group {
                RuleGroup::Preview if !effective_preview => continue,
                RuleGroup::Experimental if !experimental => continue,
                _ => {}
            }

            match *name {
                "modern_module_syntax" => {
                    reg = reg.with_rule(crate::rules::ModernModuleSyntaxLint);
                }
                "redundant_self_import" => {
                    reg = reg.with_rule(crate::rules::RedundantSelfImportLint);
                }
                "prefer_to_string" => {
                    reg = reg.with_rule(crate::rules::PreferToStringLint);
                }
                "prefer_vector_methods" => {
                    reg = reg.with_rule(crate::rules::PreferVectorMethodsLint);
                }
                "modern_method_syntax" => {
                    reg = reg.with_rule(crate::rules::ModernMethodSyntaxLint);
                }
                "merge_test_attributes" => {
                    reg = reg.with_rule(crate::rules::MergeTestAttributesLint);
                }
                "constant_naming" => {
                    reg = reg.with_rule(crate::rules::ConstantNamingLint);
                }
                "unneeded_return" => {
                    reg = reg.with_rule(crate::rules::UnneededReturnLint);
                }
                "unnecessary_public_entry" => {
                    reg = reg.with_rule(crate::rules::UnnecessaryPublicEntryLint);
                }
                "public_mut_tx_context" => {
                    reg = reg.with_rule(crate::rules::PublicMutTxContextLint);
                }
                "while_true_to_loop" => {
                    reg = reg.with_rule(crate::rules::WhileTrueToLoopLint);
                }
                // P0 lints
                "abilities_order" => {
                    reg = reg.with_rule(crate::rules::AbilitiesOrderLint);
                }
                "doc_comment_style" => {
                    reg = reg.with_rule(crate::rules::DocCommentStyleLint);
                }
                "explicit_self_assignments" => {
                    reg = reg.with_rule(crate::rules::ExplicitSelfAssignmentsLint);
                }
                "test_abort_code" => {
                    reg = reg.with_rule(crate::rules::TestAbortCodeLint);
                }
                "redundant_test_prefix" => {
                    reg = reg.with_rule(crate::rules::RedundantTestPrefixLint);
                }
                // P1 lints
                "equality_in_assert" => {
                    reg = reg.with_rule(crate::rules::EqualityInAssertLint);
                }
                "admin_cap_position" => {
                    reg = reg.with_rule(crate::rules::AdminCapPositionLint);
                }
                "manual_option_check" => {
                    reg = reg.with_rule(crate::rules::ManualOptionCheckLint);
                }
                "manual_loop_iteration" => {
                    reg = reg.with_rule(crate::rules::ManualLoopIterationLint);
                }
                // Additional stable lints
                "event_suffix" => {
                    reg = reg.with_rule(crate::rules::EventSuffixLint);
                }
                "empty_vector_literal" => {
                    reg = reg.with_rule(crate::rules::EmptyVectorLiteralLint);
                }
                "typed_abort_code" => {
                    reg = reg.with_rule(crate::rules::TypedAbortCodeLint);
                }
                // Security lints (audit-backed)
                "droppable_hot_potato" => {
                    reg = reg.with_rule(crate::rules::DroppableHotPotatoLint);
                }
                "shared_capability" => {
                    reg = reg.with_rule(crate::rules::SharedCapabilityLint);
                }
                "stale_oracle_price" => {
                    reg = reg.with_rule(crate::rules::StaleOraclePriceLint);
                }
                "single_step_ownership_transfer" => {
                    reg = reg.with_rule(crate::rules::SingleStepOwnershipTransferLint);
                }
                "missing_witness_drop" => {
                    reg = reg.with_rule(crate::rules::MissingWitnessDropLint);
                }
                "public_random_access" => {
                    reg = reg.with_rule(crate::rules::PublicRandomAccessLint);
                }
                "suspicious_overflow_check" => {
                    reg = reg.with_rule(crate::rules::SuspiciousOverflowCheckLint);
                }
                "ignored_boolean_return" => {
                    reg = reg.with_rule(crate::rules::IgnoredBooleanReturnLint);
                }
                "shared_capability_object" => {
                    reg = reg.with_rule(crate::rules::SharedCapabilityObjectLint);
                }
                // Preview lints
                "pure_function_transfer" => {
                    reg = reg.with_rule(crate::rules::PureFunctionTransferLint);
                }
                "unsafe_arithmetic" => {
                    reg = reg.with_rule(crate::rules::UnsafeArithmeticLint);
                }
                "unchecked_withdrawal" => {
                    reg = reg.with_rule(crate::rules::UncheckedWithdrawalLint);
                }
                "capability_leak" => {
                    reg = reg.with_rule(crate::rules::CapabilityLeakLint);
                }
                "unchecked_coin_split" => {
                    reg = reg.with_rule(crate::rules::UncheckedCoinSplitLint);
                }
                other => unreachable!("unexpected fast lint name: {other}"),
            }
        }

        Ok(reg)
    }
}

/// Get the RuleGroup for a lint by name.
/// This is used during filtering to determine if a lint should be enabled.
fn get_lint_group(name: &str) -> RuleGroup {
    // All P0 and P1 lints are stable
    match name {
        "modern_module_syntax"
        | "redundant_self_import"
        | "prefer_to_string"
        | "prefer_vector_methods"
        | "modern_method_syntax"
        | "merge_test_attributes"
        | "constant_naming"
        | "unneeded_return"
        | "unnecessary_public_entry"
        | "public_mut_tx_context"
        | "while_true_to_loop"
        // P0 lints
        | "abilities_order"
        | "doc_comment_style"
        | "explicit_self_assignments"
        | "test_abort_code"
        | "redundant_test_prefix"
        // P1 lints
        | "equality_in_assert"
        | "admin_cap_position"
        | "manual_option_check"
        | "manual_loop_iteration"
        // Additional stable lints
        | "event_suffix"
        | "empty_vector_literal"
        | "typed_abort_code"
        // Security lints (audit-backed, stable)
        | "stale_oracle_price"
        | "single_step_ownership_transfer"
        | "missing_witness_drop"
        | "public_random_access"
        | "suspicious_overflow_check"     // Promoted to stable
        | "ignored_boolean_return" => RuleGroup::Stable,  // Typus hack pattern

        // Deprecated lints (name-based, not recommended)
        | "droppable_hot_potato"      // Use droppable_hot_potato_v2 (type-based)
        | "shared_capability"         // Use share_owned_authority (type-based)
        | "shared_capability_object"  // Use share_owned_authority (type-based)
        | "capability_naming"         // Sui uses Cap suffix, not _cap
        | "event_naming"              // Sui events don't use _event suffix
        | "getter_naming" => RuleGroup::Deprecated,  // Sui uses get_ prefix

        // Experimental lints (high FP risk, require --experimental flag)
        | "unchecked_coin_split"      // Name-based, high FP
        | "capability_leak"           // Name-based, needs type-based rewrite
        | "unchecked_withdrawal"      // Name-based, needs CFG-based rewrite
        | "pure_function_transfer"
        | "unsafe_arithmetic" => RuleGroup::Experimental,

        // Default to stable for unknown lints
        _ => RuleGroup::Stable,
    }
}
