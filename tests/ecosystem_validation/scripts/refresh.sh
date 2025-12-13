#!/bin/bash
# =============================================================================
# refresh.sh - Full refresh: lint all repos → import → generate report
# =============================================================================
#
# Usage: ./scripts/refresh.sh [--preview]
#
# Options:
#   --preview    Include preview lints
#
# This is the main entry point for updating the triage database.
# Run this whenever:
# - Ecosystem repos are updated
# - New lints are added
# - You want a fresh baseline
# =============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_DIR="$(dirname "$SCRIPT_DIR")"

echo "╔══════════════════════════════════════════════╗"
echo "║   Move Clippy - Full Triage Refresh          ║"
echo "╚══════════════════════════════════════════════╝"
echo ""
echo "Date: $(date '+%Y-%m-%d %H:%M:%S')"
echo ""

# Pass through any arguments (like --preview)
ARGS="$@"

# Step 1: Lint all repos
echo "┌──────────────────────────────────────────────┐"
echo "│ Step 1: Linting all ecosystem repos          │"
echo "└──────────────────────────────────────────────┘"
echo ""
"$SCRIPT_DIR/lint_all.sh" $ARGS
echo ""

# Step 2: Import findings
echo "┌──────────────────────────────────────────────┐"
echo "│ Step 2: Importing findings to triage DB      │"
echo "└──────────────────────────────────────────────┘"
echo ""
"$SCRIPT_DIR/import_all.sh"
echo ""

# Step 3: Generate stats
echo "┌──────────────────────────────────────────────┐"
echo "│ Step 3: Generating Statistics                │"
echo "└──────────────────────────────────────────────┘"
echo ""

CLIPPY="$BASE_DIR/../../target/release/move-clippy"
DB="$BASE_DIR/triage.json"

$CLIPPY triage summary --database "$DB"

echo ""
echo "╔══════════════════════════════════════════════╗"
echo "║   Refresh Complete!                          ║"
echo "╚══════════════════════════════════════════════╝"
echo ""
echo "Next steps:"
echo "  1. Review security findings:"
echo "     move-clippy triage list --category security --status needs_review"
echo ""
echo "  2. Update finding status:"
echo "     move-clippy triage update <id> --status confirmed --notes '...'"
echo ""
echo "  3. Generate fresh report:"
echo "     move-clippy triage report --format md -o TRIAGE_REPORT.md"
echo ""
