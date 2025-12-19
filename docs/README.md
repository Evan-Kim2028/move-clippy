# Move Clippy Documentation

This directory contains user-facing documentation for move-clippy development and usage.

**Status:** Index (kept current)

**Documentation taxonomy (code-as-docs):**
- **Generated reference**: derived from the code (authoritative, do not edit).
- **Developer workflow**: how to build/test/extend the tool (kept current).
- **Design notes**: research, drafts, and historical notes (may drift).

## Documentation Map

### For Contributors

- [`DEVELOPMENT.md`](DEVELOPMENT.md): Local development workflow, error handling, telemetry, and testing
- [`LINT_DEVELOPMENT_GUIDE.md`](LINT_DEVELOPMENT_GUIDE.md): How to develop new lints with examples
- [`FP_PREVENTION.md`](FP_PREVENTION.md): False positive prevention methodology and testing
- [`STABILITY.md`](STABILITY.md): Rule stability policy and promotion criteria

### For Users

- [`SECURITY_LINTS.md`](SECURITY_LINTS.md): Security lint reference with audit sources and detection methods
- [`SEMANTIC_LINT_STATUS.md`](SEMANTIC_LINT_STATUS.md): Status of semantic lints (requires `--mode full`)

### Generated Reference (authoritative)

- `docs/LINT_REFERENCE.md` (generated per-lint catalog; see header for regen command)
- `docs/LINT_CATALOG_SUMMARY.md` (generated counts by tier/phase/category; see header for regen command)

---

## Research & Design Notes

This repo keeps a mix of user-facing docs and deeper research/design notes in the same `docs/` directory (e.g. analysis gaps, phase summaries, specs). These are useful when developing new lints and maintaining FP guarantees.

If you’re looking for the “single entry point”:

- `docs/LINT_REFERENCE.md` (generated per-lint reference; see header for regen command)
- `docs/STABILITY.md` (tier policy)
- `docs/FP_PREVENTION.md` (how we avoid false positives)

Long-form notes and saved writeups live under `docs/notes/`.
