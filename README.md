# Move Clippy

Move linter inspired by Rust Clippy, focused on Move 2024 style and Sui conventions.

## What it does

- Lints Move source files or packages for style, modernization, and test quality issues (e.g. `constant_naming`, `unneeded_return`, `unnecessary_public_entry`, `public_mut_tx_context`, `while_true_to_loop`).
- Supports `move-clippy.toml` (per-lint `allow`/`warn`/`error` levels and a `disabled` list).
- Respects `#[allow(lint::name)]` attributes on modules, structs, functions, constants, and use items.
- Optional `--mode full` semantic analysis (behind the `full` feature) for capability/event/getter naming.

## Usage

```bash
# Fast syntax-only mode
cargo run -- lint path/to/sources

# Full semantic mode (requires building with --features full)
cargo run --features full -- lint --mode full --package path/to/Move/package
```

## Auto-Fix Support

Move Clippy can automatically fix certain lint violations:

```bash
# Preview fixes without applying (shows unified diff)
cargo run -- --fix --fix-dry-run path/to/sources

# Apply fixes (creates .bak backup files)
cargo run -- --fix path/to/sources

# Apply fixes without creating backups
cargo run -- --fix --no-backup path/to/sources
```

**Lints with auto-fix support:**
- `while_true_to_loop` - Replace `while (true)` with `loop`
- `empty_vector_literal` - Replace `vector::empty()` with `vector[]`
- `abilities_order` - Reorder struct abilities to canonical order (key, copy, drop, store)

## Development

- The repository is a single-member Cargo workspace so release/test profile settings (panic = abort, split debuginfo, `release-lto`) and dependency versions stay centralized.
- Basic tracing instrumentation is available via the `telemetry` feature (on by default). Set `RUST_LOG=move_clippy=info` to inspect spans around semantic linting and fixture modernization.

## Documentation

- [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md) - Developer workflow and architecture
- [`docs/STABILITY.md`](docs/STABILITY.md) - Lint stability policy and configuration
- [`docs/SECURITY_LINTS.md`](docs/SECURITY_LINTS.md) - Security lint reference
- [`docs/LINT_DEVELOPMENT_GUIDE.md`](docs/LINT_DEVELOPMENT_GUIDE.md) - How to add new lints
