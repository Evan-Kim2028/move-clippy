# Move Clippy Developer Notes

Internal notes for contributors working on the Move Clippy workspace.

## Workspace Overview

- The crate is a single-member Cargo workspace. Build/test profiles, shared dependencies, and feature flags (e.g. `full`, `telemetry`) are defined once in `Cargo.toml`.
- Release profiles enforce `panic = abort`, split debuginfo, and LTO so that release binaries match the Sui project defaults.
- Semantic functionality lives behind the `full` feature. Fast mode stays tree-sitter only.

## Error Handling & Results

- Library APIs return `ClippyResult<T>` (alias for `Result<T, MoveClippyError>`). Avoid exposing `anyhow::Result` from crate internals so errors remain structured.
- `MoveClippyError` provides helpers: `MoveClippyError::semantic`, `MoveClippyError::fixture`, and `clippy_bail!`. Prefer these over `anyhow::bail!`.
- When interacting with external crates that still return `anyhow::Error`, convert at the boundary by calling `.into_anyhow()` (see `semantic::lint_sui_visitors`).

## Telemetry & Instrumentation

- Tracing initializes automatically in `main` via `telemetry::init_tracing()`. The `telemetry` feature is enabled by default so spans are always available.
- Use the `instrument_block!` helper for lightweight span creation inside ad-hoc blocks (tests, migrations). Example: wrapping `run_fixture` in `tests/sui_lints.rs`.
- Set `RUST_LOG=move_clippy=info` locally to inspect spans around semantic linting, fixture modernization, and CLI commands.

## Tree-sitter Pattern Helpers

- `src/rules/patterns.rs` centralizes string-level helpers for modernization lints. Typical consumers are rules that only have textual node content, not AST structure.
- Helpers normalize nested parentheses, trailing semicolons, and colon-path identifiers. Add tests for every new helper branch and keep coverage high (see the `#[cfg(test)]` module in the same file).

## Semantic Fixtures & Tests

- Full mode tests rely on `tests/fixtures/semantic_pkg`, which now includes a minimal `sui::object::UID` stub to keep compilation self-contained.
- Always run:
  - `cargo test --all-features` (covers fast + semantic + fixtures)
  - `cargo test --features full --test sui_lints` (quick semantic regression loop)
- When modifying fixtures, keep them intentionally “almost-correct” so lints trigger while still compiling under Sui rules (e.g. valid `key` structs with naming violations).

## Repository Hygiene Checklist

1. **Formatting:** `cargo fmt`
2. **Clippy:** (optional) `cargo clippy --all-features --all-targets`
3. **Tests:** commands above
4. **Docs:** Update this file when changing developer workflows (error handling, telemetry flags, fixture layouts, etc.)
