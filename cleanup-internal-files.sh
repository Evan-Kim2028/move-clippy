#!/bin/bash
# Remove internal documentation and research files from git tracking
# These files should remain local-only and not be committed

set -e

echo "Removing internal documentation from git tracking..."

# Internal docs that should not be version controlled
git rm --cached -f \
  docs/DATAFLOW_ANALYSIS_GAPS.md \
  docs/KEY_STORE_PROXY_ANALYSIS.md \
  docs/LINT_GROUNDING_ANALYSIS.md \
  docs/LINT_INVENTORY.md \
  docs/LOCAL_ECOSYSTEM_VALIDATION.md \
  docs/PHASE_I_SUMMARY.md \
  docs/PHASE_II_III_COMPLETION_SUMMARY.md \
  docs/PHASE_II_III_IMPLEMENTATION.md \
  docs/IMPLEMENTATION_COMPLETE.md \
  docs/SEMANTIC_LINTER_EXPANSION_SPEC.md \
  2>/dev/null || true

echo "Removing ecosystem validation internal files from git tracking..."

# Remove entire ecosystem_validation directory except README
git rm --cached -rf tests/ecosystem_validation/ 2>/dev/null || true
# Add back the README
git add -f tests/ecosystem_validation/README.md 2>/dev/null || true

echo "Removing audit-analysis, notes, validation, scripts directories..."

git rm --cached -rf \
  audit-analysis/ \
  notes/ \
  validation/ \
  scripts/ \
  2>/dev/null || true

echo ""
echo "✓ Internal files removed from git tracking"
echo ""
echo "These files are now gitignored and will remain on your local disk."
echo "They won't be included in commits or pushed to the remote repository."
echo ""
echo "To complete the cleanup, commit the changes:"
echo "  git add .gitignore docs/README.md"
echo "  git commit -m 'chore: separate internal vs external documentation'"
