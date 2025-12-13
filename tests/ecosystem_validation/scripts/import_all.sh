#!/bin/bash
# =============================================================================
# import_all.sh - Import all JSON findings into triage database
# =============================================================================
#
# Usage: ./scripts/import_all.sh [--database <path>]
#
# Options:
#   --database   Path to triage database (default: ./triage.json)
#
# This script:
# 1. Imports findings from each repo's JSON file
# 2. Preserves existing triage status for known findings
# 3. Generates a fresh TRIAGE_REPORT.md
# =============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_DIR="$(dirname "$SCRIPT_DIR")"
CLIPPY="$BASE_DIR/../../target/release/move-clippy"
FINDINGS_DIR="$BASE_DIR/findings"
DB="$BASE_DIR/triage.json"

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --database)
            DB="$2"
            shift 2
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Check if clippy binary exists
if [[ ! -f "$CLIPPY" ]]; then
    echo "ERROR: move-clippy binary not found at $CLIPPY"
    echo "Run: cargo build --release"
    exit 1
fi

# Check if findings directory exists
if [[ ! -d "$FINDINGS_DIR" ]]; then
    echo "ERROR: Findings directory not found at $FINDINGS_DIR"
    echo "Run: ./scripts/lint_all.sh first"
    exit 1
fi

echo "=============================================="
echo "Move Clippy - Import Findings to Triage DB"
echo "=============================================="
echo "Date: $(date '+%Y-%m-%d %H:%M:%S')"
echo "Database: $DB"
echo ""

# Get initial count if DB exists
INITIAL_COUNT=0
if [[ -f "$DB" ]]; then
    INITIAL_COUNT=$(grep -c '"id"' "$DB" 2>/dev/null || echo "0")
    echo "Existing findings: $INITIAL_COUNT"
fi

IMPORTED=0
FILES_PROCESSED=0

for json in "$FINDINGS_DIR"/*.json; do
    # Skip manifest.json
    if [[ "$(basename "$json")" == "manifest.json" ]]; then
        continue
    fi
    
    repo=$(basename "$json" .json)
    
    # Skip empty files
    if [[ ! -s "$json" ]] || [[ "$(cat "$json")" == "[]" ]]; then
        echo "⚠️  SKIP: $repo (no findings)"
        continue
    fi
    
    echo -n "Importing $repo... "
    
    # Import findings
    RESULT=$($CLIPPY triage import "$json" --repo "$repo" --database "$DB" 2>&1)
    
    # Extract count from output (e.g., "Imported 123 findings...")
    COUNT=$(echo "$RESULT" | grep -o 'Imported [0-9]*' | grep -o '[0-9]*' || echo "0")
    echo "✓ $COUNT findings"
    
    IMPORTED=$((IMPORTED + COUNT))
    ((FILES_PROCESSED++))
done

echo ""
echo "=============================================="
echo "Generating Report"
echo "=============================================="

REPORT="$BASE_DIR/TRIAGE_REPORT.md"
$CLIPPY triage report --format md --database "$DB" -o "$REPORT"
echo "✓ Report: $REPORT"

# Get final count
FINAL_COUNT=$(grep -c '"id"' "$DB" 2>/dev/null || echo "0")
NEW_FINDINGS=$((FINAL_COUNT - INITIAL_COUNT))

echo ""
echo "=============================================="
echo "Summary"
echo "=============================================="
echo "Files processed: $FILES_PROCESSED"
echo "Total imported:  $IMPORTED"
echo "New findings:    $NEW_FINDINGS"
echo "Total in DB:     $FINAL_COUNT"
echo ""
echo "Database: $DB"
echo "Report:   $REPORT"
