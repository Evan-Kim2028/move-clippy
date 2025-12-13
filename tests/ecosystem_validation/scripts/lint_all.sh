#!/bin/bash
# =============================================================================
# lint_all.sh - Lint all ecosystem repos and save JSON findings
# =============================================================================
#
# Usage: ./scripts/lint_all.sh [--preview]
#
# Options:
#   --preview    Include preview lints (higher FP rate but more coverage)
#
# Output: JSON files in ./findings/ directory
# =============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_DIR="$(dirname "$SCRIPT_DIR")"
CLIPPY="$BASE_DIR/../../target/release/move-clippy"
REPOS_DIR="$BASE_DIR/../../../ecosystem-test-repos"
OUTPUT_DIR="$BASE_DIR/findings"

# Parse arguments
PREVIEW_FLAG=""
if [[ "$1" == "--preview" ]]; then
    PREVIEW_FLAG="--preview"
    echo "Running with --preview (includes preview lints)"
fi

# Check if clippy binary exists
if [[ ! -f "$CLIPPY" ]]; then
    echo "ERROR: move-clippy binary not found at $CLIPPY"
    echo "Run: cargo build --release"
    exit 1
fi

# Check if repos directory exists
if [[ ! -d "$REPOS_DIR" ]]; then
    echo "ERROR: Ecosystem repos not found at $REPOS_DIR"
    exit 1
fi

mkdir -p "$OUTPUT_DIR"

# List of repos to lint
REPOS=(
    "alphalend"
    "bluefin-integer"
    "bluefin-pro"
    "bluefin-spot"
    "cetus-clmm"
    "deepbookv3"
    "openzeppelin-sui"
    "scallop-lend"
    "steamm"
    "suilend"
    "suilend-liquid-staking"
)

echo "=============================================="
echo "Move Clippy - Ecosystem Lint Run"
echo "=============================================="
echo "Date: $(date '+%Y-%m-%d %H:%M:%S')"
echo "Repos: ${#REPOS[@]}"
echo "Output: $OUTPUT_DIR"
echo ""

TOTAL_FINDINGS=0
SUCCESSFUL=0
FAILED=0

for repo in "${REPOS[@]}"; do
    REPO_PATH="$REPOS_DIR/$repo"
    OUTPUT_FILE="$OUTPUT_DIR/$repo.json"
    
    if [[ ! -d "$REPO_PATH" ]]; then
        echo "⚠️  SKIP: $repo (directory not found)"
        ((FAILED++))
        continue
    fi
    
    echo -n "Linting $repo... "
    
    # Run linter and capture output
    if $CLIPPY --format json $PREVIEW_FLAG "$REPO_PATH" 2>/dev/null > "$OUTPUT_FILE"; then
        # Count findings (each finding has a "lint" field)
        COUNT=$(grep -c '"lint"' "$OUTPUT_FILE" 2>/dev/null || echo "0")
        echo "✓ $COUNT findings"
        TOTAL_FINDINGS=$((TOTAL_FINDINGS + COUNT))
        ((SUCCESSFUL++))
    else
        echo "✗ ERROR"
        ((FAILED++))
        # Create empty JSON array on error
        echo "[]" > "$OUTPUT_FILE"
    fi
done

echo ""
echo "=============================================="
echo "Summary"
echo "=============================================="
echo "Successful: $SUCCESSFUL/${#REPOS[@]}"
echo "Failed:     $FAILED"
echo "Total:      $TOTAL_FINDINGS findings"
echo ""
echo "Findings saved to: $OUTPUT_DIR/"
echo ""

# Create a manifest file with run metadata
MANIFEST="$OUTPUT_DIR/manifest.json"
cat > "$MANIFEST" << EOF
{
  "run_date": "$(date -u '+%Y-%m-%dT%H:%M:%SZ')",
  "clippy_version": "$($CLIPPY --version 2>/dev/null || echo 'unknown')",
  "preview_mode": $([ -n "$PREVIEW_FLAG" ] && echo "true" || echo "false"),
  "repos_count": ${#REPOS[@]},
  "successful": $SUCCESSFUL,
  "failed": $FAILED,
  "total_findings": $TOTAL_FINDINGS,
  "repos": [
$(for i in "${!REPOS[@]}"; do
    echo "    \"${REPOS[$i]}\"$([ $i -lt $((${#REPOS[@]}-1)) ] && echo ",")"
done)
  ]
}
EOF

echo "Manifest: $MANIFEST"
