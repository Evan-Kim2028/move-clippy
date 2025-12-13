use crate::diagnostics::{Diagnostic, Span};
use crate::level::LintLevel;
use crate::suppression;
use anyhow::{Result, anyhow};
use std::collections::{HashMap, HashSet};
use tree_sitter::Node;

#[derive(Debug, Clone, Copy)]
pub enum LintCategory {
    Style,
    Modernization,
    Naming,
    TestQuality,
    Suspicious,
}

impl LintCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            LintCategory::Style => "style",
            LintCategory::Modernization => "modernization",
            LintCategory::Naming => "naming",
            LintCategory::TestQuality => "test_quality",
            LintCategory::Suspicious => "suspicious",
        }
    }
}

#[derive(Debug)]
pub struct LintDescriptor {
    pub name: &'static str,
    pub category: LintCategory,
    pub description: &'static str,
}

pub trait LintRule: Send + Sync {
    fn descriptor(&self) -> &'static LintDescriptor;
    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>);
}

#[derive(Debug, Clone, Default)]
pub struct LintSettings {
    levels: HashMap<String, LintLevel>,
}

impl LintSettings {
    pub fn with_config_levels(mut self, levels: HashMap<String, LintLevel>) -> Self {
        self.levels.extend(levels);
        self
    }

    pub fn disable(mut self, disabled: impl IntoIterator<Item = String>) -> Self {
        for name in disabled {
            self.levels.insert(name, LintLevel::Allow);
        }
        self
    }

    pub fn level_for(&self, lint_name: &str) -> LintLevel {
        self.levels.get(lint_name).copied().unwrap_or_default()
    }
}

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

pub const FAST_LINT_NAMES: &[&str] = &[
    "modern_module_syntax",
    "redundant_self_import",
    "prefer_to_string",
    "prefer_vector_methods",
    "modern_method_syntax",
    "merge_test_attributes",
];

pub const SEMANTIC_LINT_NAMES: &[&str] = &["capability_naming", "event_naming", "getter_naming"];

pub fn is_semantic_lint(name: &str) -> bool {
    SEMANTIC_LINT_NAMES.iter().any(|n| *n == name)
}

pub fn all_known_lints() -> HashSet<&'static str> {
    FAST_LINT_NAMES
        .iter()
        .copied()
        .chain(SEMANTIC_LINT_NAMES.iter().copied())
        .collect()
}

pub struct LintRegistry {
    rules: Vec<Box<dyn LintRule>>,
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
            .with_rule(crate::rules::ModernModuleSyntaxLint)
            .with_rule(crate::rules::RedundantSelfImportLint)
            .with_rule(crate::rules::PreferToStringLint)
            .with_rule(crate::rules::PreferVectorMethodsLint)
            .with_rule(crate::rules::ModernMethodSyntaxLint)
            .with_rule(crate::rules::MergeTestAttributesLint)
    }

    pub fn default_rules_filtered(
        only: &[String],
        skip: &[String],
        disabled: &[String],
    ) -> Result<Self> {
        let known = all_known_lints();

        for n in only.iter().chain(skip.iter()).chain(disabled.iter()) {
            if !known.contains(n.as_str()) {
                return Err(anyhow!("unknown lint: {n}"));
            }
        }

        let only_set: Option<HashSet<&str>> = if only.is_empty() {
            None
        } else {
            Some(only.iter().map(|s| s.as_str()).collect())
        };
        let skip_set: HashSet<&str> = skip.iter().map(|s| s.as_str()).collect();
        let disabled_set: HashSet<&str> = disabled.iter().map(|s| s.as_str()).collect();

        let include = |name: &'static str| {
            let selected =
                only_set.as_ref().map_or(true, |s| s.contains(name)) && !skip_set.contains(name);
            if !selected {
                return false;
            }

            // Config-disabled lints are excluded unless explicitly selected.
            if disabled_set.contains(name) {
                return only_set.as_ref().map_or(false, |s| s.contains(name));
            }

            true
        };

        let mut reg = Self::new();
        if include("modern_module_syntax") {
            reg = reg.with_rule(crate::rules::ModernModuleSyntaxLint);
        }
        if include("redundant_self_import") {
            reg = reg.with_rule(crate::rules::RedundantSelfImportLint);
        }
        if include("prefer_to_string") {
            reg = reg.with_rule(crate::rules::PreferToStringLint);
        }
        if include("prefer_vector_methods") {
            reg = reg.with_rule(crate::rules::PreferVectorMethodsLint);
        }
        if include("modern_method_syntax") {
            reg = reg.with_rule(crate::rules::ModernMethodSyntaxLint);
        }
        if include("merge_test_attributes") {
            reg = reg.with_rule(crate::rules::MergeTestAttributesLint);
        }

        Ok(reg)
    }
}
