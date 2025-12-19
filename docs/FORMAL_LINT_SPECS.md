# Formal Lint Specifications

**Created:** 2025-12-18
**Purpose:** Formal specifications for type-based and CFG-based lints that enable mathematical verification of correctness.

**Status:** Design note / WIP specs (may drift)

## Overview

This document provides formal specifications for lints that can be mathematically verified. These specifications serve as:
1. **Ground truth** for testing - fixtures can be generated from the spec
2. **Documentation** of exact behavior - no ambiguity about what triggers/doesn't trigger
3. **Foundation for Move Prover integration** - specs can be translated to formal verification

---

## Tier 1: Type-Based Lints (Zero FP by Design)

### `droppable_hot_potato`

**Invariant:**
```
For struct S with abilities A and fields F:
  IF A = {drop} AND |F| > 0
  THEN WARN "broken hot potato"

Equivalently:
  has_drop(A) ∧ ¬has_copy(A) ∧ ¬has_key(A) ∧ ¬has_store(A) ∧ field_count(S) > 0
```

**Formal Definition:**
- `abilities(S) = {drop}` means S has ONLY the drop ability
- `|fields(S)| > 0` means S has at least one field (not empty)
- Empty structs (witnesses) are excluded: `|fields(S)| = 0`

**Why Zero FP:**
The ability set is compiler-verified. A struct with only `drop` and non-empty fields has no legitimate use case in Sui Move.

**Exhaustive Test Matrix (32 cases):**

Implemented in `tests/droppable_hot_potato.rs`. All 32 combinations tested:

| # | drop | copy | key | store | fields | Expected | Rationale |
|---|------|------|-----|-------|--------|----------|-----------|
| 0 | - | - | - | - | 0 | NO | Empty marker type |
| 1 | - | - | - | - | 1+ | NO | True hot potato (correct design) |
| 2 | ✓ | - | - | - | 0 | NO | Empty witness struct (OTW pattern) |
| **3** | **✓** | **-** | **-** | **-** | **1+** | **WARN** | **Broken hot potato** |
| 4 | - | ✓ | - | - | 0 | NO | Copy-only empty |
| 5 | - | ✓ | - | - | 1+ | NO | Copy-only with fields |
| 6 | - | - | ✓ | - | 0 | NO | Key-only empty |
| 7 | - | - | ✓ | - | 1+ | NO | Key-only with fields (object) |
| 8 | - | - | - | ✓ | 0 | NO | Store-only empty |
| 9 | - | - | - | ✓ | 1+ | NO | Store-only with fields |
| 10 | ✓ | ✓ | - | - | 0 | NO | Empty event |
| 11 | ✓ | ✓ | - | - | 1+ | NO | Event/DTO pattern |
| 12 | - | ✓ | ✓ | - | 0 | NO | Copy+key empty |
| 13 | - | ✓ | ✓ | - | 1+ | NO | Copyable object |
| 14 | - | ✓ | - | ✓ | 0 | NO | Copy+store empty |
| 15 | - | ✓ | - | ✓ | 1+ | NO | Config struct |
| 16 | ✓ | - | ✓ | - | 0 | NO | Drop+key empty |
| 17 | ✓ | - | ✓ | - | 1+ | NO | Droppable object |
| 18 | ✓ | - | - | ✓ | 0 | NO | Drop+store empty |
| 19 | ✓ | - | - | ✓ | 1+ | NO | Embeddable droppable |
| 20 | - | - | ✓ | ✓ | 0 | NO | Capability marker |
| 21 | - | - | ✓ | ✓ | 1+ | NO | Resource/capability |
| 22 | ✓ | ✓ | ✓ | - | 0 | NO | Copy+drop+key empty |
| 23 | ✓ | ✓ | ✓ | - | 1+ | NO | Freely usable object |
| 24 | ✓ | ✓ | - | ✓ | 0 | NO | Config empty |
| 25 | ✓ | ✓ | - | ✓ | 1+ | NO | Config with fields |
| 26 | - | ✓ | ✓ | ✓ | 0 | NO | Copy+key+store empty |
| 27 | - | ✓ | ✓ | ✓ | 1+ | NO | Copyable resource |
| 28 | ✓ | - | ✓ | ✓ | 0 | NO | Drop+key+store empty |
| 29 | ✓ | - | ✓ | ✓ | 1+ | NO | Droppable resource |
| 30 | ✓ | ✓ | ✓ | ✓ | 0 | NO | All abilities empty |
| 31 | ✓ | ✓ | ✓ | ✓ | 1+ | NO | All abilities with fields |

**Result:** Only case #3 triggers a warning, proving zero false positives by exhaustion.

**Implementation:** `tests/droppable_hot_potato.rs`
- `TEST_MATRIX`: Complete 32-case matrix with rationales
- `generate_fixture()`: Auto-generates Move code for each case
- `spec_droppable_hot_potato_exhaustive()`: Runs all 32 tests
- `spec_matrix_is_complete()`: Verifies matrix completeness

---

### `share_owned_authority`

**Invariant:**
```
For call share_object<T>(...) or public_share_object<T>(...):
  IF abilities(T) ⊇ {key, store}
  THEN WARN "sharing transferable authority"
```

**Formal Definition:**
- `abilities(T)` includes both `key` and `store`
- Called function is `transfer::share_object` or `transfer::public_share_object`

**Why Zero FP:**
Type abilities are compiler-verified. Objects with `key + store` can be transferred, making them "authority-like".

**Guard Patterns (should NOT fire):**
- `share_object<Pool>()` where `Pool has key` (no store) - Cannot be transferred
- `transfer<AdminCap>(cap, sender)` - Transfer, not share

**True Positive Patterns (should fire):**
- `share_object<AdminCap>()` where `AdminCap has key, store`
- `public_share_object<TreasuryCap<T>>()`

---

### `unused_return_value`

**Invariant:**
```
For call f(...) in statement position (not bound to variable):
  IF f ∈ IMPORTANT_FUNCTIONS AND return_type(f) ≠ unit
  THEN WARN "ignored return value"
```

**IMPORTANT_FUNCTIONS:**
```
{
  coin::split, coin::take,
  balance::split, balance::withdraw_all,
  option::extract, option::destroy_some,
  vector::pop_back,
  table::remove, bag::remove
}
```

**Why Low FP:**
These functions return values that represent assets or important state. Ignoring them is almost always a bug.

---

### `event_emit_type_sanity`

**Invariant:**
```
For call event::emit<T>(...):
  IF NOT (abilities(T) ⊇ {copy, drop} AND key ∉ abilities(T))
  THEN WARN "emitting non-event type"
```

**Event Type Definition:**
- Has `copy` and `drop` abilities
- Does NOT have `key` ability (key would make it an object)

---

## Tier 2: CFG-Based Lints (Preview, Near-Zero FP)

### `unchecked_division_v2`

**Invariant:**
```
For operation e1 / e2 or e1 % e2 at location L:
  LET D = divisor_variable(e2)
  REQUIRE: ∃ guard G that dominates L where G proves D ≠ 0
  
  IF NOT (guard_exists AND dominates(guard, L))
  THEN WARN "division without zero-check"
```

**Guard Patterns (prove D ≠ 0):**
```
- assert!(D != 0, _)
- assert!(D > 0, _)
- assert!(0 < D, _)
- if (D == 0) abort _
- if (D == 0) return _
- D is non-zero constant literal
- D is named constant (assumed non-zero)
```

**Domination Definition:**
Guard G dominates location L if:
- G is on ALL control flow paths from function entry to L
- G executes BEFORE L on every path

**Why Near-Zero FP:**
CFG analysis tracks domination precisely. Only false positives occur when guards are in complex patterns not recognized.

---

### `destroy_zero_unchecked_v2`

**Invariant:**
```
For call balance::destroy_zero(B) or coin::destroy_zero(C) at location L:
  LET V = value_variable(B or C)
  REQUIRE: ∃ guard G that dominates L where G proves V == 0
  
  IF NOT (guard_exists AND dominates(guard, L))
  THEN WARN "destroy_zero without zero-value check"
```

**Guard Patterns (prove V == 0):**
```
- assert!(value(&V) == 0, _)
- assert!(balance::value(&V) == 0, _)
- if (value(&V) != 0) abort _
- V is result of balance::zero() or coin::zero()
```

---

### `fresh_address_reuse_v2`

**Invariant:**
```
For variable A assigned from fresh_object_address(ctx):
  COUNT uses(A) where use is new_uid_from_address(A)
  
  IF uses(A) > 1
  THEN WARN "fresh address used multiple times"
```

**Why This Matters:**
Each UID must have a unique address. Reusing a fresh address violates object uniqueness.

---

## Testing Strategy

### For Type-Based Lints (Formal Spec)

1. **Enumerate all ability combinations** from spec
2. **Generate fixtures** for each combination
3. **Assert exact match** between spec and lint behavior

Example test matrix for `droppable_hot_potato`:
```
| Abilities      | Fields | Expected |
|----------------|--------|----------|
| {drop}         | 0      | NO WARN  |  (witness)
| {drop}         | 1+     | WARN     |  (broken hot potato)
| {copy, drop}   | any    | NO WARN  |  (DTO)
| {key, store}   | any    | NO WARN  |  (resource)
| {}             | any    | NO WARN  |  (true hot potato)
```

### For CFG-Based Lints (Mutation Testing)

1. Start with **known-correct code** (passes spec)
2. **Mutate** to create known-incorrect code
3. Assert lint **catches all mutations**
4. Assert lint **doesn't fire on original**

Example mutations for `unchecked_division_v2`:
```move
// ORIGINAL (correct)
fun safe_div(a: u64, b: u64): u64 {
    assert!(b > 0, 1);
    a / b
}

// MUTATION 1: Remove guard
fun mutant_1(a: u64, b: u64): u64 {
    a / b  // MUST WARN
}

// MUTATION 2: Guard after division
fun mutant_2(a: u64, b: u64): u64 {
    let r = a / b;  // MUST WARN
    assert!(b > 0, 1);
    r
}

// MUTATION 3: Guard checks wrong variable
fun mutant_3(a: u64, b: u64): u64 {
    assert!(a > 0, 1);  // Wrong variable!
    a / b  // MUST WARN
}
```

---

### `copyable_capability`

**Invariant:**
```
For struct S with abilities A:
  IF A ⊇ {key, store, copy}
  THEN WARN "capability can be duplicated"
```

**Formal Definition:**
- `abilities(S)` includes `key` (is an object)
- `abilities(S)` includes `store` (can be transferred)
- `abilities(S)` includes `copy` (can be duplicated) - THIS IS THE BUG

**Why Zero FP:**
A capability with `copy` defeats access control because anyone can duplicate it. The combination `key + store + copy` is never correct for an authority object.

**Test Matrix (16 cases):**
Implemented in `tests/copyable_capability_spec.rs`.

| # | key | store | copy | drop | Expected | Rationale |
|---|-----|-------|------|------|----------|-----------|
| 0 | - | - | - | - | NO | No abilities |
| 1 | ✓ | - | - | - | NO | Key only |
| 2 | - | ✓ | - | - | NO | Store only |
| 3 | - | - | ✓ | - | NO | Copy only |
| 4 | - | - | - | ✓ | NO | Drop only |
| 5 | ✓ | ✓ | - | - | NO | Proper capability |
| 6 | ✓ | - | ✓ | - | NO | Missing store |
| 7 | ✓ | - | - | ✓ | NO | Missing store |
| 8 | - | ✓ | ✓ | - | NO | Missing key |
| 9 | - | ✓ | - | ✓ | NO | Missing key |
| 10 | - | - | ✓ | ✓ | NO | Event pattern |
| **11** | **✓** | **✓** | **✓** | **-** | **WARN** | **Copyable capability** |
| 12 | ✓ | ✓ | - | ✓ | NO | Droppable capability |
| 13 | ✓ | - | ✓ | ✓ | NO | Missing store |
| 14 | - | ✓ | ✓ | ✓ | NO | Missing key |
| **15** | **✓** | **✓** | **✓** | **✓** | **WARN** | **Copyable capability** |

**Result:** Only cases #11 and #15 trigger a warning (key+store+copy variants).

---

### `droppable_capability`

**Invariant:**
```
For struct S with abilities A:
  IF A ⊇ {key, store, drop} AND copy ∉ A
  THEN WARN "capability can be silently discarded"
```

**Formal Definition:**
- `abilities(S)` includes `key` (is an object)
- `abilities(S)` includes `store` (can be transferred)
- `abilities(S)` includes `drop` (can be silently discarded) - THIS IS THE BUG
- `abilities(S)` does NOT include `copy` (handled by `copyable_capability`)

**Why Zero FP:**
A capability with `drop` defeats the explicit handling guarantee. Admin capabilities can be "lost" to evade audit trails, and the "must handle" property is broken.

**Security References:**
- Mirage Audits (2025): "The Ability Mistakes That Will Drain Your Sui Move Protocol"
  <https://www.mirageaudits.com/blog/sui-move-ability-security-mistakes>
- SlowMist (2024): "Sui Move Smart Contract Auditing Primer"
  <https://github.com/slowmist/Sui-MOVE-Smart-Contract-Auditing-Primer>

**Test Matrix (16 cases):**
Implemented in `tests/droppable_capability_spec.rs`.

| # | key | store | copy | drop | Expected | Rationale |
|---|-----|-------|------|------|----------|-----------|
| 0 | - | - | - | - | NO | No abilities |
| 1 | ✓ | - | - | - | NO | Key only |
| 2 | - | ✓ | - | - | NO | Store only |
| 3 | - | - | ✓ | - | NO | Copy only |
| 4 | - | - | - | ✓ | NO | Drop only (hot potato) |
| 5 | ✓ | ✓ | - | - | NO | **Proper capability** |
| 6 | ✓ | - | ✓ | - | NO | Missing store |
| 7 | ✓ | - | - | ✓ | NO | Missing store |
| 8 | - | ✓ | ✓ | - | NO | Missing key |
| 9 | - | ✓ | - | ✓ | NO | Missing key |
| 10 | - | - | ✓ | ✓ | NO | Event pattern |
| 11 | ✓ | ✓ | ✓ | - | NO | Handled by copyable_capability |
| **12** | **✓** | **✓** | **-** | **✓** | **WARN** | **Droppable capability** |
| 13 | ✓ | - | ✓ | ✓ | NO | Missing store |
| 14 | - | ✓ | ✓ | ✓ | NO | Missing key |
| 15 | ✓ | ✓ | ✓ | ✓ | NO | Handled by copyable_capability |

**Result:** Only case #12 triggers a warning (key+store+drop without copy).

**Relationship to `copyable_capability`:**
These two lints are mutually exclusive:
- `copyable_capability`: fires when `copy` is present (key+store+copy)
- `droppable_capability`: fires when `drop` is present AND `copy` is absent (key+store+drop)

Together they enforce the rule: **proper capabilities have only `key + store`**.

---

### `non_transferable_fungible_object`

**Invariant:**
```
For struct S with abilities A:
  IF key ∈ A AND store ∉ A AND (copy ∈ A OR drop ∈ A)
  THEN WARN "non-transferable object with fungible abilities"
```

**Formal Definition:**
- `abilities(S)` includes `key` (is an object)
- `abilities(S)` does NOT include `store` (non-transferable/soulbound)
- `abilities(S)` includes `copy` OR `drop` (fungible) - THIS IS THE INCOHERENCE

**Why Zero FP:**
An object without `store` is intentionally non-transferable (soulbound). Adding `copy` or `drop` to such an object creates an incoherent design:
- If you want fungibility, add `store` to enable transfer
- If you want soulbound semantics, remove `copy` and `drop`

**First Principles Reasoning:**
This lint is derived from exhaustive analysis of all 16 ability combinations:
- `{key}` alone = legitimate soulbound object
- `{key, store}` = legitimate transferable object
- `{key, drop}` = INCOHERENT (can drop but not transfer)
- `{key, copy}` = INCOHERENT (can copy but not transfer)
- `{key, copy, drop}` = INCOHERENT (fully fungible but not transferable)

**Test Matrix (16 cases):**
Implemented in `tests/non_transferable_fungible_object_spec.rs`.

| # | key | store | copy | drop | Expected | Rationale |
|---|-----|-------|------|------|----------|-----------|
| 0 | - | - | - | - | NO | Hot potato |
| 1 | ✓ | - | - | - | NO | **Legitimate soulbound** |
| 2 | - | ✓ | - | - | NO | Embeddable |
| 3 | - | - | ✓ | - | NO | Not an object |
| 4 | - | - | - | ✓ | NO | Droppable hot potato |
| 5 | ✓ | ✓ | - | - | NO | **Legitimate capability** |
| **6** | **✓** | **-** | **✓** | **-** | **WARN** | **Copyable non-transferable** |
| **7** | **✓** | **-** | **-** | **✓** | **WARN** | **Droppable non-transferable** |
| 8 | - | ✓ | ✓ | - | NO | Embeddable copyable |
| 9 | - | ✓ | - | ✓ | NO | Embeddable droppable |
| 10 | - | - | ✓ | ✓ | NO | Event/DTO |
| 11 | ✓ | ✓ | ✓ | - | NO | Handled by copyable_capability |
| 12 | ✓ | ✓ | - | ✓ | NO | Handled by droppable_capability |
| **13** | **✓** | **-** | **✓** | **✓** | **WARN** | **Fully fungible non-transferable** |
| 14 | - | ✓ | ✓ | ✓ | NO | Value struct |
| 15 | ✓ | ✓ | ✓ | ✓ | NO | Handled by copyable_capability |

**Result:** Cases #6, #7, and #13 trigger a warning (key without store, with copy or drop).

**Relationship to other lints:**
This lint is disjoint from `copyable_capability` and `droppable_capability`:
- Those require `store` (transferable capabilities)
- This requires NO `store` (non-transferable objects)

---

### `storable_hot_potato`

**Invariant:**
```
For struct S with abilities A and fields F:
  IF A = {store} AND |F| > 0 AND S.name ≠ "UID"
  THEN WARN "hot potato can be stored"

Equivalently:
  has_store(A) ∧ ¬has_key(A) ∧ ¬has_copy(A) ∧ ¬has_drop(A) ∧ field_count(S) > 0 ∧ name(S) ≠ "UID"
```

**Formal Definition:**
- `abilities(S) = {store}` means S has ONLY the store ability
- `|fields(S)| > 0` means S has at least one field
- `name(S) ≠ "UID"` excludes the Sui framework's UID type

**Why Experimental (Medium FP):**
Structs with only `store` have legitimate uses as embedded/wrapper types. The lint fires on these patterns, making it too noisy for Stable tier.

**Exclusions:**
- `UID` - The Sui framework's UID type legitimately has only `store`
- Empty structs - Marker types with 0 fields

**Status:** Experimental - requires `--experimental` flag

---

## Move Prover Integration (Future)

For CFG-based lints, we can use Move Prover as ground truth:

```move
fun divide(a: u64, b: u64): u64 {
    assert!(b > 0, 1);
    a / b
}
spec divide {
    aborts_if b == 0;
}
```

**Differential Testing:**
1. Run Move Prover on fixture
2. Run move-clippy lint on same fixture
3. If Prover says "safe" but lint fires → **False Positive**
4. If Prover says "unsafe" but lint doesn't fire → **False Negative**

This provides mathematical confidence in lint correctness.
