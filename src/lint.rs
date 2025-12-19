use crate::annotations;
use crate::diagnostics::{Diagnostic, Span, Suggestion};
use crate::level::LintLevel;
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
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

    /// Deprecated rules that are no longer active.
    /// These are kept for backwards compatibility but produce no diagnostics.
    /// Require `--experimental` flag to be included (for config compatibility).
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
            RuleGroup::Deprecated => Some("--experimental"), // Deprecated lints require experimental flag
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
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
    /// Requires `--mode full` and is typically gated by `--experimental` due to cost.
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
        matches!(self, AnalysisKind::TypeBasedCFG)
    }
}

// ============================================================================
// Fix Safety Classification - Inspired by Ruff
// ============================================================================

/// Safety classification for auto-fixes.
///
/// - `Safe` fixes preserve runtime behavior exactly
/// - `Unsafe` fixes may change runtime behavior and require explicit opt-in
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

// ============================================================================
// Type System Gap Classification
// ============================================================================

/// Classification of the type system gap that a lint addresses.
///
/// This helps understand WHY a lint exists and guides systematic discovery
/// of new lints.
///
/// **For users:** See [docs/TYPE_SYSTEM_GAPS.md](../docs/TYPE_SYSTEM_GAPS.md) for detailed gap taxonomy and examples.
///
/// **For developers:** Use this enum to classify new lints systematically. Ask "what invariant does the type system NOT enforce?"
///
/// # Gap Categories
///
/// - **AbilityMismatch**: Wrong ability combinations (e.g., hot potato with drop)
/// - **OwnershipViolation**: Incorrect object ownership transitions
/// - **CapabilityEscape**: Admin/sensitive capabilities leaking scope
/// - **ValueFlow**: Values going to wrong destinations (e.g., ignored returns)
/// - **ApiMisuse**: Using stdlib functions incorrectly
/// - **TemporalOrdering**: Operations in wrong sequence (e.g., use before check)
/// - **ArithmeticSafety**: Numeric operations without validation
/// - **StyleConvention**: Style/convention issues (no security impact)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypeSystemGap {
    /// Wrong ability combinations (e.g., hot potato with drop, missing key)
    AbilityMismatch,
    /// Incorrect object ownership transitions (e.g., sharing non-fresh objects)
    OwnershipViolation,
    /// Admin/sensitive capabilities leaking scope
    CapabilityEscape,
    /// Values going to wrong destinations (e.g., ignored return values)
    ValueFlow,
    /// Using stdlib functions incorrectly (e.g., Coin in struct field)
    ApiMisuse,
    /// Operations in wrong sequence (e.g., use before check)
    TemporalOrdering,
    /// Numeric operations without validation (e.g., division by zero)
    ArithmeticSafety,
    /// DoS risks via unbounded execution or resource blowups
    ResourceExhaustion,
    /// Generic/type parameter misuse enabling type confusion
    TypeConfusion,
    /// Style/convention issues (no security impact)
    StyleConvention,
}

impl TypeSystemGap {
    pub fn as_str(&self) -> &'static str {
        match self {
            TypeSystemGap::AbilityMismatch => "ability_mismatch",
            TypeSystemGap::OwnershipViolation => "ownership_violation",
            TypeSystemGap::CapabilityEscape => "capability_escape",
            TypeSystemGap::ValueFlow => "value_flow",
            TypeSystemGap::ApiMisuse => "api_misuse",
            TypeSystemGap::TemporalOrdering => "temporal_ordering",
            TypeSystemGap::ArithmeticSafety => "arithmetic_safety",
            TypeSystemGap::ResourceExhaustion => "resource_exhaustion",
            TypeSystemGap::TypeConfusion => "type_confusion",
            TypeSystemGap::StyleConvention => "style_convention",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            TypeSystemGap::AbilityMismatch => "Wrong ability combinations",
            TypeSystemGap::OwnershipViolation => "Incorrect object ownership transitions",
            TypeSystemGap::CapabilityEscape => "Capabilities leaking scope",
            TypeSystemGap::ValueFlow => "Values going to wrong destinations",
            TypeSystemGap::ApiMisuse => "Incorrect stdlib function usage",
            TypeSystemGap::TemporalOrdering => "Operations in wrong sequence",
            TypeSystemGap::ArithmeticSafety => "Numeric operations without validation",
            TypeSystemGap::ResourceExhaustion => "Unbounded execution / resource exhaustion",
            TypeSystemGap::TypeConfusion => "Generic/type misuse (type confusion)",
            TypeSystemGap::StyleConvention => "Style/convention issues",
        }
    }
}

/// Static metadata describing a lint rule.
#[derive(Debug)]
pub struct LintDescriptor {
    pub name: &'static str,
    pub category: LintCategory,
    pub description: &'static str,
    /// Stability group: Stable, Preview, or Experimental.
    pub group: RuleGroup,
    /// Auto-fix availability and safety classification.
    pub fix: FixDescriptor,
    /// Detection method used by this lint.
    pub analysis: AnalysisKind,
    /// The type system gap this lint addresses (for security/suspicious lints).
    /// None for style/convention lints.
    pub gap: Option<TypeSystemGap>,
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
            gap: None,
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
            gap: None,
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
            gap: None,
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
            gap: None,
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
            gap: None,
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
            gap: None,
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
            gap: None,
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
            gap: None,
        }
    }
}

/// A single lint rule that can inspect a syntax tree.
pub trait LintRule: Send + Sync {
    fn descriptor(&self) -> &'static LintDescriptor;
    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>);
}

/// Per-lint configuration derived from `move-clippy.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LintSettings {
    levels: HashMap<String, LintLevel>,
}

impl LintSettings {
    #[must_use]
    pub fn with_config_levels(mut self, levels: HashMap<String, LintLevel>) -> Self {
        // Resolve aliases when storing levels
        for (name, level) in levels {
            let canonical = resolve_lint_alias(&name);
            self.levels.insert(canonical.to_string(), level);
        }
        self
    }

    #[must_use]
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

pub(crate) fn effective_level_for_scopes(
    settings: &LintSettings,
    lint: &'static LintDescriptor,
    module_scope: &annotations::SuppressionScope,
    item_scope: &annotations::SuppressionScope,
) -> LintLevel {
    let mut level = settings.level_for(lint.name);
    let category = lint.category.as_str();

    // Apply module directives (outer scope).
    if module_scope.is_suppressed(lint.name) || module_scope.is_suppressed(category) {
        level = LintLevel::Allow;
    }
    if module_scope.is_denied(lint.name) || module_scope.is_denied(category) {
        level = LintLevel::Error;
    }

    // Apply item directives (inner scope; overrides module).
    if item_scope.is_suppressed(lint.name) || item_scope.is_suppressed(category) {
        level = LintLevel::Allow;
    }
    if item_scope.is_denied(lint.name) || item_scope.is_denied(category) {
        level = LintLevel::Error;
    }

    // `expect` is a testing invariant: don't silently drop expected lints.
    if level == LintLevel::Allow
        && (module_scope.is_expected(lint.name)
            || module_scope.is_expected(category)
            || item_scope.is_expected(lint.name)
            || item_scope.is_expected(category))
    {
        level = LintLevel::Warn;
    }

    level
}

/// Mutable context passed to lint rules while traversing a file.
pub struct LintContext<'src> {
    source: &'src str,
    settings: LintSettings,
    diagnostics: Vec<Diagnostic>,
    module_scope: annotations::SuppressionScope,
    module_expected_unfired: HashSet<String>,
    item_scope_cache: HashMap<usize, annotations::SuppressionScope>,
    item_expected_unfired: HashMap<usize, HashSet<String>>,
}

impl<'src> LintContext<'src> {
    pub fn new(source: &'src str, settings: LintSettings) -> Self {
        let module_scope = annotations::module_scope(source);
        let module_expected_unfired = module_scope
            .unfired_expectations()
            .cloned()
            .collect::<HashSet<_>>();

        Self {
            source,
            settings,
            diagnostics: Vec::new(),
            module_scope,
            module_expected_unfired,
            item_scope_cache: HashMap::new(),
            item_expected_unfired: HashMap::new(),
        }
    }

    /// Precollect per-item directive scopes (notably `#[expect(...)]`) so they can be enforced
    /// even when a scope produces zero diagnostics.
    pub(crate) fn precollect_item_directives(&mut self, root: Node) {
        let mut seen: HashSet<usize> = HashSet::new();
        self.precollect_item_directives_rec(root, &mut seen);
    }

    fn precollect_item_directives_rec(&mut self, node: Node, seen: &mut HashSet<usize>) {
        if is_directive_item_kind(node.kind()) {
            let start = node.start_byte();
            if seen.insert(start) {
                self.ensure_item_scope_cached(start);
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.precollect_item_directives_rec(child, seen);
        }
    }

    fn ensure_item_scope_cached(&mut self, anchor_start_byte: usize) {
        if self.item_scope_cache.contains_key(&anchor_start_byte) {
            return;
        }

        let scope = annotations::item_scope(self.source, anchor_start_byte);
        let expected_unfired = scope
            .unfired_expectations()
            .cloned()
            .collect::<HashSet<_>>();
        if !expected_unfired.is_empty() {
            self.item_expected_unfired
                .insert(anchor_start_byte, expected_unfired);
        }

        self.item_scope_cache.insert(anchor_start_byte, scope);
    }

    fn effective_level_for_anchor(
        &mut self,
        lint: &'static LintDescriptor,
        anchor_start_byte: usize,
    ) -> LintLevel {
        self.ensure_item_scope_cached(anchor_start_byte);
        let item_scope = self
            .item_scope_cache
            .get(&anchor_start_byte)
            .expect("item scope should be cached");

        effective_level_for_scopes(&self.settings, lint, &self.module_scope, item_scope)
    }

    fn mark_expected_fired(&mut self, anchor_start_byte: usize, lint: &'static LintDescriptor) {
        let lint_name = lint.name;
        let category = lint.category.as_str();

        if self.module_scope.is_expected(lint_name) {
            self.module_expected_unfired.remove(lint_name);
        }
        if self.module_scope.is_expected(category) {
            self.module_expected_unfired.remove(category);
        }

        if let Some(unfired) = self.item_expected_unfired.get_mut(&anchor_start_byte) {
            unfired.remove(lint_name);
            unfired.remove(category);
            if unfired.is_empty() {
                self.item_expected_unfired.remove(&anchor_start_byte);
            }
        }
    }

    pub fn report(
        &mut self,
        lint: &'static LintDescriptor,
        span: Span,
        message: impl Into<String>,
    ) {
        let mut level = self.settings.level_for(lint.name);
        if self.module_scope.is_suppressed(lint.name)
            || self.module_scope.is_suppressed(lint.category.as_str())
        {
            level = LintLevel::Allow;
        }
        if self.module_scope.is_denied(lint.name)
            || self.module_scope.is_denied(lint.category.as_str())
        {
            level = LintLevel::Error;
        }
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
        let anchor_start_byte = crate::suppression::anchor_item_start_byte(node);
        let level = self.effective_level_for_anchor(lint, anchor_start_byte);
        if level == LintLevel::Allow {
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

        self.mark_expected_fired(anchor_start_byte, lint);
    }

    /// Report a diagnostic directly.
    ///
    /// Note: This does NOT apply suppression logic because it has no node/span
    /// anchoring information. Prefer `report_node`, `report_node_diagnostic`, or
    /// `report_diagnostic_for_node` for tree-sitter based lints.
    pub fn report_diagnostic(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    /// Report an already-constructed diagnostic, enforcing allow/suppression using `node`.
    ///
    /// This is intended for lints that build `Diagnostic` objects to include
    /// suggestions and other fields, but still need to respect `#[allow]` and
    /// `#![allow]` directives.
    pub fn report_diagnostic_for_node(&mut self, node: Node, mut diagnostic: Diagnostic) {
        let anchor_start_byte = crate::suppression::anchor_item_start_byte(node);
        let level = self.effective_level_for_anchor(diagnostic.lint, anchor_start_byte);
        if level == LintLevel::Allow {
            return;
        }

        let lint_descriptor = diagnostic.lint;
        diagnostic.level = level;
        self.diagnostics.push(diagnostic);
        self.mark_expected_fired(anchor_start_byte, lint_descriptor);
    }

    pub fn report_span_with_anchor(
        &mut self,
        lint: &'static LintDescriptor,
        anchor_start_byte: usize,
        span: Span,
        message: impl Into<String>,
    ) {
        let level = self.effective_level_for_anchor(lint, anchor_start_byte);
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

        self.mark_expected_fired(anchor_start_byte, lint);
    }

    pub fn report_span_diagnostic_with_anchor(
        &mut self,
        lint: &'static LintDescriptor,
        anchor_start_byte: usize,
        span: Span,
        message: impl Into<String>,
        help: Option<String>,
        suggestion: Option<Suggestion>,
    ) {
        let level = self.effective_level_for_anchor(lint, anchor_start_byte);
        if level == LintLevel::Allow {
            return;
        }

        self.diagnostics.push(Diagnostic {
            lint,
            level,
            file: None,
            span,
            message: message.into(),
            help,
            suggestion,
        });

        self.mark_expected_fired(anchor_start_byte, lint);
    }

    pub fn source(&self) -> &'src str {
        self.source
    }

    pub fn settings(&self) -> &LintSettings {
        &self.settings
    }

    #[must_use]
    pub fn into_diagnostics(mut self) -> Vec<Diagnostic> {
        self.append_unfulfilled_expectation_diagnostics();
        self.diagnostics
    }

    fn append_unfulfilled_expectation_diagnostics(&mut self) {
        let mut module_unfired: Vec<String> = self.module_expected_unfired.drain().collect();
        module_unfired.sort();
        for lint_name in module_unfired {
            self.diagnostics.push(Diagnostic {
                lint: &UNFULFILLED_EXPECTATION,
                level: LintLevel::Error,
                file: None,
                span: Span {
                    start: crate::diagnostics::Position { row: 1, column: 1 },
                    end: crate::diagnostics::Position { row: 1, column: 1 },
                },
                message: format!(
                    "Expected `lint::{}` to produce a diagnostic in this file, but it did not",
                    lint_name
                ),
                help: Some(
                    "Remove the `#![expect(...)]` directive or adjust the code/lint so it triggers."
                        .to_string(),
                ),
                suggestion: None,
            });
        }

        let mut anchors: Vec<usize> = self.item_expected_unfired.keys().copied().collect();
        anchors.sort();
        for anchor in anchors {
            let Some(mut unfired) = self.item_expected_unfired.remove(&anchor) else {
                continue;
            };

            let mut lint_names: Vec<String> = unfired.drain().collect();
            lint_names.sort();

            let pos = position_from_byte_offset(self.source, anchor);
            let span = Span {
                start: pos,
                end: pos,
            };

            for lint_name in lint_names {
                self.diagnostics.push(Diagnostic {
                    lint: &UNFULFILLED_EXPECTATION,
                    level: LintLevel::Error,
                    file: None,
                    span,
                    message: format!(
                        "Expected `lint::{}` to produce a diagnostic in this scope, but it did not",
                        lint_name
                    ),
                    help: Some(
                        "Remove the `#[expect(...)]` directive or adjust the code/lint so it triggers."
                            .to_string(),
                    ),
                    suggestion: None,
                });
            }
        }
    }
}

const FULL_MODE_SUPERSEDED_LINTS: &[&str] = &["public_mut_tx_context", "unnecessary_public_entry"];

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
    crate::unified::unified_registry()
        .descriptors()
        .map(|d| d.name)
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
    #[must_use]
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    #[must_use]
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

    #[must_use = "registry should be used to create an engine"]
    pub fn default_rules() -> Self {
        crate::unified::build_syntactic_registry()
    }

    /// Returns error if any lint name in `only`, `skip`, or `disabled` is unknown.
    pub fn default_rules_filtered(
        only: &[String],
        skip: &[String],
        disabled: &[String],
        full_mode: bool,
        preview: bool,
    ) -> Result<Self> {
        Self::default_rules_filtered_with_experimental(
            only, skip, disabled, full_mode, preview, false,
        )
    }

    /// Filter rules with full tier support including experimental.
    ///
    /// # Errors
    ///
    /// Returns error if any lint name in `only`, `skip`, or `disabled` is unknown.
    pub fn default_rules_filtered_with_experimental(
        only: &[String],
        skip: &[String],
        disabled: &[String],
        full_mode: bool,
        preview: bool,
        experimental: bool,
    ) -> Result<Self> {
        // Note: experimental flag implies preview
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
        let all = Self::default_rules();
        for rule in all.rules {
            let descriptor = rule.descriptor();
            let name = descriptor.name;

            if full_mode && FULL_MODE_SUPERSEDED_LINTS.iter().any(|l| *l == name) {
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

            match descriptor.group {
                RuleGroup::Preview if !effective_preview => continue,
                RuleGroup::Experimental if !experimental => continue,
                _ => {}
            }

            reg.rules.push(rule);
        }

        Ok(reg)
    }
}

/// Descriptor for an unfulfilled expectation diagnostic.
pub(crate) static UNFULFILLED_EXPECTATION: LintDescriptor = LintDescriptor {
    name: "unfulfilled_expectation",
    category: LintCategory::TestQuality,
    description: "An #[expect(lint::...)] directive did not match any emitted diagnostics",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::Syntactic,
    gap: None,
};

pub(crate) fn is_directive_item_kind(kind: &str) -> bool {
    if kind == "module_definition" || kind == "use_declaration" {
        return true;
    }

    // Be resilient to grammar naming differences.
    kind.contains("function")
        || kind.contains("struct")
        || kind.contains("enum")
        || kind.contains("constant")
}

fn position_from_byte_offset(source: &str, byte_offset: usize) -> crate::diagnostics::Position {
    let mut row = 1usize;
    let mut col = 1usize;

    let end = byte_offset.min(source.len());
    for b in source.as_bytes().iter().take(end) {
        if *b == b'\n' {
            row += 1;
            col = 1;
        } else {
            col += 1;
        }
    }

    crate::diagnostics::Position { row, column: col }
}
