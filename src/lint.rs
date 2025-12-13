use crate::diagnostics::{Diagnostic, Span};
use anyhow::{anyhow, Result};
use std::collections::HashSet;
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

pub struct LintContext<'src> {
    source: &'src str,
    diagnostics: Vec<Diagnostic>,
}

impl<'src> LintContext<'src> {
    pub fn new(source: &'src str) -> Self {
        Self {
            source,
            diagnostics: Vec::new(),
        }
    }

    pub fn report(
        &mut self,
        lint: &'static LintDescriptor,
        span: Span,
        message: impl Into<String>,
    ) {
        self.diagnostics.push(Diagnostic {
            lint,
            span,
            message: message.into(),
            help: None,
            suggestion: None,
        });
    }

    pub fn source(&self) -> &'src str {
        self.source
    }

    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
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

    pub fn default_rules_filtered(only: &[String], skip: &[String]) -> Result<Self> {
        let known: HashSet<&'static str> = [
            "modern_module_syntax",
            "redundant_self_import",
            "prefer_to_string",
            "prefer_vector_methods",
            "modern_method_syntax",
            "merge_test_attributes",
        ]
        .into_iter()
        .collect();

        for n in only.iter().chain(skip.iter()) {
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

        let include = |name: &'static str| {
            only_set.as_ref().map_or(true, |s| s.contains(name)) && !skip_set.contains(name)
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
