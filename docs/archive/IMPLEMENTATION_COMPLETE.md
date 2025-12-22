# Semantic Linter Implementation Status

**Last Updated**: 2025-12-14

---

## ⚠️ STATUS: Infrastructure Complete, Lints Broken

The Phase II/III **infrastructure** (SimpleAbsInt, CallGraph) was successfully implemented, but validation against the Sui framework revealed that **all security lints use broken heuristics**.

### What Was Built (Infrastructure - Working)

- ✅ SimpleAbsInt abstract interpretation framework
- ✅ CallGraph for cross-module analysis
- ✅ Integration with Move compiler typing AST
- ✅ Diagnostic infrastructure

### What's Broken (Detection Logic)

All security lints use **name-based pattern matching** instead of **type-based detection**:

| Lint | Heuristic Used | FPs on Framework |
|------|---------------|------------------|
| `transitive_capability_leak` | `name.ends_with("Cap")` | 302 |
| `unused_capability_param_v2` | `name.contains("cap")` | 209 |
| `missing_access_control` | `name.ends_with("_cap")` | 181 |
| `flashloan_without_repay` | `name.contains("borrow")` | 40 |

**Total: 792 false positives on Sui framework alone.**

---

## The Problem

### Current Implementation (Broken)

```rust
fn is_capability_param(name: &str) -> bool {
    name.ends_with("_cap") || name.ends_with("Cap") || name.contains("capability")
}
```

This flags:
- `_treasury_cap` - intentionally unused for authorization-by-presence
- `Bag`, `Table` - have `key+store` but aren't capabilities
- `vector::borrow` - not a flashloan

### Required Fix (Type-Based)

```rust
fn is_capability_type(abilities: &AbilitySet) -> bool {
    abilities.has_ability_(Ability_::Key) 
        && abilities.has_ability_(Ability_::Store)
        && !abilities.has_ability_(Ability_::Copy)
        && !abilities.has_ability_(Ability_::Drop)
}
```

---

## Files Implemented

| File | Lines | Status |
|------|-------|--------|
| `src/absint_lints.rs` | 884 | Infrastructure ✅, Detection ❌ |
| `src/cross_module_lints.rs` | 628 | Infrastructure ✅, Detection ❌ |
| `src/semantic.rs` | 1900+ | Integration ✅, Lints ❌ |

---

## Next Steps

1. **Fix Detection Logic** - Replace all heuristics with type-based detection
2. **Validate on Framework** - Must produce 0 FPs on `sui-framework`
3. **Test on Ecosystem** - Run on real DeFi packages

See **Issue #24** for complete heuristic audit and **Issue #12** for roadmap.

---

## The Principle

> **A lint that uses name-based heuristics when type information is available is a broken lint.**

The infrastructure is solid. The detection logic needs to be rewritten to use the Move compiler's type system correctly.
