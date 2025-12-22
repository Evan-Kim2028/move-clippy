#!/usr/bin/env python3
"""
Ecosystem Benchmark Script for move-clippy

Runs move-clippy against a curated set of benchmark repositories
and outputs results in JSON format for comparison across versions.

Usage:
    python3 scripts/ecosystem_benchmark.py [--repos-dir PATH] [--output FILE] [--parallel N]
    
Examples:
    # Run benchmark with defaults
    python3 scripts/ecosystem_benchmark.py
    
    # Custom repos directory
    python3 scripts/ecosystem_benchmark.py --repos-dir /path/to/repos
    
    # Save to specific file
    python3 scripts/ecosystem_benchmark.py --output baseline_v0.3.0.json
"""

import argparse
import json
import os
import subprocess
import sys
import time
from collections import defaultdict
from concurrent.futures import ProcessPoolExecutor, as_completed
from dataclasses import dataclass, asdict
from datetime import datetime
from pathlib import Path
from typing import Optional


@dataclass
class LintResult:
    name: str
    count: int
    repos: list[str]
    tier: str = ""


@dataclass 
class RepoResult:
    name: str
    total_findings: int
    move_files: int
    lint_breakdown: dict[str, int]
    duration_seconds: float
    error: Optional[str] = None


@dataclass
class BenchmarkResult:
    version: str
    timestamp: str
    duration_seconds: float
    total_repos: int
    repos_with_findings: int
    total_findings: int
    by_lint: dict[str, dict]
    by_tier: dict[str, int]
    by_category: dict[str, int]
    repos: dict[str, dict]


def get_version() -> str:
    """Get move-clippy version from binary."""
    try:
        result = subprocess.run(
            ["./target/release/move-clippy", "--version"],
            capture_output=True, text=True, timeout=10
        )
        return result.stdout.strip().replace("move-clippy ", "")
    except Exception:
        return "unknown"


def get_lint_metadata() -> dict[str, dict]:
    """Get lint tiers and categories from list-rules."""
    metadata = {}
    try:
        result = subprocess.run(
            ["./target/release/move-clippy", "list-rules"],
            capture_output=True, text=True, timeout=30
        )
        for line in result.stdout.strip().split('\n'):
            parts = line.split('\t')
            if len(parts) >= 3:
                name, category, tier = parts[0], parts[1], parts[2]
                metadata[name] = {"category": category, "tier": tier}
    except Exception as e:
        print(f"Warning: Could not get lint metadata: {e}", file=sys.stderr)
    return metadata


def count_move_files(repo_path: str) -> int:
    """Count .move files in repo (excluding dependencies)."""
    count = 0
    for root, dirs, files in os.walk(repo_path):
        # Skip dependency directories
        dirs[:] = [d for d in dirs if d not in ('dependencies', 'build', '.git', 'node_modules')]
        count += sum(1 for f in files if f.endswith('.move'))
    return count


def run_lint_on_repo(args: tuple[str, str, bool]) -> RepoResult:
    """Run move-clippy on a single repo."""
    repo_name, repo_path, experimental = args
    start = time.time()
    
    if not os.path.isdir(repo_path):
        return RepoResult(
            name=repo_name,
            total_findings=0,
            move_files=0,
            lint_breakdown={},
            duration_seconds=0,
            error=f"Directory not found: {repo_path}"
        )
    
    move_files = count_move_files(repo_path)
    if move_files == 0:
        return RepoResult(
            name=repo_name,
            total_findings=0,
            move_files=0,
            lint_breakdown={},
            duration_seconds=time.time() - start,
            error="No .move files found"
        )
    
    try:
        cmd = ["./target/release/move-clippy"]
        if experimental:
            cmd.append("--experimental")
        cmd.append(repo_path)
        
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=180  # 3 minute timeout per repo
        )
        
        output = result.stdout + result.stderr
        lint_counts = defaultdict(int)
        total = 0
        
        for line in output.split('\n'):
            if 'warning:' in line:
                total += 1
                # Extract lint name from "warning: lint_name: message"
                try:
                    after_warning = line.split('warning:')[1].strip()
                    lint_name = after_warning.split(':')[0].strip()
                    lint_counts[lint_name] += 1
                except (IndexError, ValueError):
                    lint_counts["unknown"] += 1
        
        return RepoResult(
            name=repo_name,
            total_findings=total,
            move_files=move_files,
            lint_breakdown=dict(lint_counts),
            duration_seconds=time.time() - start
        )
        
    except subprocess.TimeoutExpired:
        return RepoResult(
            name=repo_name,
            total_findings=0,
            move_files=move_files,
            lint_breakdown={},
            duration_seconds=time.time() - start,
            error="Timeout after 180s"
        )
    except Exception as e:
        return RepoResult(
            name=repo_name,
            total_findings=0,
            move_files=move_files,
            lint_breakdown={},
            duration_seconds=time.time() - start,
            error=str(e)
        )


def load_repo_list(benchmark_dir: Path) -> list[str]:
    """Load curated repo list from REPOS.txt."""
    repos_file = benchmark_dir / "REPOS.txt"
    if not repos_file.exists():
        print(f"Error: {repos_file} not found", file=sys.stderr)
        sys.exit(1)
    
    repos = []
    with open(repos_file) as f:
        for line in f:
            line = line.strip()
            # Skip comments and empty lines
            if line and not line.startswith('#'):
                repos.append(line)
    return repos


def run_benchmark(
    repos_dir: Path,
    benchmark_dir: Path,
    parallel: int = 4,
    experimental: bool = True
) -> BenchmarkResult:
    """Run the full benchmark suite."""
    start_time = time.time()
    
    # Load repo list
    repos = load_repo_list(benchmark_dir)
    print(f"Loaded {len(repos)} repos from benchmark list")
    
    # Get lint metadata
    lint_metadata = get_lint_metadata()
    
    # Prepare work items
    work_items = [
        (repo, str(repos_dir / repo), experimental)
        for repo in repos
    ]
    
    # Run in parallel
    results: list[RepoResult] = []
    print(f"Running with {parallel} parallel workers...")
    
    with ProcessPoolExecutor(max_workers=parallel) as executor:
        futures = {executor.submit(run_lint_on_repo, item): item[0] for item in work_items}
        
        for i, future in enumerate(as_completed(futures), 1):
            repo_name = futures[future]
            try:
                result = future.result()
                results.append(result)
                status = f"{result.total_findings} findings" if not result.error else f"ERROR: {result.error}"
                print(f"  [{i}/{len(repos)}] {repo_name}: {status}")
            except Exception as e:
                print(f"  [{i}/{len(repos)}] {repo_name}: FAILED - {e}")
                results.append(RepoResult(
                    name=repo_name,
                    total_findings=0,
                    move_files=0,
                    lint_breakdown={},
                    duration_seconds=0,
                    error=str(e)
                ))
    
    # Aggregate results
    total_findings = sum(r.total_findings for r in results)
    repos_with_findings = sum(1 for r in results if r.total_findings > 0)
    
    # Aggregate by lint
    by_lint: dict[str, dict] = defaultdict(lambda: {"count": 0, "repos": []})
    for result in results:
        for lint_name, count in result.lint_breakdown.items():
            by_lint[lint_name]["count"] += count
            by_lint[lint_name]["repos"].append(result.name)
    
    # Add tier info
    for lint_name in by_lint:
        if lint_name in lint_metadata:
            by_lint[lint_name]["tier"] = lint_metadata[lint_name]["tier"]
            by_lint[lint_name]["category"] = lint_metadata[lint_name]["category"]
    
    # Aggregate by tier
    by_tier: dict[str, int] = defaultdict(int)
    by_category: dict[str, int] = defaultdict(int)
    for lint_name, data in by_lint.items():
        tier = data.get("tier", "unknown")
        category = data.get("category", "unknown")
        by_tier[tier] += data["count"]
        by_category[category] += data["count"]
    
    # Build repos dict
    repos_dict = {
        r.name: {
            "findings": r.total_findings,
            "move_files": r.move_files,
            "duration_seconds": round(r.duration_seconds, 2),
            "error": r.error
        }
        for r in results
    }
    
    return BenchmarkResult(
        version=get_version(),
        timestamp=datetime.utcnow().isoformat() + "Z",
        duration_seconds=round(time.time() - start_time, 2),
        total_repos=len(repos),
        repos_with_findings=repos_with_findings,
        total_findings=total_findings,
        by_lint=dict(by_lint),
        by_tier=dict(by_tier),
        by_category=dict(by_category),
        repos=repos_dict
    )


def main():
    parser = argparse.ArgumentParser(description="Run move-clippy ecosystem benchmark")
    parser.add_argument(
        "--repos-dir",
        type=Path,
        default=Path.home() / "Documents/learning_move/packages/ecosystem-test-repos/repos",
        help="Directory containing ecosystem repos"
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=None,
        help="Output JSON file (default: baselines/baseline_<version>.json)"
    )
    parser.add_argument(
        "--parallel",
        type=int,
        default=4,
        help="Number of parallel workers (default: 4)"
    )
    parser.add_argument(
        "--no-experimental",
        action="store_true",
        help="Disable experimental lints"
    )
    
    args = parser.parse_args()
    
    # Ensure we're in the right directory
    if not Path("./target/release/move-clippy").exists():
        print("Error: Run from move-clippy root directory with release build", file=sys.stderr)
        print("  cd /path/to/move-clippy && cargo build --release", file=sys.stderr)
        sys.exit(1)
    
    benchmark_dir = Path("benchmark_repos")
    if not benchmark_dir.exists():
        print(f"Error: {benchmark_dir} not found", file=sys.stderr)
        sys.exit(1)
    
    print(f"=== Move-Clippy Ecosystem Benchmark ===")
    print(f"Repos directory: {args.repos_dir}")
    print(f"Parallel workers: {args.parallel}")
    print(f"Experimental lints: {not args.no_experimental}")
    print()
    
    result = run_benchmark(
        repos_dir=args.repos_dir,
        benchmark_dir=benchmark_dir,
        parallel=args.parallel,
        experimental=not args.no_experimental
    )
    
    # Determine output path
    baselines_dir = Path("baselines")
    baselines_dir.mkdir(exist_ok=True)
    
    if args.output:
        output_path = args.output
    else:
        output_path = baselines_dir / f"baseline_{result.version}.json"
    
    # Write results
    with open(output_path, 'w') as f:
        json.dump(asdict(result), f, indent=2)
    
    print()
    print(f"=== Summary ===")
    print(f"Version: {result.version}")
    print(f"Duration: {result.duration_seconds}s")
    print(f"Repos: {result.repos_with_findings}/{result.total_repos} with findings")
    print(f"Total findings: {result.total_findings}")
    print()
    print(f"By tier:")
    for tier, count in sorted(result.by_tier.items(), key=lambda x: -x[1]):
        print(f"  {tier}: {count}")
    print()
    print(f"Top 10 lints:")
    sorted_lints = sorted(result.by_lint.items(), key=lambda x: -x[1]["count"])[:10]
    for lint, data in sorted_lints:
        print(f"  {lint}: {data['count']} ({len(data['repos'])} repos)")
    print()
    print(f"Results saved to: {output_path}")


if __name__ == "__main__":
    main()
