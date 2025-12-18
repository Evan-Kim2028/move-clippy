//! Generate `docs/LINT_REFERENCE.md` from the unified lint registry.
//!
//! Usage:
//!   cargo run --features full --bin gen_lint_reference > docs/LINT_REFERENCE.md

use move_clippy::lint::{AnalysisKind, RuleGroup};
use move_clippy::unified::{LintPhase, unified_registry};

fn escape_md_cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

fn phase_label(phase: LintPhase) -> &'static str {
    match phase {
        LintPhase::Syntactic => "syntactic",
        LintPhase::Semantic => "semantic",
        LintPhase::AbstractInterpretation => "absint",
        LintPhase::CrossModule => "cross-module",
    }
}

fn requirements(analysis: AnalysisKind, tier: RuleGroup) -> String {
    let mut parts: Vec<&'static str> = Vec::new();
    if analysis.requires_full_mode() {
        parts.push("--mode full");
    }
    if let Some(flag) = tier.required_flag() {
        parts.push(flag);
    }
    if parts.is_empty() {
        "fast".to_string()
    } else {
        parts.join(" ")
    }
}

fn main() {
    let reg = unified_registry();

    let mut rows: Vec<(
        &'static str,
        &'static move_clippy::lint::LintDescriptor,
        LintPhase,
    )> = reg
        .all()
        .map(|l| (l.descriptor.name, l.descriptor, l.phase))
        .collect();
    rows.sort_by_key(|(name, _, _)| *name);

    let total = rows.len();
    let stable = rows
        .iter()
        .filter(|(_, d, _)| d.group == RuleGroup::Stable)
        .count();
    let preview = rows
        .iter()
        .filter(|(_, d, _)| d.group == RuleGroup::Preview)
        .count();
    let experimental = rows
        .iter()
        .filter(|(_, d, _)| d.group == RuleGroup::Experimental)
        .count();
    let deprecated = rows
        .iter()
        .filter(|(_, d, _)| d.group == RuleGroup::Deprecated)
        .count();

    println!("# Move-Clippy Lint Reference\n");
    println!("**Status:** Generated (do not edit by hand)\n");
    println!("This file is generated from the unified lint registry.\n");
    println!("Regenerate with:\n");
    println!("```bash");
    println!("cargo run --features full --bin gen_lint_reference > docs/LINT_REFERENCE.md");
    println!("```\n");

    println!("## Summary\n");
    println!("- Total: {total}");
    println!("- Stable: {stable}");
    println!("- Preview: {preview}");
    println!("- Experimental: {experimental}");
    println!("- Deprecated: {deprecated}\n");

    println!("## Lints\n");
    println!("| Lint | Tier | Phase | Category | Analysis | Requires | Description |");
    println!("|------|------|-------|----------|----------|----------|-------------|");

    for (_name, desc, phase) in rows {
        let lint = escape_md_cell(desc.name);
        let tier = escape_md_cell(desc.group.as_str());
        let phase = escape_md_cell(phase_label(phase));
        let category = escape_md_cell(desc.category.as_str());
        let analysis = escape_md_cell(desc.analysis.as_str());
        let requires = escape_md_cell(&requirements(desc.analysis, desc.group));
        let description = escape_md_cell(desc.description);

        println!(
            "| `{lint}` | {tier} | {phase} | {category} | {analysis} | `{requires}` | {description} |"
        );
    }
}
