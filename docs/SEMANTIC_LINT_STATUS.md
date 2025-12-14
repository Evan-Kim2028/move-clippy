# Semantic Lint Status

This document tracks the implementation status of semantic lints in move-clippy.
Semantic lints require Move compiler typing information and are only available when:

- Built with the `full` feature flag
- Run with `--mode full`
- Targeting a valid Move package

**Last Updated**: 2025-12-14

## Overview

| Status | Count | Description |
|--------|-------|-------------|
| ✅ Active (Custom) | 9 | Fully implemented in move-clippy |
| ⚡ Delegated (Sui) | 11 | Handled by Sui Move compiler visitors |
| **Total** | **20** | Total semantic lint descriptors |

---

## Architecture: Delegated vs Custom Lints

move-clippy uses a **hybrid approach** for semantic linting:

### Delegated Lints (Sui Compiler)
- **Implementation**: Sui Move compiler's built-in lint visitors
- **Location**: `move-compiler/src/sui_mode/linters/`
- **Technology**: SimpleAbsInt (abstract interpretation), CFGIRVisitor
- **Integration**: `lint_sui_visitors()` in `src/semantic.rs`
- **Why**: Leverage production-quality lints from Sui team
- **Count**: 11 lints

### Custom Lints (move-clippy)
- **Implementation**: Custom typing AST traversal in `src/semantic.rs`
- **Technology**: TypingProgramInfo, Typing AST recursion, basic state tracking
- **Why**: Security patterns from audits not in Sui compiler
- **Count**: 9 lints (3 naming + 6 security)

---

## Active Lints (Implemented in move-clippy)

These lints have full implementations in `src/semantic.rs`.

### Naming Lints (Custom Implementation)

| Lint | Function | Analysis Type | Description |
|------|----------|---------------|-------------|
| `capability_naming` | `lint_capability_naming` | TypingProgramInfo | Capability structs (key+store) should be suffixed with `_cap` |
| `event_naming` | `lint_event_naming` | TypingProgramInfo | Event structs (copy+drop) should follow `<past_tense>_<noun>_event` pattern |
| `getter_naming` | `lint_getter_naming` | Typing AST body inspection | Avoid `get_` prefix for simple field getters taking `&Self` |

### Security Lints (Custom Implementation)

| Lint | Function | Analysis Type | Reference |
|------|----------|---------------|-----------|
| `unfrozen_coin_metadata` | `lint_unfrozen_coin_metadata` | Typing AST recursion | MoveBit 2023-07-07 |
| `unused_capability_param` | `lint_unused_capability_param` | Typing AST recursion + var tracking | SlowMist 2024 |
| `unchecked_division` | `lint_unchecked_division` | Basic state tracking (validated vars) | Common vulnerability |
| `oracle_zero_price` | `lint_oracle_zero_price` | Basic state tracking (validated prices) | Bluefin Audit 2024 |
| `unused_return_value` | `lint_unused_return_value` | Typing AST recursion | DeFi audit patterns |
| `missing_access_control` | `lint_missing_access_control` | Type + heuristics | SlowMist 2024 |

**Implementation Note**: Current custom lints use simple AST traversal with basic state tracking. Phase II will upgrade priority lints to use SimpleAbsInt for better control-flow awareness (see `SEMANTIC_LINTER_EXPANSION_SPEC.md`).

---

## Delegated Lints (Sui Compiler Visitors)

These lints are **not implemented** in move-clippy code. Instead, they are handled by the 
Sui Move compiler's built-in lint visitors. The descriptors exist for:

1. **Documentation**: Listing available semantic checks in `list-rules`
2. **Configuration**: Allowing `allow`/`deny` settings in `move-clippy.toml`
3. **Output Aggregation**: Collecting Sui compiler warnings into move-clippy's output format

### Sui Visitor Lints

| Lint | Description | Sui Code | Analysis Type |
|------|-------------|----------|---------------|
| `share_owned` | Possible owned object share may abort | `W04001` | Abstract Interpretation |
| `self_transfer` | Non-composable transfer to sender | `W04002` | Abstract Interpretation |
| `custom_state_change` | Custom transfer/share/freeze should call private variants | `W04003` | Call Graph |
| `coin_field` | Avoid storing `sui::coin::Coin` fields in structs | `W04004` | Type Visitor |
| `freeze_wrapped` | Do not wrap shared objects before freezing | `W04005` | Abstract Interpretation |
| `collection_equality` | Avoid equality checks over bags/tables/collections | `W04006` | Type Visitor |
| `public_random` | Random state should remain private and uncopyable | `W04007` | Type Visitor |
| `missing_key` | Shared/transferred structs should have key ability | `W04008` | Type Visitor |
| `freezing_capability` | Avoid storing freeze capabilities | `W04009` | Type Visitor |
| `prefer_mut_tx_context` | Use `&mut TxContext` instead of `&TxContext` | `W04010` | Type Visitor |
| `unnecessary_public_entry` | Remove unnecessary `public` on `entry` functions | `W04011` | Type Visitor |

### How Delegation Works

The `lint_sui_visitors()` function in `src/semantic.rs`:

1. Compiles the Move package using the Sui compiler
2. Registers Sui's lint visitors via `add_visitors(linters::linter_visitors())`
3. Collects all warnings from Sui's lint visitors
4. Maps Sui warning codes (e.g., `W04001`) to move-clippy descriptors via `descriptor_for_sui_code()`
5. Converts locations and formats to move-clippy's output format

```rust
fn lint_sui_visitors(
    out: &mut Vec<Diagnostic>,
    settings: &LintSettings,
    build_plan: &BuildPlan,
    package_root: &Path,
) -> ClippyResult<()> {
    // Build with Sui compiler, collect its warnings
    build_plan.compile_with_driver_and_deps(deps, &mut writer, |compiler| {
        let (attr, filters) = linters::known_filters();
        let compiler = compiler
            .add_custom_known_filters(attr, filters)
            .add_visitors(linters::linter_visitors(CompilerLintLevel::All));
        // ... collect diagnostics ...
    })?;
    // Map to move-clippy diagnostics
}
```

---

## Lint Categories

| Category | Active (Custom) | Delegated (Sui) | Total |
|----------|-----------------|-----------------|-------|
| Naming | 3 | 0 | 3 |
| Security | 6 | 0 | 6 |
| Suspicious | 0 | 9 | 9 |
| Modernization | 0 | 2 | 2 |
| **Total** | **9** | **11** | **20** |

---

## Test Coverage

### Active Lint Tests (Custom)

| Lint | Test File | Test Type | Coverage |
|------|-----------|-----------|----------|
| `capability_naming` | `tests/sui_lints.rs` | Snapshot | ✅ Good |
| `event_naming` | - | - | ❌ TODO |
| `getter_naming` | - | - | ❌ TODO |
| `unfrozen_coin_metadata` | `tests/semantic_snapshots.rs` | Snapshot | ✅ Good |
| `unused_capability_param` | - | - | ❌ TODO |
| `unchecked_division` | `tests/semantic_snapshots.rs` | Snapshot | ✅ Good |
| `oracle_zero_price` | `tests/semantic_snapshots.rs` | Snapshot | ✅ Good |
| `unused_return_value` | - | - | ❌ TODO |
| `missing_access_control` | `tests/sui_lints.rs` | Snapshot (indirect) | ⚠️ Partial |

### Delegated Lint Tests (Sui)

| Lint | Test File | Test Type | Coverage |
|------|-----------|-----------|----------|
| `share_owned` | `tests/sui_lints.rs` | Snapshot | ✅ Good |
| `self_transfer` | `tests/sui_lints.rs` | Snapshot | ✅ Good |
| `custom_state_change` | `tests/sui_lints.rs` | Snapshot | ✅ Good |
| `coin_field` | - | - | ❌ TODO |
| `freeze_wrapped` | - | - | ❌ TODO |
| `collection_equality` | - | - | ❌ TODO |
| `public_random` | - | - | ❌ TODO |
| `missing_key` | - | - | ❌ TODO |
| `freezing_capability` | - | - | ❌ TODO |
| `prefer_mut_tx_context` | - | - | ❌ TODO |
| `unnecessary_public_entry` | - | - | ❌ TODO |

**Test Coverage Summary**: 6/20 lints have snapshot tests (30%)

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
