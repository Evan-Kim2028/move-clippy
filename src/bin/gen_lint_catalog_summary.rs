//! Generate `docs/LINT_CATALOG_SUMMARY.md` from the unified lint registry.
//!
//! Usage:
//!   cargo run --features full --bin gen_lint_catalog_summary > docs/LINT_CATALOG_SUMMARY.md

use move_clippy::lint::{AnalysisKind, LintCategory, RuleGroup};
use move_clippy::unified::{LintPhase, unified_registry};
use std::collections::BTreeMap;

fn count_by_key<K: Ord>(items: impl Iterator<Item = K>) -> BTreeMap<K, usize> {
    let mut out: BTreeMap<K, usize> = BTreeMap::new();
    for key in items {
        *out.entry(key).or_insert(0) += 1;
    }
    out
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
    let lints: Vec<_> = reg.all().collect();

    let total = lints.len();
    let by_tier = count_by_key(lints.iter().map(|l| l.descriptor.group));
    let by_phase = count_by_key(lints.iter().map(|l| l.phase));
    let by_category = count_by_key(lints.iter().map(|l| l.descriptor.category));
    let by_analysis = count_by_key(lints.iter().map(|l| l.descriptor.analysis));

    println!("# Move-Clippy Lint Catalog Summary\n");
    println!("**Status:** Generated (do not edit by hand)\n");
    println!("This file is generated from the unified lint registry.\n");
    println!("Regenerate with:\n");
    println!("```bash");
    println!(
        "cargo run --features full --bin gen_lint_catalog_summary > docs/LINT_CATALOG_SUMMARY.md"
    );
    println!("```\n");

    println!("## Totals\n");
    println!("- Total lints: {total}\n");

    println!("## By Tier\n");
    println!("| Tier | Count |");
    println!("|------|-------|");
    for tier in [
        RuleGroup::Stable,
        RuleGroup::Preview,
        RuleGroup::Experimental,
        RuleGroup::Deprecated,
    ] {
        let count = by_tier.get(&tier).copied().unwrap_or(0);
        println!("| {} | {} |", tier.as_str(), count);
    }
    println!();

    println!("## By Phase\n");
    println!("| Phase | Count |");
    println!("|-------|-------|");
    for phase in [
        LintPhase::Syntactic,
        LintPhase::Semantic,
        LintPhase::AbstractInterpretation,
        LintPhase::CrossModule,
    ] {
        let count = by_phase.get(&phase).copied().unwrap_or(0);
        println!("| {} | {} |", phase.as_str(), count);
    }
    println!();

    println!("## By Category\n");
    println!("| Category | Count |");
    println!("|----------|-------|");
    for category in [
        LintCategory::Style,
        LintCategory::Modernization,
        LintCategory::Naming,
        LintCategory::Security,
        LintCategory::Suspicious,
        LintCategory::TestQuality,
    ] {
        let count = by_category.get(&category).copied().unwrap_or(0);
        println!("| {} | {} |", category.as_str(), count);
    }
    println!();

    println!("## By Analysis Kind\n");
    println!("| Analysis | Count | Requires |");
    println!("|----------|-------|----------|");
    for analysis in [
        AnalysisKind::Syntactic,
        AnalysisKind::TypeBased,
        AnalysisKind::TypeBasedCFG,
        AnalysisKind::CrossModule,
    ] {
        let count = by_analysis.get(&analysis).copied().unwrap_or(0);
        let requires = requirements(analysis, RuleGroup::Stable);
        println!("| {} | {} | `{}` |", analysis.as_str(), count, requires);
    }
}
