# Fixtures (Executable Documentation)

**Purpose:** Fixtures are the most drift-resistant documentation we have. If a fixture passes, the behavior it demonstrates is real.

## Fixture Types

### Fast mode (tree-sitter)

- Single `.move` files used by fast-mode snapshot tests and unit tests.
- Directive syntax for fast-mode fixtures:
  - `#[allow(lint::<name|category>)]`
  - `#[deny(lint::<name|category>)]`
  - `#[expect(lint::<name|category>)]`
  - Module/file header forms are also supported for move-clippy parsing:
    - `#![allow(lint::<name|category>)]`, etc.

Note: `#![...]` is treated as a move-clippy directive and may not be accepted by the Move compiler; it’s primarily intended for fast-mode fixtures.

### Full mode (Move compiler / `semantic::lint_package`)

- Fixture packages under `tests/fixtures/**/<pkg>/` that contain a `Move.toml` and `sources/`.
- Directive syntax for full-mode fixtures must be compiler-valid:
  - `#[ext(move_clippy(allow(<name|category>)))]`
  - `#[ext(move_clippy(deny(<name|category>)))]`
  - `#[ext(move_clippy(expect(<name|category>)))]`

## Recommended “Lint Contract” For New Lints

When adding a lint, prefer adding three minimal fixtures/tests:

1. **Positive**: must trigger.
2. **Negative**: must not trigger.
3. **Directive coverage**: prove `allow/deny/expect` works for the lint in its intended mode.

If the lint is intended to be “zero false positives” (Stable tier), add a couple of “near miss” negatives that are visually similar to the positive but should not trigger.

## Where To Add What

- Fast-mode fixture snapshots: `tests/syntactic_snapshots.rs`
- Full-mode package snapshots: `tests/semantic_package_snapshots.rs`
- Spec-style semantic invariants: `tests/*_spec.rs`

## Fixture Layout (Directory Map)

- `tests/fixtures/<lint_name>/...`: Fast-mode fixtures (single `.move` files) used by syntactic snapshots.
- `tests/fixtures/semantic_pkg/`: Shared minimal full-mode package used by some semantic tests.
- `tests/fixtures/phase2/<pkg>/`: Phase II (semantic / type-based) fixture packages.
- `tests/fixtures/phase3/<pkg>/`: Phase III (CFG / abstract interpretation) fixture packages.
- `tests/fixtures/phase4/<pkg>/`: Phase IV (cross-module) fixture packages.

## WIP / Legacy Fixtures

Some fixture packages are intentionally kept as work-in-progress or historical references.

- If a fixture directory is not referenced by any test harness (e.g. not in `tests/semantic_package_snapshots.rs`), treat it as a sandbox, not an invariant.
- Prefer adding new fixtures to the active phase directory and wiring them into the relevant snapshot/spec tests.

## Sui Framework Shims (Test-Only)

Full-mode fixtures are compiled with the Move compiler, but we keep them self-contained by defining minimal `module sui::...` shims in fixture sources.

- Prefer staying faithful to real Sui rules (e.g. `key` structs require an `id: sui::object::UID` field).
- For some semantic/spec tests (notably ability-mismatch matrices), we intentionally give the local `UID` shim extra abilities (`copy/drop/store`) to explore “impossible in production” combinations. This is acceptable only when the lint’s contract is purely about ability sets and does not depend on real Sui framework invariants.

