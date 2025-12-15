# Move Clippy

Move linter inspired by Rust Clippy.

Move Clippy runs in two modes:
- **Fast**: tree-sitter (syntactic) analysis over `.move` sources.
- **Full**: Move compiler typing + delegated Sui lints (requires building with `--features full`).

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

### Stable (Default)

- **Enabled by default** - Zero configuration needed
- **Low false-positive rate** (<1%) - Minimal noise
- **Production-ready** - Safe for CI/CD pipelines
- **Battle-tested** - Validated across multiple Move projects

Example stable lints:
- `abilities_order` - Enforce canonical ability ordering
- `while_true_to_loop` - Prefer `loop` over `while (true)`
- `droppable_hot_potato` - Detect hot potato structs with drop ability
- `stale_oracle_price` - Detect unsafe oracle price fetching

### Preview

- **Require `--preview` flag** - Explicit opt-in needed
- **Higher FP rate** (1-5%) - More noise, still useful
- **May change** - Behavior/messages may evolve between versions
- **Community validation** - Gathering feedback for promotion to Stable

```bash
move-clippy lint --preview src/
```

Example preview lints:
- `pure_function_transfer` - Detect functions that should return objects
- `unsafe_arithmetic` - Detect potentially unsafe arithmetic
- `suspicious_overflow_check` - Detect manual overflow checks

### Experimental

- **Require `--experimental` flag** - Double opt-in (implies --preview)
- **High FP rate** (>5-10%) - Many false positives expected
- **Research/audit use only** - NOT for CI/CD
- **Heuristic detection** - Name-based or pattern-based without dataflow

```bash
move-clippy lint --experimental src/
```

**When to use Experimental:**
- ✅ Security audits where you want to explore all potential issues
- ✅ Research on new vulnerability patterns
- ✅ One-time codebase exploration
- ❌ CI/CD pipelines (will produce too much noise)
- ❌ Daily development workflow

Example experimental lints:
- `unchecked_coin_split` - Detect coin splits without balance checks
- `unchecked_withdrawal` - Detect withdrawals without balance validation
- `capability_leak` - Detect capability transfers without recipient validation

**Tier Promotion:**
Lints are promoted from Experimental → Preview → Stable based on:
- False positive rate reduction
- Ecosystem validation
- Community feedback
- Detection strategy improvements

See `docs/STABILITY.md` for detailed tier policies and promotion criteria.

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
- **CrossModule**: call-graph / inter-module checks (runs in `--mode full --preview`).

### What’s available (at a glance)

For the complete, up-to-date table (counts + per-lint detail), see `docs/LINT_INVENTORY.md`.

### Categories

- `style`, `modernization`, `naming`, `test_quality`, `suspicious`, `security`

### Notable lints (examples)

This is a representative sample (not a full list):

- Style/modernization: `while_true_to_loop`, `abilities_order`, `modern_module_syntax`
- Test quality: `test_abort_code`, `redundant_test_prefix`
- Suspicious/Sui: `share_owned`, `self_transfer`, `coin_field`, `missing_key`
- Security: `droppable_hot_potato`, `stale_oracle_price`, `public_random_access`

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
