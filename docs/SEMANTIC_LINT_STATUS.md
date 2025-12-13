# Semantic Lint Status

This document tracks the implementation status of semantic lints in move-clippy.
Semantic lints require Move compiler typing information and are only available when:

- Built with the `full` feature flag
- Run with `--mode full`
- Targeting a valid Move package

**Last Updated**: 2025-12-13

## Overview

| Status | Count | Description |
|--------|-------|-------------|
| ✅ Active | 9 | Fully implemented in move-clippy |
| ⚡ Delegated | 9 | Handled by Sui Move compiler visitors |
| **Total** | **18** | Total semantic lint descriptors |

---

## Active Lints (Implemented in move-clippy)

These lints have full implementations in `src/semantic.rs`.

### Naming Lints

| Lint | Function | Description |
|------|----------|-------------|
| `capability_naming` | `lint_capability_naming` | Capability structs (key+store) should be suffixed with `_cap` |
| `event_naming` | `lint_event_naming` | Event structs (copy+drop) should follow `<past_tense>_<noun>_event` pattern |
| `getter_naming` | `lint_getter_naming` | Avoid `get_` prefix for simple field getters taking `&Self` |

### Security Lints

| Lint | Function | Description | Reference |
|------|----------|-------------|-----------|
| `unfrozen_coin_metadata` | `lint_unfrozen_coin_metadata` | CoinMetadata should be frozen, not shared | MoveBit 2023-07-07 |
| `unused_capability_param` | `lint_unused_capability_param` | Capability parameters that are passed but never used | DeFi audit patterns |
| `unchecked_division` | `lint_unchecked_division` | Division without zero-check may panic | Common vulnerability |
| `oracle_zero_price` | `lint_oracle_zero_price` | Oracle prices should be validated before use | Price manipulation audits |
| `unused_return_value` | `lint_unused_return_value` | Important return values (balances, results) should not be discarded | DeFi audit patterns |
| `missing_access_control` | `lint_missing_access_control` | Public functions modifying state should have capability-based access control | Access control audits |

---

## Delegated Lints (Sui Compiler Visitors)

These lints are **not implemented** in move-clippy code. Instead, they are handled by the 
Sui Move compiler's built-in lint visitors. The descriptors exist for:

1. **Documentation**: Listing available semantic checks in `list-rules`
2. **Configuration**: Allowing `allow`/`deny` settings in `move-clippy.toml`
3. **Output Aggregation**: Collecting Sui compiler warnings into move-clippy's output format

### Sui Visitor Lints

| Lint | Description | Sui Lint Code |
|------|-------------|---------------|
| `share_owned` | Possible owned object share may abort | `W04001` |
| `self_transfer` | Non-composable transfer to sender | `W04002` |
| `custom_state_change` | Custom transfer/share/freeze should call private variants | `W04003` |
| `coin_field` | Avoid storing `sui::coin::Coin` fields in structs | `W04004` |
| `freeze_wrapped` | Do not wrap shared objects before freezing | `W04005` |
| `collection_equality` | Avoid equality checks over bags/tables/collections | `W04006` |
| `public_random` | Random state should remain private and uncopyable | `W04007` |
| `missing_key` | Shared/transferred structs should have key ability | `W04008` |
| `freezing_capability` | Avoid storing freeze capabilities | `W04009` |

### How Delegation Works

The `lint_sui_visitors()` function in `src/semantic.rs`:

1. Compiles the Move package using the Sui compiler
2. Collects all warnings from Sui's lint visitors
3. Maps Sui warning codes (e.g., `W04001`) to move-clippy descriptors
4. Converts locations and formats to move-clippy's output format

```rust
fn lint_sui_visitors(
    out: &mut Vec<Diagnostic>,
    settings: &LintSettings,
    build_plan: &BuildPlan,
    package_root: &Path,
) -> ClippyResult<()> {
    // Build with Sui compiler, collect its warnings
    // Map to move-clippy diagnostics
}
```

---

## Lint Categories

| Category | Active | Delegated | Total |
|----------|--------|-----------|-------|
| Naming | 3 | 0 | 3 |
| Security | 6 | 0 | 6 |
| Suspicious | 0 | 9 | 9 |
| **Total** | **9** | **9** | **18** |

---

## Test Coverage

### Active Lint Tests

| Lint | Test File | Test Type |
|------|-----------|-----------|
| `capability_naming` | `tests/sui_lints.rs` | Snapshot |
| `event_naming` | - | TODO |
| `getter_naming` | - | TODO |
| `unfrozen_coin_metadata` | `tests/semantic_snapshots.rs` | Snapshot |
| `unused_capability_param` | - | TODO |
| `unchecked_division` | `tests/semantic_snapshots.rs` | Snapshot |
| `oracle_zero_price` | `tests/semantic_snapshots.rs` | Snapshot |
| `unused_return_value` | - | TODO |
| `missing_access_control` | `tests/sui_lints.rs` | Snapshot (indirect) |

### Delegated Lint Tests

| Lint | Test File | Test Type |
|------|-----------|-----------|
| `share_owned` | `tests/sui_lints.rs` | Snapshot |
| `self_transfer` | `tests/sui_lints.rs` | Snapshot |
| `custom_state_change` | `tests/sui_lints.rs` | Snapshot |
| `coin_field` | - | TODO |
| `freeze_wrapped` | - | TODO |
| `collection_equality` | - | TODO |
| `public_random` | - | TODO |
| `missing_key` | - | TODO |
| `freezing_capability` | - | TODO |

---

## Configuration

### Enabling/Disabling Semantic Lints

In `move-clippy.toml`:

```toml
# Run in full mode to enable semantic lints
mode = "full"

[rules]
# Disable specific lints
share_owned = "allow"
self_transfer = "allow"

# Increase severity for security lints
unchecked_division = "deny"
oracle_zero_price = "deny"
```

### Command Line

```bash
# Enable full mode for semantic lints
move-clippy --mode full ./sources

# Skip specific lints
move-clippy --mode full --skip share_owned,self_transfer ./sources

# Only run specific lints
move-clippy --mode full --only unchecked_division,oracle_zero_price ./sources
```

---

## Future Work

### Planned Enhancements

1. **Auto-fix for naming lints**: Generate suggestions for capability/event renaming
2. **Cross-module analysis**: Track capability flow across module boundaries
3. **Taint analysis**: Track untrusted data from oracles through calculations
4. **Custom lint plugins**: Allow users to define project-specific semantic lints

### Potential New Lints

| Lint | Description | Priority |
|------|-------------|----------|
| `unchecked_borrow` | Borrow from table/bag without exists check | High |
| `reentrancy_risk` | State changes after external calls | High |
| `timestamp_dependence` | Logic depending on `clock::timestamp_ms()` | Medium |
| `flashloan_vulnerability` | Unchecked loan repayment | Medium |

---

## References

- [Sui Move Security Principles](https://movebit.xyz/blog/post/Sui-Objects-Security-Principles-and-Best-Practices.html) - MoveBit 2023
- [Sui Linter Documentation](https://docs.sui.io/guides/developer/first-app/debug#linting) - Sui Docs
- [Move Prover](https://github.com/move-language/move/tree/main/language/move-prover) - Formal verification
