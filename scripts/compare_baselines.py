#!/usr/bin/env python3
"""
Compare two benchmark baselines to identify regressions and improvements.

Usage:
    python3 scripts/compare_baselines.py baseline_old.json baseline_new.json
    
Examples:
    python3 scripts/compare_baselines.py baselines/baseline_0.2.0.json baselines/baseline_0.3.0.json
"""

import argparse
import json
import sys
from pathlib import Path


def load_baseline(path: Path) -> dict:
    """Load a baseline JSON file."""
    with open(path) as f:
        return json.load(f)


def compare_baselines(old: dict, new: dict) -> None:
    """Compare two baselines and print differences."""
    print(f"=== Baseline Comparison ===")
    print(f"Old: {old['version']} ({old['timestamp'][:10]})")
    print(f"New: {new['version']} ({new['timestamp'][:10]})")
    print()
    
    # Overall stats
    print("=== Overall Stats ===")
    print(f"{'Metric':<30} {'Old':>10} {'New':>10} {'Delta':>10}")
    print("-" * 62)
    
    metrics = [
        ("Total repos", old['total_repos'], new['total_repos']),
        ("Repos with findings", old['repos_with_findings'], new['repos_with_findings']),
        ("Total findings", old['total_findings'], new['total_findings']),
    ]
    
    for name, old_val, new_val in metrics:
        delta = new_val - old_val
        delta_str = f"+{delta}" if delta > 0 else str(delta)
        print(f"{name:<30} {old_val:>10} {new_val:>10} {delta_str:>10}")
    
    print()
    
    # By tier comparison
    print("=== By Tier ===")
    print(f"{'Tier':<20} {'Old':>10} {'New':>10} {'Delta':>10}")
    print("-" * 52)
    
    all_tiers = set(old.get('by_tier', {}).keys()) | set(new.get('by_tier', {}).keys())
    for tier in sorted(all_tiers):
        old_val = old.get('by_tier', {}).get(tier, 0)
        new_val = new.get('by_tier', {}).get(tier, 0)
        delta = new_val - old_val
        delta_str = f"+{delta}" if delta > 0 else str(delta)
        print(f"{tier:<20} {old_val:>10} {new_val:>10} {delta_str:>10}")
    
    print()
    
    # Lint-level comparison
    print("=== Lint Changes (sorted by delta) ===")
    print(f"{'Lint':<40} {'Old':>8} {'New':>8} {'Delta':>8}")
    print("-" * 66)
    
    old_lints = old.get('by_lint', {})
    new_lints = new.get('by_lint', {})
    all_lints = set(old_lints.keys()) | set(new_lints.keys())
    
    changes = []
    for lint in all_lints:
        old_count = old_lints.get(lint, {}).get('count', 0)
        new_count = new_lints.get(lint, {}).get('count', 0)
        delta = new_count - old_count
        if delta != 0:
            changes.append((lint, old_count, new_count, delta))
    
    # Sort by absolute delta (biggest changes first)
    changes.sort(key=lambda x: -abs(x[3]))
    
    for lint, old_count, new_count, delta in changes[:20]:
        delta_str = f"+{delta}" if delta > 0 else str(delta)
        indicator = "🔴" if delta > 0 else "🟢"
        print(f"{indicator} {lint:<38} {old_count:>8} {new_count:>8} {delta_str:>8}")
    
    if len(changes) > 20:
        print(f"  ... and {len(changes) - 20} more lints with changes")
    
    print()
    
    # New lints
    new_only = set(new_lints.keys()) - set(old_lints.keys())
    if new_only:
        print("=== New Lints ===")
        for lint in sorted(new_only):
            count = new_lints[lint].get('count', 0)
            tier = new_lints[lint].get('tier', 'unknown')
            print(f"  + {lint} ({tier}): {count} findings")
        print()
    
    # Removed lints
    removed = set(old_lints.keys()) - set(new_lints.keys())
    if removed:
        print("=== Removed Lints ===")
        for lint in sorted(removed):
            count = old_lints[lint].get('count', 0)
            print(f"  - {lint}: was {count} findings")
        print()
    
    # Repo-level regressions
    print("=== Repo Changes (top 10 by delta) ===")
    old_repos = old.get('repos', {})
    new_repos = new.get('repos', {})
    
    repo_changes = []
    for repo in set(old_repos.keys()) | set(new_repos.keys()):
        old_count = old_repos.get(repo, {}).get('findings', 0)
        new_count = new_repos.get(repo, {}).get('findings', 0)
        delta = new_count - old_count
        if delta != 0:
            repo_changes.append((repo, old_count, new_count, delta))
    
    repo_changes.sort(key=lambda x: -abs(x[3]))
    
    for repo, old_count, new_count, delta in repo_changes[:10]:
        delta_str = f"+{delta}" if delta > 0 else str(delta)
        indicator = "🔴" if delta > 0 else "🟢"
        print(f"  {indicator} {repo}: {old_count} → {new_count} ({delta_str})")
    
    print()
    
    # Summary verdict
    total_delta = new['total_findings'] - old['total_findings']
    if total_delta > 0:
        print(f"⚠️  REGRESSION: {total_delta} more findings in {new['version']}")
    elif total_delta < 0:
        print(f"✅ IMPROVEMENT: {-total_delta} fewer findings in {new['version']}")
    else:
        print(f"➡️  NO CHANGE: Same number of findings")


def main():
    parser = argparse.ArgumentParser(description="Compare move-clippy baselines")
    parser.add_argument("old", type=Path, help="Old baseline JSON file")
    parser.add_argument("new", type=Path, help="New baseline JSON file")
    
    args = parser.parse_args()
    
    if not args.old.exists():
        print(f"Error: {args.old} not found", file=sys.stderr)
        sys.exit(1)
    
    if not args.new.exists():
        print(f"Error: {args.new} not found", file=sys.stderr)
        sys.exit(1)
    
    old = load_baseline(args.old)
    new = load_baseline(args.new)
    
    compare_baselines(old, new)


if __name__ == "__main__":
    main()
