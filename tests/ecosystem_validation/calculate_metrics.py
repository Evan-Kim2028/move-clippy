#!/usr/bin/env python3
"""
Calculate FP rates and other metrics from ecosystem validation triage data.

Usage:
    python calculate_metrics.py [triage.json]
    
If no triage file is provided, prompts user to create one from results.
"""

import json
import sys
from pathlib import Path
from collections import defaultdict
from datetime import datetime
from typing import Dict, List, Tuple

# ANSI colors
GREEN = '\033[0;32m'
YELLOW = '\033[1;33m'
RED = '\033[0;31m'
BLUE = '\033[0;34m'
NC = '\033[0m'  # No Color


def load_triage(triage_path: Path) -> dict:
    """Load and validate triage JSON file."""
    with open(triage_path, 'r') as f:
        data = json.load(f)
    
    # Basic validation
    assert 'metadata' in data, "Missing metadata"
    assert 'findings' in data, "Missing findings"
    
    return data


def calculate_metrics(triage_data: dict) -> Dict[str, dict]:
    """Calculate FP rates and other metrics per lint rule."""
    
    # Group findings by lint rule
    by_lint: Dict[str, List[dict]] = defaultdict(list)
    for finding in triage_data['findings']:
        by_lint[finding['lint']].append(finding)
    
    # Calculate metrics for each lint
    metrics = {}
    for lint_name, findings in by_lint.items():
        tp_count = sum(1 for f in findings if f['classification'] == 'TP')
        fp_count = sum(1 for f in findings if f['classification'] == 'FP')
        info_count = sum(1 for f in findings if f['classification'] == 'INFO')
        skip_count = sum(1 for f in findings if f['classification'] == 'SKIP')
        
        total_classified = tp_count + fp_count
        
        # Calculate FP rate (excluding INFO and SKIP)
        if total_classified > 0:
            fp_rate = (fp_count / total_classified) * 100
        else:
            fp_rate = 0.0
        
        # Calculate precision (TP / (TP + FP))
        precision = (tp_count / total_classified * 100) if total_classified > 0 else 0.0
        
        metrics[lint_name] = {
            'total': len(findings),
            'tp': tp_count,
            'fp': fp_count,
            'info': info_count,
            'skip': skip_count,
            'fp_rate': fp_rate,
            'precision': precision,
        }
    
    return metrics


def print_summary(metrics: Dict[str, dict], triage_data: dict):
    """Print a formatted summary of metrics."""
    
    print(f"\n{BLUE}{'='*80}{NC}")
    print(f"{BLUE}Move Clippy Ecosystem Validation Metrics{NC}")
    print(f"{BLUE}{'='*80}{NC}\n")
    
    # Metadata
    meta = triage_data['metadata']
    print(f"Generated: {meta.get('generated', 'N/A')}")
    print(f"Triager: {meta.get('triager', 'N/A')}")
    print(f"Version: {meta.get('move_clippy_version', 'N/A')}")
    if 'notes' in meta:
        print(f"Notes: {meta['notes']}")
    print()
    
    # Overall statistics
    total_findings = len(triage_data['findings'])
    total_tp = sum(m['tp'] for m in metrics.values())
    total_fp = sum(m['fp'] for m in metrics.values())
    total_info = sum(m['info'] for m in metrics.values())
    
    print(f"Total Findings: {total_findings}")
    print(f"  {GREEN}True Positives: {total_tp}{NC}")
    print(f"  {RED}False Positives: {total_fp}{NC}")
    print(f"  Informational: {total_info}")
    print()
    
    # Per-lint metrics
    print(f"\n{YELLOW}Per-Lint Metrics:{NC}\n")
    print(f"{'Lint':<35} {'Total':>6} {'TP':>5} {'FP':>5} {'FP%':>7} {'Precision':>9} {'Status':<10}")
    print(f"{'-'*35} {'-'*6} {'-'*5} {'-'*5} {'-'*7} {'-'*9} {'-'*10}")
    
    # Sort by FP rate
    sorted_lints = sorted(metrics.items(), key=lambda x: x[1]['fp_rate'])
    
    for lint_name, m in sorted_lints:
        # Determine status based on FP rate
        if m['fp_rate'] < 10:
            status = f"{GREEN}STABLE{NC}"
        elif m['fp_rate'] < 25:
            status = f"{YELLOW}PREVIEW{NC}"
        else:
            status = f"{RED}RESEARCH{NC}"
        
        print(f"{lint_name:<35} {m['total']:>6} {m['tp']:>5} {m['fp']:>5} "
              f"{m['fp_rate']:>6.1f}% {m['precision']:>8.1f}% {status}")
    
    print()
    
    # Recommendations
    print(f"\n{BLUE}Recommendations:{NC}\n")
    
    promotable = [name for name, m in metrics.items() if m['fp_rate'] < 10 and m['total'] >= 5]
    needs_refinement = [name for name, m in metrics.items() if 10 <= m['fp_rate'] < 30]
    needs_demotion = [name for name, m in metrics.items() if m['fp_rate'] >= 30]
    
    if promotable:
        print(f"{GREEN}✓ Promote to Stable:{NC}")
        for lint in promotable:
            print(f"  - {lint} (FP: {metrics[lint]['fp_rate']:.1f}%)")
    
    if needs_refinement:
        print(f"\n{YELLOW}⚠ Refine (stay in Preview):{NC}")
        for lint in needs_refinement:
            print(f"  - {lint} (FP: {metrics[lint]['fp_rate']:.1f}%)")
    
    if needs_demotion:
        print(f"\n{RED}✗ Demote to Research:{NC}")
        for lint in needs_demotion:
            print(f"  - {lint} (FP: {metrics[lint]['fp_rate']:.1f}%)")
    
    print()


def generate_report(metrics: Dict[str, dict], triage_data: dict, output_path: Path):
    """Generate a detailed Markdown report."""
    
    with open(output_path, 'w') as f:
        f.write("# Move Clippy Ecosystem Validation Report\n\n")
        
        # Metadata
        meta = triage_data['metadata']
        f.write(f"**Generated:** {meta.get('generated', 'N/A')}  \n")
        f.write(f"**Triager:** {meta.get('triager', 'N/A')}  \n")
        f.write(f"**Version:** {meta.get('move_clippy_version', 'N/A')}  \n\n")
        
        if 'notes' in meta:
            f.write(f"**Notes:** {meta['notes']}\n\n")
        
        f.write("---\n\n")
        
        # Executive Summary
        f.write("## Executive Summary\n\n")
        total_findings = len(triage_data['findings'])
        total_tp = sum(m['tp'] for m in metrics.values())
        total_fp = sum(m['fp'] for m in metrics.values())
        
        f.write(f"- **Total Findings:** {total_findings}\n")
        f.write(f"- **True Positives:** {total_tp}\n")
        f.write(f"- **False Positives:** {total_fp}\n")
        f.write(f"- **Overall Precision:** {(total_tp / (total_tp + total_fp) * 100):.1f}%\n\n")
        
        # Per-Lint Metrics Table
        f.write("## Per-Lint Metrics\n\n")
        f.write("| Lint | Total | TP | FP | FP Rate | Precision | Recommended Status |\n")
        f.write("|------|-------|----|----|---------|-----------|--------------------|\n")
        
        sorted_lints = sorted(metrics.items(), key=lambda x: x[1]['fp_rate'])
        for lint_name, m in sorted_lints:
            if m['fp_rate'] < 10:
                status = "✅ Stable"
            elif m['fp_rate'] < 25:
                status = "⚠️ Preview"
            else:
                status = "❌ Research"
            
            f.write(f"| `{lint_name}` | {m['total']} | {m['tp']} | {m['fp']} | "
                   f"{m['fp_rate']:.1f}% | {m['precision']:.1f}% | {status} |\n")
        
        f.write("\n")
        
        # Detailed Findings
        f.write("## True Positive Highlights\n\n")
        f.write("Notable bugs found by the linter:\n\n")
        
        # Group TPs by severity
        critical_tps = [f for f in triage_data['findings'] 
                       if f['classification'] == 'TP' and f.get('severity') == 'critical']
        high_tps = [f for f in triage_data['findings']
                   if f['classification'] == 'TP' and f.get('severity') == 'high']
        
        if critical_tps:
            f.write("### Critical Severity\n\n")
            for finding in critical_tps[:5]:  # Top 5
                f.write(f"- **{finding['lint']}** in `{finding['repo']}/{finding['file']}:{finding['line']}`\n")
                f.write(f"  - {finding['message']}\n")
                f.write(f"  - {finding['rationale']}\n\n")
        
        if high_tps:
            f.write("### High Severity\n\n")
            for finding in high_tps[:5]:  # Top 5
                f.write(f"- **{finding['lint']}** in `{finding['repo']}/{finding['file']}:{finding['line']}`\n")
                f.write(f"  - {finding['message']}\n")
                f.write(f"  - {finding['rationale']}\n\n")
        
        # False Positive Analysis
        f.write("## False Positive Analysis\n\n")
        
        # Group FPs by lint
        fp_by_lint = defaultdict(list)
        for finding in triage_data['findings']:
            if finding['classification'] == 'FP':
                fp_by_lint[finding['lint']].append(finding)
        
        if fp_by_lint:
            f.write("Common false positive patterns by lint:\n\n")
            for lint_name, fps in fp_by_lint.items():
                f.write(f"### `{lint_name}` ({len(fps)} FPs)\n\n")
                
                # Analyze common patterns
                rationales = [fp['rationale'] for fp in fps]
                f.write("Common causes:\n")
                for rationale in set(rationales)[:3]:  # Top 3 unique
                    count = rationales.count(rationale)
                    f.write(f"- ({count}x) {rationale}\n")
                f.write("\n")
        
        # Recommendations
        f.write("## Recommendations\n\n")
        
        promotable = [name for name, m in metrics.items() if m['fp_rate'] < 10 and m['total'] >= 5]
        needs_refinement = [name for name, m in metrics.items() if 10 <= m['fp_rate'] < 30]
        needs_demotion = [name for name, m in metrics.items() if m['fp_rate'] >= 30]
        
        if promotable:
            f.write("### Promote to Stable (FP < 10%)\n\n")
            for lint in promotable:
                f.write(f"- `{lint}` - FP rate: {metrics[lint]['fp_rate']:.1f}%\n")
            f.write("\n")
        
        if needs_refinement:
            f.write("### Refine (FP 10-30%, keep in Preview)\n\n")
            for lint in needs_refinement:
                f.write(f"- `{lint}` - FP rate: {metrics[lint]['fp_rate']:.1f}%\n")
            f.write("\n")
        
        if needs_demotion:
            f.write("### Demote to Research (FP > 30%)\n\n")
            for lint in needs_demotion:
                f.write(f"- `{lint}` - FP rate: {metrics[lint]['fp_rate']:.1f}%\n")
            f.write("\n")
    
    print(f"{GREEN}✓ Report generated: {output_path}{NC}")


def main():
    """Main entry point."""
    
    # Find triage file
    if len(sys.argv) > 1:
        triage_path = Path(sys.argv[1])
    else:
        # Look for triage.json in current directory
        triage_path = Path(__file__).parent / 'triage.json'
    
    if not triage_path.exists():
        print(f"{RED}Error: Triage file not found: {triage_path}{NC}")
        print(f"\nTo create a triage file:")
        print(f"  1. Copy triage_template.json to triage.json")
        print(f"  2. Manually classify findings from results/*.json")
        print(f"  3. Run this script again")
        sys.exit(1)
    
    print(f"{BLUE}Loading triage data from: {triage_path}{NC}")
    triage_data = load_triage(triage_path)
    
    print(f"{BLUE}Calculating metrics...{NC}")
    metrics = calculate_metrics(triage_data)
    
    # Print summary to console
    print_summary(metrics, triage_data)
    
    # Generate detailed report
    report_path = Path(__file__).parent / 'VALIDATION_REPORT.md'
    generate_report(metrics, triage_data, report_path)
    
    print(f"\n{GREEN}Done!{NC}")


if __name__ == '__main__':
    main()
