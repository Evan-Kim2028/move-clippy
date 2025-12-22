# Lint Grounding Analysis: From Heuristics to Type System

**Goal:** Evaluate each Tier 2/3 lint for groundability in Move's type system.

## What Move's Type System Gives Us

### Definitive Type Information
1. **Abilities:** `key`, `store`, `copy`, `drop` - queryable per type
2. **Struct fields:** Full type information including nested types
3. **Function signatures:** Parameter types, return types, visibility
4. **Module identity:** Full path `address::module::Type`
5. **Generic constraints:** Ability requirements on type parameters

### What We Can Query at Compile Time
- Is this type a `Coin<T>`? → **Yes** (exact module::struct match)
- Does this type have `key + store`? → **Yes** (ability query)
- Is this function `public`? → **Yes** (visibility attribute)
- Is this a call to `transfer::share_object`? → **Yes** (exact function match)
- Does this parameter come from `tx_context::sender()`? → **Yes** (dataflow via CFG)

### What We CANNOT Query
- "Is this a capability?" → **No canonical definition** (it's a convention)
- "Is this a hot potato?" → **No canonical definition** (it's a convention)
- "Is this an admin function?" → **No canonical definition**
- "Should this value be checked?" → **Requires semantic understanding**

---

## Lint-by-Lint Analysis

### Tier 2: Low FP Risk

#### `droppable_hot_potato` - CANNOT GROUND
**Current:** Name heuristics (`receipt`, `ticket`, `promise`) + abilities check

**Problem:** "Hot potato" is a *design pattern*, not a type system concept. There's no
ability combination that uniquely identifies hot potatoes:
- Hot potato: no abilities (must be consumed)
- But many other types have no abilities (e.g., wrapper types)

**Verdict:** DEPRECATE or RESHAPE
- **Option A:** Deprecate - pattern is too fuzzy
- **Option B:** Reshape to `struct_with_no_abilities_not_consumed` - warns when a
  no-ability struct is created but not unpacked/consumed in the same function
  (requires CFG analysis, but is groundable in dataflow)

---

#### `shared_capability` - PARTIALLY GROUNDABLE
**Current:** Name heuristics (`Cap` suffix) + `share_object()` call

**What's groundable:**
- Detecting `share_object()` call → **100% accurate**
- Type has `key` ability → **100% accurate**

**What's NOT groundable:**
- "Is this a capability?" → Convention, not type system

**Verdict:** RESHAPE to `share_key_store_object`
- Warn when `share_object()` is called on a type with `key + store` abilities
- Rationale: Types with `key + store` are "ownable" - sharing them is often wrong
- This catches capability sharing but also other problematic patterns
- **Zero heuristics, pure type system**

---

#### `stale_oracle_price` - FULLY GROUNDABLE ✓
**Current:** Exact function name `get_price_unsafe`

**Analysis:** This is already groundable - it's detecting a specific API call.
The function is named "unsafe" by the Pyth team specifically to warn users.

**Verdict:** KEEP AS-IS
- Exact function match is not a heuristic, it's API knowledge
- Consider expanding to other oracle providers' unsafe functions

---

#### `single_step_ownership_transfer` - CANNOT GROUND
**Current:** Function name heuristics (`transfer_admin`, `set_owner`)

**Problem:** "Ownership transfer" is a design pattern with no type system representation.
There's no way to know if a function is "transferring ownership" without semantic understanding.

**Verdict:** DEPRECATE
- Cannot be made rigorous
- Alternative: Document the two-step pattern in style guide instead

---

#### `missing_witness_drop` - FULLY GROUNDABLE ✓
**Current:** OTW pattern (uppercase module name struct) + missing `drop` ability

**Analysis:** The One-Time Witness pattern IS type-system groundable:
1. Struct name == uppercase(module_name)
2. Struct has 0 fields
3. First parameter of `init` function

**Verdict:** KEEP BUT REFINE
- Ground in actual OTW detection (struct name matches module, 0 fields)
- This is convention-based but the convention is structurally verifiable

---

#### `public_random_access` - FULLY GROUNDABLE ✓
**Current:** Parameter type is `Random` or `&Random` + function is `public`

**Analysis:** Already type-system grounded:
1. Type check: parameter is `sui::random::Random`
2. Visibility check: function is `public`

**Verdict:** KEEP AS-IS - already rigorous

---

#### `suspicious_overflow_check` - PARTIALLY GROUNDABLE
**Current:** Function name + bit shifts + comparisons

**What's groundable:**
- Presence of bit shift operators → AST pattern
- Presence of comparison operators → AST pattern
- Hex constants → AST pattern

**What's NOT groundable:**
- "Is this an overflow check function?" → Name heuristic

**Verdict:** RESHAPE to `manual_bit_shift_bounds_check`
- Warn on ANY function that has (bit shifts + hex constants + comparisons)
- Remove the name requirement
- Rationale: This pattern is risky regardless of function name
- User can suppress if intentional

---

#### `ignored_boolean_return` - FULLY GROUNDABLE ✓
**Current:** Function returns `bool` + result is discarded

**Analysis:** Already type-system grounded:
1. Return type analysis: function returns `bool`
2. Result tracking: return value not bound

**Verdict:** KEEP AS-IS - already rigorous

---

#### `shared_capability_object` - SAME AS `shared_capability`
**Verdict:** MERGE with reshaped `share_key_store_object`

---

### Tier 3: Preview Lints

#### `unused_capability_param_v2` - NEEDS WORK
**Current:** Name heuristics for "capability" + CFG-based usage tracking

**What's groundable:**
- CFG-based "is this parameter used?" → **Fully groundable**
- Type abilities check → **Fully groundable**

**What's NOT groundable:**
- "Is this a capability?" → Name heuristic

**Verdict:** RESHAPE to `unused_key_store_param`
- Warn when a parameter with `key + store` abilities is never used
- Rationale: `key + store` types represent authority - unused authority params are suspicious
- **Zero heuristics**

---

#### `unchecked_division_v2` - FULLY GROUNDABLE ✓
**Current:** Division operation + divisor not validated

**Analysis:** This IS groundable through dataflow:
1. Detect division operation → AST
2. Track divisor through CFG
3. Check if divisor is validated (compared to 0, asserted non-zero)

**Problem:** Current implementation has limited guard pattern detection

**Verdict:** KEEP BUT FIX
- Improve CFG-based guard detection
- This is a legitimate lint with a real type-system basis

---

#### `unchecked_division` (non-CFG version) - DEPRECATE
**Current:** Simple pattern matching

**Verdict:** DEPRECATE in favor of CFG-aware version

---

#### `unused_return_value` - FULLY GROUNDABLE ✓
**Current:** Function call + return type is non-unit + result discarded

**Analysis:** Groundable:
1. Return type analysis
2. Result binding analysis

**Verdict:** KEEP AS-IS - already rigorous

---

#### `unchecked_coin_split` - PARTIALLY GROUNDABLE
**Current:** `coin::split` call detection

**What's groundable:**
- Detecting `coin::split` call → **Exact function match**
- Return value is `Coin<T>` → **Type check**

**What's NOT groundable:**
- "Was balance checked before split?" → Requires dataflow

**Verdict:** RESHAPE to `coin_split_return_discarded`
- Warn when `coin::split` return value is not captured
- This is the real bug - discarding split coins
- **Groundable in type system + result tracking**

---

#### `unchecked_withdrawal` - CANNOT GROUND
**Current:** Name heuristics (`withdraw`, `remove`)

**Verdict:** DEPRECATE
- "Withdrawal" is not a type system concept
- No way to rigorously define what needs checking

---

#### `capability_leak` - CANNOT GROUND
**Current:** Name heuristics (`Cap`) + `transfer` call

**Verdict:** DEPRECATE
- Same problem as `shared_capability`
- Reshaped version covered by `share_key_store_object`

---

#### `pure_function_transfer` - FULLY GROUNDABLE ✓
**Current:** `transfer::*` call in non-entry function

**Analysis:** Groundable:
1. Function visibility: not `entry`
2. Call detection: `transfer::transfer`, `transfer::share_object`, etc.

**Verdict:** KEEP AS-IS
- Already rigorous
- Promote to stable after validation

---

#### `unsafe_arithmetic` - CANNOT GROUND USEFULLY
**Current:** Presence of `+`, `-`, `*` operators

**Problem:** Nearly every function has arithmetic. Without dataflow analysis
of value ranges, this is meaningless noise.

**Verdict:** DEPRECATE
- Too noisy to be useful
- Future: Consider integration with formal verification tools

---

#### `transitive_capability_leak` - PARTIALLY GROUNDABLE
**Current:** Call graph + type abilities

**What's groundable:**
- Call graph construction → **Compiler infrastructure**
- Type abilities → **Type system**

**What's NOT groundable:**
- "Is this a capability?" → Name heuristic

**Verdict:** RESHAPE to `transitive_key_store_leak`
- Track `key + store` types through call graph
- Warn if they flow to public APIs without ownership transfer

---

#### `flashloan_without_repay` - PARTIALLY GROUNDABLE
**Current:** Call graph + name heuristics for "loan"/"repay"

**What's groundable:**
- No-ability struct creation → Type abilities
- Struct consumption tracking → CFG

**Verdict:** RESHAPE to `unconsumed_no_ability_struct`
- Warn when a struct with no abilities is created but not consumed
- This is the actual flash loan enforcement mechanism
- **Groundable in type abilities + dataflow**

---

## Summary: Recommended Actions

### KEEP (Already Rigorous)
| Lint | Basis |
|------|-------|
| `stale_oracle_price` | Exact API match |
| `missing_witness_drop` | Structural OTW pattern |
| `public_random_access` | Type + visibility |
| `ignored_boolean_return` | Return type + binding |
| `unchecked_division_v2` | CFG dataflow (needs improvement) |
| `unused_return_value` | Return type + binding |
| `pure_function_transfer` | Call + visibility |

### RESHAPE (Can Be Grounded)
| Old Lint | New Lint | Grounding |
|----------|----------|-----------|
| `shared_capability` | `share_key_store_object` | Type abilities + call |
| `shared_capability_object` | (merge above) | - |
| `unused_capability_param_v2` | `unused_key_store_param` | Type abilities + CFG |
| `suspicious_overflow_check` | `manual_bit_shift_bounds_check` | AST patterns only |
| `unchecked_coin_split` | `coin_split_return_discarded` | Call + result binding |
| `transitive_capability_leak` | `transitive_key_store_leak` | Call graph + abilities |
| `flashloan_without_repay` | `unconsumed_no_ability_struct` | Abilities + CFG |
| `droppable_hot_potato` | `unconsumed_no_ability_struct` | (same as above) |

### DEPRECATE (Cannot Be Grounded)
| Lint | Reason |
|------|--------|
| `single_step_ownership_transfer` | "Ownership" is semantic, not syntactic |
| `unchecked_division` (non-CFG) | Superseded by CFG version |
| `unchecked_withdrawal` | "Withdrawal" not type-system defined |
| `capability_leak` | "Capability" not type-system defined |
| `unsafe_arithmetic` | Too noisy without range analysis |

---

## Key Insight: The `key + store` Proxy

Many "capability" lints can be reshaped around a groundable proxy:

> **Types with `key + store` abilities represent transferable authority.**

This is because:
1. `key` = can be an object (has identity)
2. `store` = can be transferred to other objects/addresses

Together, they define "ownable authority" - which is what capabilities ARE in Sui Move.

This gives us a type-system grounded definition that captures:
- AdminCap, MintCap, etc. (true capabilities)
- TreasuryCap (coin minting authority)
- Publisher (package authority)
- UpgradeCap (upgrade authority)

And appropriately flags:
- Sharing authority objects (dangerous)
- Unused authority parameters (suspicious)
- Authority leaking through public APIs (dangerous)
