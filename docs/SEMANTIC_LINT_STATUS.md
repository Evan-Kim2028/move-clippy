# Semantic Lint Status

**Last Updated**: 2025-12-14 (Post-Framework Validation Audit)

---

## ⚠️ CRITICAL: Most Semantic Lints Are Broken

Running `move-clippy --mode full` on the **Sui framework** produces **792 false positives**.

**Root Cause**: Lints use **name-based heuristics** (`name.ends_with("_cap")`) instead of **type-based detection** (`abilities.has_ability_(Ability_::Key)`).

**See**: Issue #24 for complete audit, Issue #12 for roadmap.

---

## Lint Status Summary

| Category | Count | Status |
|----------|-------|--------|
| **Sui Delegated (WORKING)** | 9 | ✅ Production-ready |
| **Style/Modernization (WORKING)** | 17 | ✅ Production-ready |
| **Security - Semantic (BROKEN)** | 9 | ❌ 792 FPs on framework |
| **Security - Syntactic (MIXED)** | 15 | ⚠️ Some deprecated |
| **Phase II/III (BROKEN)** | 7 | ❌ Use heuristics |

---

## Working Lints (30 total)

### Sui Delegated Lints (9) - ✅ PRODUCTION READY

These use the Sui compiler's proper type analysis:

| Lint | Description |
|------|-------------|
| `share_owned` | Possible owned object share |
| `self_transfer` | Transfer back to sender |
| `custom_state_change` | Custom transfer/share/freeze |
| `coin_field` | Avoid Coin fields in structs |
| `freeze_wrapped` | Don't wrap shared before freeze |
| `collection_equality` | No equality on bags/tables |
| `public_random` | Random must be private |
| `missing_key` | Shared objects need key |
| `freezing_capability` | Avoid freeze capabilities |

### Style/Modernization Lints (17) - ✅ PRODUCTION READY

Pure syntax checks, no semantic analysis needed:

| Lint | Description |
|------|-------------|
| `abilities_order` | Struct abilities ordering |
| `empty_vector_literal` | Use `vector[]` |
| `while_true_to_loop` | Use `loop` |
| ... | (and 14 more) |

### Test Quality (3) + Conventions (1) - ✅ PRODUCTION READY

Syntactic checks that work correctly.

---

## Broken Lints (28 total)

### Naming Lints (3) - ❌ WRONG CONVENTIONS

| Lint | FPs | Problem | Issue |
|------|-----|---------|-------|
| `capability_naming` | 22 | Enforces `_cap` but Sui uses `Cap` | #20 |
| `event_naming` | 11 | Enforces `_event` suffix, Sui doesn't use | #21 |
| `getter_naming` | 5 | Flags `get_` prefix, Sui uses it | #23 |

### Security - Semantic (6) - ❌ USE HEURISTICS

| Lint | FPs | Heuristic | Issue |
|------|-----|-----------|-------|
| `unused_capability_param` | 209 | `name.ends_with("_cap")` | #14 |
| `missing_access_control` | 181 | `name.contains("cap")` | #19 |
| `unchecked_division` | 9 | Format string parsing | #16 |
| `unused_return_value` | 8 | Function name list | #22 |
| `oracle_zero_price` | ? | `var_name.contains("price")` | - |
| `unfrozen_coin_metadata` | ? | `contains("CoinMetadata")` | - |

### Phase II/III (7) - ❌ USE HEURISTICS

| Lint | FPs | Heuristic | Issue |
|------|-----|-----------|-------|
| `transitive_capability_leak` | 302 | `name.ends_with("Cap")` | #18 |
| `unused_capability_param_v2` | 209 | `name.ends_with("Cap")` | #14 |
| `flashloan_without_repay` | 40 | `name.contains("borrow")` | #15 |
| `unchecked_division_v2` | 9 | Format string parsing | #16 |
| `oracle_price_taint` | ? | `contains("get_price")` | - |
| `price_manipulation_window` | ? | `contains("oracle")` | - |
| `resource_leak` | N/A | Not implemented | - |

### Syntactic Security (15) - ⚠️ MIXED

| Lint | Status | Notes |
|------|--------|-------|
| `suspicious_overflow_check` | ✅ OK | Flags explicit patterns |
| `stale_oracle_price` | ✅ OK | Flags `get_price_unsafe` API |
| `ignored_boolean_return` | ✅ OK | Specific API patterns |
| `droppable_hot_potato` | ⚠️ | Keyword-based |
| `shared_capability` | ⚠️ | Name-based |
| `excessive_token_abilities` | ❌ DEPRECATED | High FP |
| `unbounded_vector_growth` | ❌ DEPRECATED | High FP |
| `hardcoded_address` | ❌ DEPRECATED | High FP |

---

## The Fix: Type-Based Detection

The Move compiler provides complete type information:

```rust
// Capability: key + store, no copy/drop
fn is_capability_type(abilities: &AbilitySet) -> bool {
    abilities.has_ability_(Ability_::Key) 
        && abilities.has_ability_(Ability_::Store)
        && !abilities.has_ability_(Ability_::Copy)
}

// Hot Potato: NO abilities
fn is_hot_potato_type(abilities: &AbilitySet) -> bool {
    abilities.is_empty()
}

// Event: copy + drop only  
fn is_event_type(abilities: &AbilitySet) -> bool {
    abilities.has_ability_(Ability_::Copy) 
        && abilities.has_ability_(Ability_::Drop)
        && !abilities.has_ability_(Ability_::Key)
}
```

---

## Success Metrics

| Metric | Target | Current |
|--------|--------|---------|
| FPs on `sui-framework` | **0** | 792 |
| Lints using heuristics | **0** | 28 |
| Functional lints | 100% | 46% |

---

## References

- **#24**: [META] Eliminate All Heuristics
- **#12**: Semantic Analysis Roadmap
- **#14-23**: Individual lint fix issues
