# Move Clippy Developer Notes

Internal notes for contributors working on the Move Clippy workspace.

**Status:** Developer workflow (kept current)

## Workspace Overview

- The crate is a single-member Cargo workspace. Build/test profiles, shared dependencies, and feature flags (e.g. `full`, `telemetry`) are defined once in `Cargo.toml`.
- Release profiles enforce `panic = abort`, split debuginfo, and LTO so that release binaries match the Sui project defaults.
- Semantic functionality lives behind the `full` feature. Fast mode stays tree-sitter only.

## Error Handling & Results

- The crate contains both structured errors (`ClippyResult<T>` / `MoveClippyError` in `src/error.rs`) and `anyhow::Result`.
- Today, the fast-mode engine and many helpers use `anyhow::Result` for ergonomic context chains; the semantic/fixture subsystems often prefer `MoveClippyError` for structured errors.
- When adding new subsystems, prefer `MoveClippyError` where callers need stable error kinds (e.g. fixtures/semantic); otherwise use `anyhow` with good `.context(...)` messages.

## Telemetry & Instrumentation

- Tracing initializes automatically in `main` via `telemetry::init_tracing()`. The `telemetry` feature is enabled by default so spans are always available.
- Use the `instrument_block!` helper for lightweight span creation inside ad-hoc blocks (tests, migrations). Example: wrapping `run_fixture` in `tests/sui_lints.rs`.
- Set `RUST_LOG=move_clippy=info` locally to inspect spans around semantic linting, fixture modernization, and CLI commands.

## Tree-sitter Pattern Helpers

- `src/rules/patterns.rs` centralizes string-level helpers for modernization lints. Typical consumers are rules that only have textual node content, not AST structure.
- Helpers normalize nested parentheses, trailing semicolons, and colon-path identifiers. Add tests for every new helper branch and keep coverage high (see the `#[cfg(test)]` module in the same file).

## Semantic Fixtures & Tests

- Full mode tests rely on `tests/fixtures/semantic_pkg`, which includes a minimal `sui::object::UID` stub to keep compilation self-contained.
- Integration tests that generate temporary packages (spec-style matrices) share helpers under `tests/support/`.
- Always run:
  - `cargo test` (fast-mode + unit tests)
  - `cargo test --features full` (semantic mode; may require fetching git deps)
  - `cargo test --features full --test semantic_package_snapshots` (compiler-based snapshots)
  - `cargo test --test syntactic_snapshots` (tree-sitter snapshots)
- Regenerate the lint reference with `cargo run --features full --bin gen_lint_reference > docs/LINT_REFERENCE.md`.
- Regenerate the catalog summary with `cargo run --features full --bin gen_lint_catalog_summary > docs/LINT_CATALOG_SUMMARY.md`.
- When modifying fixtures, keep them intentionally “almost-correct” so lints trigger while still compiling under Sui rules (e.g. valid `key` structs with naming violations).

## Fixtures As Documentation

- `tests/fixtures/README.md` describes fixture layout (including phase directories), WIP fixture expectations, and the recommended “lint contract” (positive, negative, and directive coverage).

## Repository Hygiene Checklist

1. **Formatting:** `cargo fmt`
2. **Clippy:** (optional) `cargo clippy --all-features --all-targets`
3. **Tests:** commands above
4. **Docs:** Update this file when changing developer workflows (error handling, telemetry flags, fixture layouts, etc.)
