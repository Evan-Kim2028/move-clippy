# Move Clippy Documentation

This directory contains user-facing documentation for move-clippy development and usage.

## User-Facing Documentation

### For Contributors

- [`DEVELOPMENT.md`](DEVELOPMENT.md): Local development workflow, error handling, telemetry, and testing
- [`LINT_DEVELOPMENT_GUIDE.md`](LINT_DEVELOPMENT_GUIDE.md): How to develop new lints with examples
- [`FP_PREVENTION.md`](FP_PREVENTION.md): False positive prevention methodology and testing
- [`STABILITY.md`](STABILITY.md): Rule stability policy and promotion criteria

### For Users

- [`SECURITY_LINTS.md`](SECURITY_LINTS.md): Security lint reference with audit sources and detection methods
- [`SEMANTIC_LINT_STATUS.md`](SEMANTIC_LINT_STATUS.md): Status of semantic lints (requires `--mode full`)

---

## Internal Documentation (Gitignored)

The following docs are for internal development/research and **not committed to version control**:

- `DATAFLOW_ANALYSIS_GAPS.md` - Analysis of dataflow limitations
- `KEY_STORE_PROXY_ANALYSIS.md` - Deep dive into key+store type detection
- `LINT_GROUNDING_ANALYSIS.md` - Lint audit grounding research
- `LINT_INVENTORY.md` - Internal lint catalog and status tracking
- `LOCAL_ECOSYSTEM_VALIDATION.md` - Local validation workflow notes
- `PHASE_*_SUMMARY.md` - Implementation phase summaries
- `IMPLEMENTATION_COMPLETE.md` - Completion checklist
- `SEMANTIC_LINTER_EXPANSION_SPEC.md` - Internal spec document

These are kept locally for reference but excluded from the repository to keep it focused on user-facing documentation.
