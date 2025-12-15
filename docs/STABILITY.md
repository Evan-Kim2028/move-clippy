# Rule Stability Policy

Move Clippy uses a stability classification system inspired by [Ruff](https://docs.astral.sh/ruff/) to ensure high-quality, low-false-positive linting rules.

## Quick Reference

**Total Lints:** 59  
- **Stable:** 51 (enabled by default)
- **Preview:** 3 (require `--preview`)
- **Experimental:** 12 (require `--experimental`)
- **Deprecated:** 3 (backwards compatibility only)

**See also:** [LINT_REFERENCE.md](./LINT_REFERENCE.md) for complete lint catalog

---

## Analysis Types Explained

Move Clippy uses different analysis techniques depending on lint requirements:

| Analysis Type | Speed | Accuracy | Mode Required | Description |
|--------------|-------|----------|---------------|-------------|
| **Syntactic** | ⚡ Fast (10-100ms) | Pattern-based | Default | Tree-sitter parsing, no compilation needed |
| **TypeBased** | 🐢 Slower (1-5s) | Type-aware | `--mode full` | Uses Move compiler's type checker |
| **TypeBasedCFG** | 🐌 Slowest (5-15s) | Dataflow-aware | `--mode full` | Control-flow + dataflow analysis |
| **CrossModule** | 🐌 Slowest (10-30s) | Call-graph | `--mode full` | Analyzes across module boundaries |

### Why CFG Analysis Matters

**Syntactic lints** use pattern matching:
```move
// Syntactic lint: Does `destroy_zero` appear after `== 0`?
assert!(balance == 0, E_NOT_ZERO);
coin::destroy_zero(coin);  // ✅ Pattern matched
```

**CFG lints** track values through control flow:
```move
// CFG lint: Does the zero-check dominate destroy_zero on ALL paths?
if (condition) {
    assert!(balance == 0, E_NOT_ZERO);
    coin::destroy_zero(coin);  // ✅ Check dominates call
} else {
    coin::destroy_zero(coin);  // ❌ No check on this path!
}
```

**Result:** CFG lints have **near-zero false positives** but require compilation.

---

## Rule Groups

### Stable

**Definition:** Battle-tested rules with minimal false positives, enabled by default.

**Characteristics:**
- Low false-positive rate (< 1%)
- Clear, actionable error messages
- Well-documented behavior
- Consistent behavior across Move codebases
- FP prevention tests in `tests/false_positive_prevention.rs`

**Example usage:**
```bash
# Stable rules are enabled by default
move-clippy lint src/
```

### Preview

**Definition:** New rules that need community validation before becoming stable.

**Characteristics:**
- Near-zero false-positive rate (< 1%) with CFG analysis
- Behavior may change between versions
- Require explicit opt-in with `--preview`
- All current Preview lints use TypeBasedCFG analysis

**Current Preview Lints (3):**

| Lint | Analysis | FP Risk | Description |
|------|----------|---------|-------------|
| `unchecked_division_v2` | TypeBasedCFG | < 1% | Division without zero-check (dataflow-aware) |
| `destroy_zero_unchecked_v2` | TypeBasedCFG | < 1% | `destroy_zero` without verifying zero value |
| `fresh_address_reuse_v2` | TypeBasedCFG | < 1% | `fresh_object_address` result reused |

**Example usage:**
```bash
# Enable preview rules via CLI (requires compilation)
move-clippy lint --mode full --preview path/to/package

# Or via config file (move-clippy.toml)
[lints]
preview = true
```

**Why Preview and not Stable?** These lints have excellent accuracy but we're gathering feedback on:
- Performance impact (CFG analysis is slower)
- Error message clarity
- Edge case handling

### Experimental

**Definition:** Rules with medium-high false-positive risk, useful for research and security audits.

**Characteristics:**
- Medium-high false-positive rate (5-20% depending on lint)
- Detection strategies are heuristic-based or use simple pattern matching
- Not recommended for CI pipelines
- Require `--experimental` flag (implies `--preview`)
- Useful for security audits and codebase exploration

**Current Experimental Lints (12):**

**High FP - Heuristic Detection (8 lints):**
| Lint | Analysis | FP Risk | Reason |
|------|----------|---------|--------|
| `destroy_zero_unchecked` | Syntactic | Medium | No CFG - can't see cross-function guarantees |
| `otw_pattern_violation` | Syntactic | Medium | Module naming edge cases |
| `digest_as_randomness` | Syntactic | Medium | Keyword-based detection |
| `fresh_address_reuse` | Syntactic | Medium | Simple counting heuristic |
| `unchecked_coin_split` | Syntactic | High | **Deprecated** - Sui runtime enforces this |
| `unchecked_withdrawal` | Syntactic | High | **Deprecated** - requires formal verification |
| `pure_function_transfer` | Syntactic | Medium-High | Many legitimate patterns |
| `unsafe_arithmetic` | Syntactic | High | Variable name heuristics |

**Medium FP - Complex Analysis (4 lints):**
| Lint | Analysis | FP Risk | Reason |
|------|----------|---------|--------|
| `phantom_capability` | TypeBasedCFG | Medium | "Privileged sink" detection uses heuristics |
| `capability_transfer_v2` | TypeBased | Medium | Intentional cap grants are common |
| `transitive_capability_leak` | CrossModule | Medium | Cross-module analysis edge cases |
| `flashloan_without_repay` | CrossModule | Medium | Naming heuristics for flashloan patterns |

**Example usage:**
```bash
# Enable experimental rules via CLI
move-clippy lint --experimental src/

# Experimental implies preview, so both are enabled
move-clippy lint --experimental --show-tier src/

# Or via config file (move-clippy.toml)
[lints]
experimental = true  # Implies preview = true
```

**When to use Experimental:**
- ✅ Security audits where false positives are acceptable
- ✅ Exploring potential issues in a codebase
- ✅ Research on new lint patterns
- ❌ **DO NOT** use in CI/CD pipelines
- ❌ Daily development workflow

**CFG Versions Available:** For syntactic lints with high FP, check if a `_v2` CFG version exists (e.g., `destroy_zero_unchecked_v2`).

### Deprecated

**Definition:** Rules that have been superseded by better implementations or are no longer useful.

**Characteristics:**
- Not enabled by default
- Require `--experimental` flag to enable (for backwards compatibility)
- Will be removed in next major version

**Current Deprecated Lints (3):**

| Lint | Reason | Superseded By |
|------|--------|---------------|
| `unchecked_coin_split` | Sui runtime already enforces balance checks | (runtime enforcement) |
| `unchecked_withdrawal` | Business logic bugs require formal verification, not linting | (formal methods) |
| `capability_leak` | Name-based heuristics superseded by type-based detection | `capability_transfer_v2` |

---

## Current Lint Lists

### Stable Lints (51)

**Syntactic - Style & Conventions (28):**
- `abilities_order` - Enforce canonical ability ordering
- `while_true_to_loop` - Prefer `loop` over `while (true)`
- `modern_module_syntax` - Use modern module syntax
- `redundant_self_import` - Remove unnecessary Self imports
- `prefer_to_string` - Prefer to_string() over manual formatting
- `constant_naming` - SCREAMING_SNAKE_CASE for constants
- `unneeded_return` - Remove redundant return keyword
- `doc_comment_style` - Use /// for documentation
- `event_suffix` - Event structs should have Event suffix
- `empty_vector_literal` - Prefer vector[] over vector::empty()
- `typed_abort_code` - Use named constants for abort codes
- `test_abort_code` - Test aborts should use named constants
- `redundant_test_prefix` - Don't prefix test functions with test_
- `merge_test_attributes` - Merge multiple #[test] attributes
- `admin_cap_position` - Admin cap should be first parameter
- `equality_in_assert` - Use assert!(a == b) not assert(a == b, ...)
- `manual_option_check` - Use option::is_some/is_none
- `manual_loop_iteration` - Use for loops instead of while with index
- `prefer_vector_methods` - Use vector methods over manual operations
- `modern_method_syntax` - Use method call syntax
- `explicit_self_assignments` - Remove redundant self assignments
- `unnecessary_public_entry` - Remove public from entry functions
- `public_mut_tx_context` - TxContext should be &mut in public functions
- (+ 5 more style lints)

**Syntactic - Security (6):**
- `stale_oracle_price` - Using get_price_unsafe without freshness check
- `single_step_ownership_transfer` - Admin transfer without two-step confirmation
- `missing_witness_drop` - OTW struct missing drop ability
- `public_random_access` - Public function exposes Random object
- `suspicious_overflow_check` - Manual overflow checks are error-prone
- `ignored_boolean_return` - Boolean return value ignored
- `divide_by_zero_literal` - Division by literal zero

**TypeBased - Semantic (13):**
- `share_owned_authority` - Don't share key+store objects
- `droppable_hot_potato_v2` - Hot potato has drop ability
- `unused_return_value` - Important return value ignored
- `event_emit_type_sanity` - Emitting non-event type
- (+ 9 Sui monorepo pass-through lints)

### Preview Lints (3)

All use **TypeBasedCFG** analysis with near-zero FP:
- `unchecked_division_v2` - Division without zero-check (CFG-aware)
- `destroy_zero_unchecked_v2` - destroy_zero without verifying zero (CFG-aware)
- `fresh_address_reuse_v2` - fresh_object_address result reused (CFG-aware)

### Experimental Lints (12)

See detailed list in "Experimental" section above.

### Deprecated Lints (3)

See "Deprecated" section above.

---

## FP Risk Assessment Methodology

// ... existing code ...
