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

## References

- Design notes live in `../notes/move-clippy/`, especially:
  - `04-focused-roadmap.md` for immediate milestones
  - `06-semantic-analysis-path.md` for analysis approach
