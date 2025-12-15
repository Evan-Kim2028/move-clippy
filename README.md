# Move Clippy

Move linter inspired by Rust Clippy.

Move Clippy runs in two modes:
- **Fast**: tree-sitter (syntactic) analysis over `.move` sources.
- **Full**: Move compiler typing + delegated Sui lints (requires building with `--features full`).

> **Complete Reference:** See [docs/LINT_REFERENCE.md](docs/LINT_REFERENCE.md) for all 59 lints with tiers, analysis types, and FP risk

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

**Quick Stats:**
- **59 total lints**: 51 Stable | 3 Preview | 12 Experimental | 3 Deprecated
- **FP Rates**: Stable <1% | Preview <1% | Experimental 5-20%

### Stable (Default)

- **Enabled by default** - Zero configuration needed
- **Low false-positive rate** (<1%) - Minimal noise
- **Production-ready** - Safe for CI/CD pipelines
- **Battle-tested** - Validated across multiple Move projects

**Breakdown:**
- 28 syntactic style lints (fast mode)
- 6 syntactic security lints (fast mode)
- 13 type-based semantic lints (full mode)
- 4 Sui monorepo pass-through lints (full mode)

Example stable lints:
- `abilities_order` - Enforce canonical ability ordering
- `while_true_to_loop` - Prefer `loop` over `while (true)`
- `droppable_hot_potato_v2` - Detect hot potato structs with drop ability
- `stale_oracle_price` - Detect unsafe oracle price fetching
- `divide_by_zero_literal` - Division by literal zero

### Preview

- **Require `--preview` flag** - Explicit opt-in needed
- **Near-zero FP rate** (<1%) - CFG-based analysis
- **May change** - Gathering feedback on performance and ergonomics
- **All use TypeBasedCFG** - Requires `--mode full`

```bash
move-clippy lint --mode full --preview path/to/package
```

**Current Preview lints (3):**
- `unchecked_division_v2` - Division without zero-check (CFG-aware)
- `destroy_zero_unchecked_v2` - `destroy_zero` without verifying zero value (CFG-aware)
- `fresh_address_reuse_v2` - `fresh_object_address` result reused (CFG-aware)

**Why Preview?** These lints use precise dataflow analysis with excellent accuracy, but we're gathering feedback on performance impact and edge cases before promoting to Stable.

### Experimental

- **Require `--experimental` flag** - Double opt-in (implies --preview)
- **Medium-high FP rate** (5-20%) - Many false positives expected
- **Research/audit use only** - NOT for CI/CD
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

**Current Experimental lints (12):**
- 8 heuristic-based syntactic lints (medium FP)
- 4 complex analysis lints with edge cases (medium FP)

**Tip:** Many experimental lints have CFG-based `_v2` versions in Preview tier with near-zero FP.

Example experimental lints:
- `destroy_zero_unchecked` - Syntactic version (use `_v2` for CFG)
- `otw_pattern_violation` - Module naming edge cases
- `digest_as_randomness` - Keyword-based detection
- `phantom_capability` - Privileged sink heuristics

**Tier Promotion:**
Lints are promoted from Experimental → Preview → Stable based on:
- False positive rate reduction (measured on ecosystem repos)
- Ecosystem validation (deepbookv3, openzeppelin-sui, etc.)
- Community feedback
- Detection strategy improvements (e.g., syntactic → CFG upgrade)

See [docs/STABILITY.md](docs/STABILITY.md) for detailed tier policies and promotion criteria.

---

## Understanding Analysis Types

Move Clippy uses different analysis techniques with trade-offs between speed and accuracy:

| Analysis Type | Speed | Accuracy | Mode | Description |
|--------------|-------|----------|------|-------------|
| **Syntactic** | ⚡ Fast (10-100ms) | Pattern-based | `--mode fast` (default) | Tree-sitter parsing, no compilation |
| **TypeBased** | 🐢 Slower (1-5s) | Type-aware | `--mode full` | Uses Move compiler's type checker |
| **TypeBasedCFG** | 🐌 Slowest (5-15s) | Dataflow-aware | `--mode full --preview` | Control-flow + dataflow analysis |
| **CrossModule** | 🐌 Slowest (10-30s) | Call-graph | `--mode full --preview` | Analyzes across module boundaries |

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

**Result:** CFG lints have **near-zero false positives** but require compilation. This is why Preview lints (all CFG-based) have <1% FP despite being newer.

### Analysis Type Distribution

| Tier | Syntactic | TypeBased | TypeBasedCFG | CrossModule |
|------|-----------|-----------|--------------|-------------|
| **Stable** | 34 | 17 | 0 | 0 |
| **Preview** | 0 | 0 | 3 | 0 |
| **Experimental** | 8 | 2 | 1 | 2 |

**Insight:** All current Preview lints use CFG analysis, explaining their low FP rate despite being "preview" tier.

---

## TypeSystemGap Categories

Move Clippy classifies lints by the type system gaps they address. This helps systematically discover new lints.

**The 8 Gap Types:**

| Gap | Lints | Description |
|-----|-------|-------------|
| **AbilityMismatch** | 3 | Wrong ability combinations (e.g., hot potato with `drop`) |
| **OwnershipViolation** | 6 | Incorrect object ownership transitions |
| **CapabilityEscape** | 3 | Capabilities leaking intended scope |
| **ValueFlow** | 5 | Values going to wrong destinations |
| **ApiMisuse** | 8 | Incorrect stdlib/Sui function usage |
| **TemporalOrdering** | 3 | Operations in wrong sequence |
| **ArithmeticSafety** | 4 | Numeric operations without validation |
| **StyleConvention** | 27 | Style and convention issues |

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
- **CrossModule**: call-graph / inter-module checks (runs in `--mode full --preview`).

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
