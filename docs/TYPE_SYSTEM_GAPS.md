# Type System Gaps Analysis

**Date**: 2025-12-15  
**Purpose**: Systematically identify type system gaps that could become lints  
**Status**: Initial implementation complete - 5 new lints added

## Overview

This document catalogs:
1. **Existing lints** classified by what type system gap they address
2. **Sui framework implicit contracts** - preconditions not enforced by types
3. **Uncovered gaps** - potential new lints

---

## TypeSystemGap Enum (Added to lint.rs)

```rust
pub enum TypeSystemGap {
    AbilityMismatch,     // Wrong ability combinations
    OwnershipViolation,  // Incorrect object ownership transitions
    CapabilityEscape,    // Capabilities leaking scope
    ValueFlow,           // Values going to wrong destinations
    ApiMisuse,           // Incorrect stdlib function usage
    TemporalOrdering,    // Operations in wrong sequence
    ArithmeticSafety,    // Numeric operations without validation
    StyleConvention,     // Style/convention issues (no security impact)
}
```

All security/suspicious lints now have a `gap: Option<TypeSystemGap>` field.

---

## Part 1: Existing Lint Classification by Gap Type

### Category: Ability Mismatch
*Wrong ability combinations or missing abilities*

| Lint | Source | Gap Explanation |
|------|--------|-----------------|
| `missing_key` | sui_mode::linters | `share_object<T: key>` requires key, but type definition site isn't checked |
| `droppable_hot_potato_v2` | semantic.rs | Hot potatoes shouldn't have `drop`, but compiler allows it |
| `missing_witness_drop` | security.rs | OTW types must have `drop` for `create_currency` |
| `event_emit_type_sanity` | semantic.rs | Events should be `copy + drop`, not `key` |

### Category: Ownership Violation  
*Incorrect object ownership transitions*

| Lint | Source | Gap Explanation |
|------|--------|-----------------|
| `share_owned` | sui_mode::linters | Sharing non-fresh objects fails at runtime |
| `share_owned_authority` | semantic.rs | Sharing `key+store` objects weakens access control |
| `self_transfer` | sui_mode::linters | Transfer to sender should be return instead |
| `freeze_wrapped` | sui_mode::linters | Freezing objects with wrapped children |
| `freezing_capability` | sui_mode::linters | Capabilities shouldn't be frozen |

### Category: Capability Escape
*Admin/sensitive capabilities leaking scope*

| Lint | Source | Gap Explanation |
|------|--------|-----------------|
| `capability_transfer_v2` | semantic.rs | Caps transferred outside defining module |
| `capability_leak` | security.rs (deprecated) | Superseded by type-based version |
| `transitive_capability_leak` | cross_module_lints.rs | Cross-module cap exposure |
| `phantom_capability` | absint_lints.rs | Unused capability references |

### Category: Value Flow
*Values going to wrong destinations*

| Lint | Source | Gap Explanation |
|------|--------|-----------------|
| `unused_return_value` | semantic.rs | Important returns discarded (e.g., split coins) |
| `ignored_boolean_return` | security.rs | Boolean success/failure ignored |
| `unchecked_coin_split` | security.rs (deprecated) | Split result not captured |
| `unchecked_withdrawal` | security.rs (deprecated) | Withdrawal result unchecked |

### Category: API Misuse
*Using stdlib functions incorrectly*

| Lint | Source | Gap Explanation |
|------|--------|-----------------|
| `coin_field` | sui_mode::linters | Use `Balance` not `Coin` in struct fields |
| `custom_state_change` | sui_mode::linters | Use private transfer/share variants |
| `public_random` | sui_mode::linters | Random state should be private |
| `public_random_access` | security.rs | Accessing random in public functions |
| `collection_equality` | sui_mode::linters | Don't compare collections with == |

### Category: Temporal Ordering
*Operations in wrong sequence*

| Lint | Source | Gap Explanation |
|------|--------|-----------------|
| `stale_oracle_price` | security.rs | Using oracle without freshness check |
| `flashloan_without_repay` | cross_module_lints.rs | Borrow without repay in same tx |
| `single_step_ownership_transfer` | security.rs | Admin transfer without acceptance |

### Category: Arithmetic Safety
*Numeric operations without validation*

| Lint | Source | Gap Explanation |
|------|--------|-----------------|
| `unchecked_division_v2` | absint_lints.rs | Division without zero check |
| `suspicious_overflow_check` | security.rs | Overflow check patterns |

---

## Part 2: Sui Framework Implicit Contracts (Not Yet Linted)

Based on framework analysis, these are **high-value gaps** not covered by existing lints:

### HIGH Priority - Strong Lintability

#### 1. `destroy_zero` on Possibly Non-Zero Balance/Coin ✅ IMPLEMENTED
**Lint**: `destroy_zero_unchecked` (Preview)  
**Module**: `sui::balance`, `sui::coin`  
**Gap**: `destroy_zero(balance)` assumes `balance.value == 0`, runtime aborts otherwise  
**Detection**: Dataflow analysis - warn if `destroy_zero` called without prior `== 0` check  
**Risk**: Silent fund loss if non-zero balance destroyed in error path

```move
// BAD - no check before destroy
public fun cleanup(b: Balance<SUI>) {
    balance::destroy_zero(b);  // WARN: may not be zero
}

// GOOD - explicit check
public fun cleanup(b: Balance<SUI>) {
    assert!(balance::value(&b) == 0, E_NOT_EMPTY);
    balance::destroy_zero(b);
}
```

#### 2. `fresh_object_address` Result Reused ✅ IMPLEMENTED
**Lint**: `fresh_address_reuse` (Preview)  
**Module**: `sui::tx_context`  
**Gap**: Each `fresh_object_address` should create exactly one UID  
**Detection**: Track variable usage - warn if result used multiple times  
**Risk**: Undefined behavior, potential object ID collision

```move
// BAD - reused address
let addr = tx_context::fresh_object_address(ctx);
let uid1 = object::new_uid_from_address(addr);
let uid2 = object::new_uid_from_address(addr);  // WARN: reuse

// GOOD - fresh address per UID  
let uid1 = object::new(ctx);
let uid2 = object::new(ctx);
```

#### 3. `tx_context::digest` Used as Randomness ✅ IMPLEMENTED
**Lint**: `digest_as_randomness` (Preview)  
**Module**: `sui::tx_context`  
**Gap**: Digest is predictable, should NOT be used for randomness  
**Detection**: Taint analysis - warn if `digest()` flows into security-sensitive ops  
**Risk**: Predictable "randomness" in lotteries, games, etc.

```move
// BAD - predictable randomness
let seed = tx_context::digest(ctx);
let winner = (*vector::borrow(seed, 0) as u64) % num_participants;  // WARN

// GOOD - use sui::random
let mut rng = random::new_generator(r, ctx);
let winner = random::generate_u64_in_range(&mut rng, 0, num_participants);
```

#### 4. `divide_into_n` with n=0 ✅ IMPLEMENTED
**Lint**: `divide_by_zero_literal` (Stable)  
**Module**: `sui::coin`  
**Gap**: `divide_into_n(coin, 0)` causes runtime abort  
**Detection**: Constant propagation - detect `n = 0` statically  
**Risk**: DoS if user controls n parameter

```move
// BAD - potential div by zero
public fun split_evenly(c: &mut Coin<SUI>, n: u64, ctx): vector<Coin<SUI>> {
    coin::divide_into_n(c, n, ctx)  // WARN if n could be 0
}
```

#### 5. OTW Pattern Violation in `create_currency` ✅ IMPLEMENTED
**Lint**: `otw_pattern_violation` (Preview)  
**Module**: `sui::coin`  
**Gap**: `create_currency` requires one-time-witness but type system doesn't enforce  
**Detection**: Type name must match module name pattern (e.g., `my_coin::MY_COIN`)  
**Risk**: Runtime abort if not OTW

```move
// BAD - not OTW pattern
module my_coin {
    struct Token has drop {}  // Wrong name!
    
    fun init(witness: Token, ctx: &mut TxContext) {
        coin::create_currency(witness, ...)  // WARN: not OTW
    }
}

// GOOD - OTW pattern
module my_coin {
    struct MY_COIN has drop {}  // Matches module name (uppercase)
    
    fun init(witness: MY_COIN, ctx: &mut TxContext) {
        coin::create_currency(witness, ...)
    }
}
```

### MEDIUM Priority - Moderate Lintability

#### 6. `object::delete` with Orphaned Dynamic Fields
**Module**: `sui::object`, `sui::dynamic_field`  
**Gap**: Deleting UID with attached dynamic fields = permanent data loss  
**Detection**: Track `dynamic_field::add` / `remove` pairs per UID  
**Complexity**: Requires interprocedural analysis

#### 7. Duplicate `dynamic_field::add`
**Module**: `sui::dynamic_field`  
**Gap**: Adding same field twice causes runtime abort  
**Detection**: Track field names per UID within function  
**Complexity**: Field names may be dynamic

#### 8. Transfer to Object Address
**Module**: `sui::transfer`  
**Gap**: `transfer(obj, id_address(other_obj))` makes obj inaccessible  
**Detection**: Warn when recipient comes from `object::id_address()`  
**Risk**: Permanent object loss

### LOW Priority - Hard to Lint Statically

- `split` amount > balance (requires value tracking)
- Supply overflow (extremely rare)
- `receive` from wrong parent (requires runtime ownership)

---

## Part 3: Gap Discovery Framework

### Systematic Approach to Finding New Gaps

1. **Enumerate Runtime Errors**
   - Find all `abort` codes in Sui framework
   - For each, ask: "Can we detect this statically?"

2. **Analyze Ability Requirements**
   - Find all `<T: ability>` constraints in public APIs
   - For each, ask: "Is this checked at definition site or only call site?"

3. **Map Data Flow Invariants**
   - Identify "must happen" patterns (e.g., split → capture result)
   - Identify "must not happen" patterns (e.g., share → non-fresh object)

4. **Review Audit Findings**
   - Each real-world exploit reveals a gap
   - Codify the pattern as a lint

### Gap Taxonomy

```
Type System Gaps
├── Ability Gaps
│   ├── Missing ability at definition (e.g., OTW needs drop)
│   └── Wrong ability combination (e.g., hot potato with drop)
├── Ownership Gaps  
│   ├── Provenance not tracked (fresh vs non-fresh)
│   └── Destination not validated (transfer to object addr)
├── Value Gaps
│   ├── Return values discarded
│   └── Zero checks missing
├── Temporal Gaps
│   ├── Ordering not enforced (check before use)
│   └── Pairing not enforced (borrow/repay)
└── API Contract Gaps
    ├── Implicit preconditions
    └── Semantic requirements (OTW pattern)
```

---

## Recommendations

### Immediate (Low-hanging fruit)
1. **`destroy_zero_unchecked`** - Warn on `destroy_zero` without prior check
2. **`otw_pattern_violation`** - Check OTW naming convention
3. **`digest_as_randomness`** - Taint analysis on `tx_context::digest`

### Medium-term (Requires dataflow)
4. **`fresh_address_reuse`** - Track `fresh_object_address` usage count
5. **`orphaned_dynamic_fields`** - Track add/remove pairs

### Long-term (Requires interprocedural)
6. **`transfer_to_object`** - Detect `id_address` in transfer recipient
7. **`duplicate_field_add`** - Track dynamic field names

---

## References

- [Sui Framework Source](https://github.com/MystenLabs/sui/tree/main/crates/sui-framework/packages/sui-framework/sources)
- [sui_mode::linters](https://github.com/MystenLabs/sui/tree/main/external-crates/move/crates/move-compiler/src/sui_mode/linters)
- [Move Prover Spec Language](https://github.com/move-language/move/blob/main/language/move-prover/doc/user/spec-lang.md)
