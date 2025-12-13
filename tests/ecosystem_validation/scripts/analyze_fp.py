#!/usr/bin/env python3
"""
analyze_fp.py - Analyze False Positive patterns in triage database

Usage:
    python scripts/analyze_fp.py [triage.json] [--output FP_PATTERNS.md]

This script:
1. Loads the triage database
2. Groups false positives by lint
3. Extracts common patterns from notes
4. Generates a markdown report with recommendations
"""

import json
import sys
import argparse
from collections import defaultdict
from datetime import datetime
from pathlib import Path


def load_triage(path: str) -> dict:
    """Load triage database from JSON file."""
    with open(path) as f:
        return json.load(f)


def analyze_findings(db: dict) -> dict:
    """Analyze all findings and compute statistics."""
    stats = {
        "total": 0,
        "by_status": defaultdict(int),
        "by_lint": defaultdict(lambda: {"total": 0, "confirmed": 0, "fp": 0, "wont_fix": 0, "needs_review": 0}),
        "by_repo": defaultdict(lambda: {"total": 0, "confirmed": 0, "fp": 0}),
        "by_category": defaultdict(lambda: {"total": 0, "confirmed": 0, "fp": 0}),
        "fps_by_lint": defaultdict(list),
        "confirmed_by_lint": defaultdict(list),
    }
    
    for finding in db.get("findings", {}).values():
        stats["total"] += 1
        status = finding.get("status", "needs_review")
        lint = finding.get("lint", "unknown")
        repo = finding.get("repo", "unknown")
        category = finding.get("category", "unknown")
        
        # Count by status
        stats["by_status"][status] += 1
        
        # Count by lint
        stats["by_lint"][lint]["total"] += 1
        if status == "confirmed":
            stats["by_lint"][lint]["confirmed"] += 1
            stats["confirmed_by_lint"][lint].append(finding)
        elif status == "false_positive":
            stats["by_lint"][lint]["fp"] += 1
            stats["fps_by_lint"][lint].append(finding)
        elif status == "wont_fix":
            stats["by_lint"][lint]["wont_fix"] += 1
        else:
            stats["by_lint"][lint]["needs_review"] += 1
        
        # Count by repo
        stats["by_repo"][repo]["total"] += 1
        if status == "confirmed":
            stats["by_repo"][repo]["confirmed"] += 1
        elif status == "false_positive":
            stats["by_repo"][repo]["fp"] += 1
        
        # Count by category
        stats["by_category"][category]["total"] += 1
        if status == "confirmed":
            stats["by_category"][category]["confirmed"] += 1
        elif status == "false_positive":
            stats["by_category"][category]["fp"] += 1
    
    return stats


def extract_patterns(findings: list) -> dict:
    """Extract common patterns from finding notes."""
    patterns = defaultdict(list)
    
    for f in findings:
        note = f.get("notes", "").strip()
        if not note:
            note = "No pattern noted"
        
        # Normalize common variations
        note_lower = note.lower()
        if "dto" in note_lower or "data transfer" in note_lower:
            note = "Data Transfer Object (DTO)"
        elif "return" in note_lower and ("type" in note_lower or "value" in note_lower):
            note = "Return value struct"
        elif "test" in note_lower:
            note = "Test-only code"
        elif "intentional" in note_lower:
            note = "Intentional design choice"
        
        patterns[note].append(f)
    
    return dict(patterns)


def calculate_fp_rate(lint_stats: dict) -> float:
    """Calculate FP rate for a lint."""
    reviewed = lint_stats["confirmed"] + lint_stats["fp"] + lint_stats["wont_fix"]
    if reviewed == 0:
        return 0.0
    return (lint_stats["fp"] / reviewed) * 100


def generate_fp_report(stats: dict) -> str:
    """Generate FP analysis markdown report."""
    lines = [
        "# False Positive Pattern Analysis",
        "",
        f"**Generated:** {datetime.utcnow().strftime('%Y-%m-%d %H:%M UTC')}",
        "",
        "---",
        "",
        "## Overview",
        "",
        f"- **Total Findings:** {stats['total']}",
        f"- **Confirmed Issues:** {stats['by_status'].get('confirmed', 0)}",
        f"- **False Positives:** {stats['by_status'].get('false_positive', 0)}",
        f"- **Needs Review:** {stats['by_status'].get('needs_review', 0)}",
        "",
        "---",
        "",
        "## FP Rate by Lint",
        "",
        "| Lint | Total | Reviewed | FP | FP Rate | Status |",
        "|------|-------|----------|----|---------|---------| ",
    ]
    
    # Sort lints by FP count (descending)
    sorted_lints = sorted(
        stats["by_lint"].items(),
        key=lambda x: x[1]["fp"],
        reverse=True
    )
    
    for lint, data in sorted_lints:
        reviewed = data["confirmed"] + data["fp"] + data["wont_fix"]
        fp_rate = calculate_fp_rate(data)
        
        # Determine status emoji
        if reviewed == 0:
            status = "⏳ Unreviewed"
        elif fp_rate == 0:
            status = "✅ Clean"
        elif fp_rate < 10:
            status = "✅ Low FP"
        elif fp_rate < 25:
            status = "⚠️ Medium FP"
        else:
            status = "❌ High FP"
        
        lines.append(
            f"| {lint} | {data['total']} | {reviewed} | {data['fp']} | {fp_rate:.1f}% | {status} |"
        )
    
    lines.extend([
        "",
        "---",
        "",
        "## FP Patterns by Lint",
        "",
    ])
    
    # Detail FP patterns for each lint with FPs
    for lint, findings in sorted(stats["fps_by_lint"].items(), key=lambda x: -len(x[1])):
        if not findings:
            continue
        
        lint_data = stats["by_lint"][lint]
        fp_rate = calculate_fp_rate(lint_data)
        
        lines.extend([
            f"### {lint}",
            "",
            f"**Total FPs:** {len(findings)} | **FP Rate:** {fp_rate:.1f}%",
            "",
        ])
        
        patterns = extract_patterns(findings)
        
        for pattern, items in sorted(patterns.items(), key=lambda x: -len(x[1])):
            lines.extend([
                f"#### Pattern: {pattern}",
                f"**Count:** {len(items)}",
                "",
                "**Examples:**",
            ])
            
            for item in items[:3]:  # Show up to 3 examples
                lines.append(f"- `{item['repo']}/{item['file']}:{item['line']}`")
            
            if len(items) > 3:
                lines.append(f"- ... and {len(items) - 3} more")
            
            lines.append("")
        
        # Add recommendation
        lines.extend([
            "**Recommendation:**",
            "",
            _generate_recommendation(lint, patterns),
            "",
            "---",
            "",
        ])
    
    return "\n".join(lines)


def _generate_recommendation(lint: str, patterns: dict) -> str:
    """Generate a recommendation based on FP patterns."""
    pattern_names = list(patterns.keys())
    
    if not pattern_names or pattern_names == ["No pattern noted"]:
        return "> Document FP patterns with notes to generate recommendations."
    
    recommendations = []
    
    for pattern in pattern_names:
        pattern_lower = pattern.lower()
        
        if "dto" in pattern_lower or "data transfer" in pattern_lower:
            recommendations.append(
                f"> Consider excluding structs with names ending in `Info`, `Data`, `Config`, `Params`"
            )
        elif "return" in pattern_lower:
            recommendations.append(
                f"> Consider checking if struct is used as a function return type"
            )
        elif "test" in pattern_lower:
            recommendations.append(
                f"> Consider excluding `#[test_only]` modules and test files"
            )
        elif "intentional" in pattern_lower:
            recommendations.append(
                f"> Document intentional exceptions in lint description"
            )
    
    if not recommendations:
        recommendations.append(
            f"> Review patterns and determine lint refinement strategy"
        )
    
    return "\n".join(set(recommendations))


def generate_stats_report(stats: dict) -> str:
    """Generate a statistics summary report."""
    lines = [
        "# Triage Statistics",
        "",
        f"**Generated:** {datetime.utcnow().strftime('%Y-%m-%d %H:%M UTC')}",
        "",
        "## By Status",
        "",
        "| Status | Count | % |",
        "|--------|-------|---|",
    ]
    
    for status in ["confirmed", "false_positive", "wont_fix", "needs_review"]:
        count = stats["by_status"].get(status, 0)
        pct = (count / stats["total"] * 100) if stats["total"] > 0 else 0
        lines.append(f"| {status} | {count} | {pct:.1f}% |")
    
    lines.extend([
        "",
        "## By Category",
        "",
        "| Category | Total | Confirmed | FP |",
        "|----------|-------|-----------|----| ",
    ])
    
    for cat, data in sorted(stats["by_category"].items()):
        lines.append(f"| {cat} | {data['total']} | {data['confirmed']} | {data['fp']} |")
    
    lines.extend([
        "",
        "## By Repository",
        "",
        "| Repository | Total | Confirmed | FP |",
        "|------------|-------|-----------|----| ",
    ])
    
    for repo, data in sorted(stats["by_repo"].items(), key=lambda x: -x[1]["total"]):
        lines.append(f"| {repo} | {data['total']} | {data['confirmed']} | {data['fp']} |")
    
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(
        description="Analyze FP patterns in triage database"
    )
    parser.add_argument(
        "database",
        nargs="?",
        default="triage.json",
        help="Path to triage database (default: triage.json)"
    )
    parser.add_argument(
        "--output", "-o",
        help="Output file (default: stdout)"
    )
    parser.add_argument(
        "--format", "-f",
        choices=["fp", "stats", "both"],
        default="fp",
        help="Report format: fp (FP patterns), stats (statistics), both"
    )
    
    args = parser.parse_args()
    
    # Load database
    try:
        db = load_triage(args.database)
    except FileNotFoundError:
        print(f"ERROR: Database not found: {args.database}", file=sys.stderr)
        sys.exit(1)
    except json.JSONDecodeError as e:
        print(f"ERROR: Invalid JSON: {e}", file=sys.stderr)
        sys.exit(1)
    
    # Analyze findings
    stats = analyze_findings(db)
    
    # Generate report
    if args.format == "fp":
        report = generate_fp_report(stats)
    elif args.format == "stats":
        report = generate_stats_report(stats)
    else:
        report = generate_stats_report(stats) + "\n\n---\n\n" + generate_fp_report(stats)
    
    # Output
    if args.output:
        Path(args.output).write_text(report)
        print(f"Report written to: {args.output}")
    else:
        print(report)


if __name__ == "__main__":
    main()
