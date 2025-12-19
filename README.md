# Move Clippy

Move linter inspired by Rust Clippy.

Move Clippy runs in two modes:
- **Fast**: tree-sitter (syntactic) analysis over `.move` sources.
- **Full**: Move compiler typing + delegated Sui lints (requires building with `--features full`).

> **Complete Reference:** See [docs/LINT_REFERENCE.md](docs/LINT_REFERENCE.md) for per-lint details. For the authoritative set in your build, run `move-clippy list-rules`.

> Full lint inventory: `docs/LINT_INVENTORY.md`

## Quickstart

```bash
# Fast mode (default)
move-clippy lint path/to/sources

# Enable preview rules (higher FP risk / may change)
move-clippy lint --preview path/to/sources

# Enable experimental rules (high FP risk, research/audits only)
move-clippy lint --experimental path/to/sources

# Show lint tier in output
move-clippy lint --show-tier path/to/sources

# Full mode against a Move package (requires building with --features full)
move-clippy lint --mode full --package path/to/move/package

# Output formats
move-clippy lint --format json path/to/sources
move-clippy lint --format github path/to/sources

# Apply safe fixes
move-clippy lint --fix path/to/sources

# Preview fixes without applying (unified diff)
move-clippy lint --fix --fix-dry-run path/to/sources

# Also apply unsafe fixes (review carefully)
move-clippy lint --fix --unsafe-fixes path/to/sources
```

## Lint Tier System

Move Clippy uses a three-tier stability system inspired by [Ruff](https://docs.astral.sh/ruff/):

The set of available lints depends on how move-clippy is built (notably whether `--features full` is enabled).

For the current build, use:

```bash
move-clippy list-rules
```

### Stable (Default)

- **Enabled by default** - Zero configuration needed
- **Low false-positive rate** - Minimal noise
- **Validated** - Tested across multiple Move projects

**Breakdown:**
The stable tier includes both syntactic and semantic rules depending on your build and chosen mode. Use `move-clippy list-rules` to see the current breakdown.

Example stable lints:
- `abilities_order` - Enforce canonical ability ordering
- `while_true_to_loop` - Prefer `loop` over `while (true)`
- `droppable_hot_potato_v2` - Detect hot potato structs with drop ability
- `stale_oracle_price` - Detect unsafe oracle price fetching
- `divide_by_zero_literal` - Division by literal zero

### Preview

- **Require `--preview` flag** - Explicit opt-in needed
- **High precision** - Intended to be low-noise, but still gathering validation
- **May change** - Gathering feedback on performance and ergonomics
- **Requires `--mode full`** - Uses compiler typing and/or CFG analysis

```bash
move-clippy lint --mode full --preview path/to/package
```

Preview rules are only available when building with `--features full`.

**Why Preview?** Some lints are dataflow-aware (CFG), others are conservative type-based checks. Both classes aim for low false positives, but we keep them opt-in while we validate them across more real packages.

### Experimental

- **Require `--experimental` flag** - Double opt-in (implies --preview)
- **Higher false-positive risk** - Research/audit use only
- **Heuristic detection** - Pattern-based or name-based without precise tracking

```bash
move-clippy lint --experimental src/
```

**When to use Experimental:**
- ✅ Security audits where you want to explore all potential issues
- ✅ Research on new vulnerability patterns
- ✅ One-time codebase exploration
- ❌ CI/CD pipelines (will produce too much noise)
- ❌ Daily development workflow

**Tip:** Prefer semantic/CFG-based lints for higher precision when available.

**Tier Promotion:**
Lints are promoted from Experimental → Preview → Stable based on:
- Ecosystem validation (deepbookv3, openzeppelin-sui, etc.)
- Community feedback
- Detection strategy improvements (e.g., syntactic → CFG upgrade)

See [docs/STABILITY.md](docs/STABILITY.md) for detailed tier policies and promotion criteria.

## Ecosystem Validation

Ecosystem runs and baselines live in the local runner at
`../ecosystem-test-repos` (manifest + baselines + runner). See
`../ecosystem-test-repos/README.md` for usage.

---

## Understanding Analysis Types

Move Clippy uses different analysis techniques with trade-offs between speed and accuracy:

| Analysis Type | Speed | Accuracy | Mode | Description |
|--------------|-------|----------|------|-------------|
| **Syntactic** | Fast | Pattern-based | `--mode fast` (default) | Tree-sitter parsing, no compilation |
| **TypeBased** | Slower | Type-aware | `--mode full` | Uses Move compiler's type checker |
| **TypeBasedCFG** | Slowest | Dataflow-aware | `--mode full --preview` | Control-flow + dataflow analysis |
| **CrossModule** | Slowest | Call-graph | `--mode full --experimental` | Analyzes across module boundaries |

### Why CFG Analysis Matters

**Syntactic lints** use simple pattern matching:
```move
// Syntactic: Does `destroy_zero` appear after `== 0`?
assert!(balance == 0, E_NOT_ZERO);
coin::destroy_zero(coin);  // ✅ Pattern matched
```

**CFG lints** track values through ALL execution paths:
```move
// CFG: Does the zero-check dominate destroy_zero on ALL paths?
if (condition) {
    assert!(balance == 0, E_NOT_ZERO);
    coin::destroy_zero(coin);  // ✅ Check dominates on this path
} else {
    coin::destroy_zero(coin);  // ❌ CFG detects: no check on this path!
}
```

**Result:** CFG lints have **near-zero false positives** but require compilation. This is why we gate them behind `--mode full` and typically place them in Preview while validating performance.

### Analysis Type Inventory

For a complete list (including tier + analysis kind per lint), use `move-clippy list-rules` or see `docs/LINT_REFERENCE.md`.

---

## Directives (allow/deny/expect)

move-clippy supports allow/deny/expect directives for both exact lint names and lint categories.

### Fast mode (tree-sitter)

- **Allow (suppress):** `#[allow(lint::<name|category>)]` or `#![allow(lint::<name|category>)]`
- **Deny (promote):** `#[deny(lint::<name|category>)]` or `#![deny(lint::<name|category>)]`
- **Expect (test invariant):** `#[expect(lint::<name|category>)]` or `#![expect(lint::<name|category>)]`

These forms are intended for move-clippy parsing and may not compile under the Move compiler.

### Full mode (compiler-valid)

Use `ext` attributes so packages still compile:

- **Allow:** `#[ext(move_clippy(allow(<name|category>)))]`
- **Deny:** `#[ext(move_clippy(deny(<name|category>)))]`
- **Expect:** `#[ext(move_clippy(expect(<name|category>)))]`

If an `expect` directive does not match any emitted diagnostics in its scope, move-clippy emits an `unfulfilled_expectation` error diagnostic.

Category names are the `LintCategory::as_str()` values:

- `style`, `modernization`, `naming`, `test_quality`, `suspicious`, `security`

---

## TypeSystemGap Categories

Move Clippy classifies lints by the type system gaps they address. This helps systematically discover new lints.

**The 8 Gap Types:**

| Gap | Description |
|-----|-------------|
| **AbilityMismatch** | Wrong ability combinations (e.g., hot potato with `drop`) |
| **OwnershipViolation** | Incorrect object ownership transitions |
| **CapabilityEscape** | Capabilities leaking intended scope |
| **ValueFlow** | Values going to wrong destinations |
| **ApiMisuse** | Incorrect stdlib/Sui function usage |
| **TemporalOrdering** | Operations in wrong sequence |
| **ArithmeticSafety** | Numeric operations without validation |
| **StyleConvention** | Style and convention issues |

**For Developers:** See [docs/TYPE_SYSTEM_GAPS.md](docs/TYPE_SYSTEM_GAPS.md) for detailed gap taxonomy and how to use it for lint discovery.

## Commands

- `move-clippy lint [PATH ...]`: Lint files/directories (or stdin if no PATH is provided).
- `move-clippy list-rules`: List available lints.
- `move-clippy explain <lint>`: Show a short explanation of a lint.
- `move-clippy triage …`: Import/track findings and generate reports.

## Coverage (condensed)

Move Clippy organizes rules by **analysis kind** and **stability group**.

### Analysis kinds

- **Syntactic**: tree-sitter pattern matching (runs in `--mode fast`).
- **TypeBased**: Move compiler typing info (runs in `--mode full`).
- **TypeBasedCFG**: CFG-aware analysis via abstract interpretation (runs in `--mode full --preview`).
- **CrossModule**: call-graph / inter-module checks (runs in `--mode full --experimental`).

### What’s available (at a glance)

For the complete, up-to-date table (counts + per-lint detail), see `docs/LINT_INVENTORY.md`.

### Categories

- `style`, `modernization`, `naming`, `test_quality`, `suspicious`, `security`

### Notable lints (examples)

This is a representative sample (not a full list):

- Style/modernization: `while_true_to_loop`, `abilities_order`, `modern_module_syntax`
- Test quality: `test_abort_code`, `redundant_test_prefix`
- Security: `droppable_hot_potato_v2`, `stale_oracle_price`, `public_random_access`, `share_owned_authority`
- Sui Monorepo (pass-through): `share_owned`, `self_transfer`, `coin_field`, `missing_key`

> **Note:** Sui Monorepo lints are pass-through wrappers for the official lints from `sui_mode::linters`. They provide unified output formatting in `--mode full`.

## Full mode and Sui integration

When built with `--features full`, Move Clippy uses the local Sui monorepo Move compiler crates
(via `../sui/external-crates/move/...` path dependencies in `Cargo.toml`). In full mode it:

- Builds the Move package to obtain compiler typing information.
- Delegates Sui-specific semantic lints to the upstream compiler implementation.

See `src/semantic.rs` for the integration entrypoint.

## Configuration

Move Clippy supports `move-clippy.toml` for per-lint levels and enable/disable lists.
Use `move-clippy list-rules` to discover lint names; use `move-clippy explain <lint>` to learn what they check.

## Development

See `docs/DEVELOPMENT.md` for contributor workflow and `docs/LINT_DEVELOPMENT_GUIDE.md` for adding new lints.

Related:
- `docs/FP_PREVENTION.md` (how we keep Stable lints low-noise)
- `docs/STABILITY.md` (tier policy and promotion criteria)
- `tests/fixtures/README.md` (fixtures as executable documentation)
