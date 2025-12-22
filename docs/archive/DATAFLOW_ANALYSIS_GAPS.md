# Dataflow Analysis Gaps and Improvement Paths

**Created:** 2024-12-14
**Updated:** 2024-12-14 (Phase 1 & 2 Implemented)
**Purpose:** Technical analysis of move-clippy's dataflow capabilities vs. Clippy-level quality

## Executive Summary

Move-clippy uses the Move compiler's `SimpleAbsInt` framework for CFG-aware analysis.

### Implementation Status

| Phase | Description | Status |
|-------|-------------|--------|
| **Phase 1** | Type-based sink detection + Guard pattern recognition | ✅ **IMPLEMENTED** |
| **Phase 2** | Rich 4-state value tracking | ✅ **IMPLEMENTED** |
| **Phase 3** | Inter-procedural analysis | ❌ Not yet implemented |

### What's Now Implemented (Phase 1 & 2)

1. **Rich value tracking** - 4-state lattice: `Unused → AccessedNotValidated → PendingValidation → Validated`
2. **Guard pattern detection** - `JumpIf` commands (compiled `assert!`/`if`) are recognized
3. **Type-based sink detection** - Replaces name-based heuristics with ability checks
4. **Validation state survives Move** - `cap_validated` map tracks validation across ownership transfers

### Remaining Gap (Phase 3)

- **No inter-procedural analysis** - We don't follow capabilities into called functions

---

## Current Architecture (Post Phase 1 & 2)

### CapValue Lattice

```rust
pub enum CapValue {
    /// Capability parameter never accessed
    Unused,
    /// Capability was accessed (field read, passed to function) but result
    /// was not used in a guard condition
    AccessedNotValidated(Loc),
    /// A validation check was computed (cap.id == x) but hasn't flowed
    /// into a guard (assert/if) yet
    PendingValidation(Loc),
    /// Capability was validated through a guard (assert!/if condition)
    Validated(Loc),
}
```

### Key Components

1. **`exp_custom`**: Tracks capability accesses and comparison expressions
2. **`command_custom`**: Detects `JumpIf` guard patterns
3. **`call_custom`**: Type-based privileged sink detection
4. **`finish`**: Reports both "unused" and "accessed but not validated" warnings

### Type-Based Sink Detection

Instead of name-based heuristics, we now check:
- Known transfer functions (transfer, public_transfer, share_object, etc.)
- &mut references to value-bearing resources (key+store with numeric/Balance fields)
- Return types that extract value-bearing resources

---

## Gap Analysis

### Gap 1: No Guard Pattern Recognition

**Current behavior:**
```move
public fun withdraw(cap: &AdminCap, pool: &mut Pool, amount: u64) {
    // This check is invisible to our analysis:
    assert!(cap.pool_id == object::id(pool), E_WRONG_CAP);
    pool.withdraw(amount)
}
```

We see:
1. `cap` is borrowed → mark as Used
2. Function calls `pool.withdraw` → privileged sink
3. `cap` is Used → no warning ✓

But also:
```move
public fun withdraw(cap: &AdminCap, pool: &mut Pool, amount: u64) {
    // Cap is read but result discarded - SHOULD WARN
    cap.pool_id;  // Dead read
    pool.withdraw(amount)
}
```

We see the same thing → no warning ✗

**What Clippy does:** Tracks that the *result* of `cap.pool_id` flows into `assert!`, not just that `cap` was accessed.

### Gap 2: Boolean Flow Tracking

**Current behavior:**
```move
public fun is_authorized(cap: &AdminCap, ctx: &TxContext): bool {
    cap.owner == tx_context::sender(ctx)
}

public fun withdraw(cap: &AdminCap, pool: &mut Pool, ctx: &TxContext) {
    let authorized = is_authorized(cap, ctx);  // Bool result
    // We don't track that `authorized` contains the cap validation result
    pool.withdraw(amount)  // Should warn - authorized not checked!
}
```

We don't warn because `cap` was "used" (passed to `is_authorized`).

**What's needed:** Track that `authorized` holds a "pending validation" that must flow into a guard.

### Gap 3: Inter-procedural Analysis

**Current behavior:**
```move
module validator {
    public fun validate_cap(cap: &AdminCap, expected: address): bool {
        cap.pool_id == expected
    }
}

module pool {
    public fun withdraw(cap: &AdminCap, pool: &mut Pool) {
        // We don't know that validate_cap is a "validation" function
        assert!(validator::validate_cap(cap, pool.id), E_WRONG_CAP);
        pool.do_withdraw()
    }
}
```

We can't recognize that `validator::validate_cap` is doing the capability validation.

**What's needed:** Either:
1. Annotations: `#[validates(cap)]` on functions
2. Inference: Track that function returns bool and reads capability fields
3. Summaries: Pre-compute function effects

### Gap 4: Name-Based Sink Detection

**Current code:**
```rust
fn is_privileged_sink_call(&self, call: &ModuleCall) -> bool {
    let module_name = call.module.value.module.value().as_str();
    let fn_name = call.name.value().as_str();
    
    // Heuristic list
    matches!(
        (module_name, fn_name),
        ("balance", "withdraw_all")
        | ("balance", "split")
        | ("coin", "take")
        // ... more patterns
    )
}
```

**Problem:** New protocols with different naming won't be detected.

**What's needed:** Type-based detection - any function that:
- Takes a mutable reference to a resource with value
- Returns a resource (potential extraction)
- Modifies global state

---

## Improvement Paths

### Path 1: Rich Value Tracking (Medium effort, High impact)

Replace binary `CapValue` with rich tracking:

```rust
pub enum CapValue {
    /// Never accessed
    Unused,
    /// Accessed but result not used in guard
    AccessedNotValidated(Loc),
    /// Validation result pending (must flow to guard)
    PendingValidation(Loc),
    /// Fully validated (flowed through assert/if guard)
    Validated(Loc),
}
```

Implementation:
1. In `exp_custom`: When we see `cap.field` or `cap == x`, return `PendingValidation`
2. In `call_custom`: When `assert!` or `if` condition uses a `PendingValidation`, upgrade to `Validated`
3. In `finish`: Warn if capability is `AccessedNotValidated` but function has privileged sink

### Path 2: Guard Pattern Detection (Medium effort, High impact)

Track which expressions flow into conditional guards:

```rust
fn command_custom(&self, ctx: &mut ExecutionContext, state: &mut State, cmd: &Command) -> bool {
    match &cmd.value {
        Command_::JumpIf { cond, .. } => {
            // Mark variables used in `cond` as validated
            self.mark_validated_in_expr(state, cond);
        }
        _ => {}
    }
    false
}
```

This handles:
```move
if (cap.owner == sender) { ... }  // cap is validated
assert!(cap.pool_id == pool.id, E_WRONG);  // cap is validated
```

### Path 3: Type-Based Sink Detection (Low effort, Medium impact)

Replace name-based heuristics with type queries:

```rust
fn is_privileged_sink_call(&self, call: &ModuleCall, info: &TypingProgramInfo) -> bool {
    // Check if any parameter is &mut Resource where Resource has value
    for (_, _, param_ty) in &call.signature.parameters {
        if is_mutable_value_resource(param_ty, info) {
            return true;
        }
    }
    
    // Check if return type is a value-bearing resource
    if is_value_resource(&call.signature.return_type, info) {
        return true;
    }
    
    false
}

fn is_mutable_value_resource(ty: &Type, info: &TypingProgramInfo) -> bool {
    // &mut T where T has key+store and non-zero value fields
    if let Type_::Ref(true, inner) = &ty.value {
        if let Some(struct_info) = get_struct_info(inner, info) {
            return struct_info.has_value_fields();
        }
    }
    false
}
```

### Path 4: Function Summaries (High effort, Very high impact)

Pre-compute per-function summaries:

```rust
struct FunctionSummary {
    /// Parameters that must be validated for safe execution
    requires_validation: Vec<ParamIndex>,
    /// Whether this function validates a parameter
    validates_param: Option<ParamIndex>,
    /// Whether this function is a privileged sink
    is_privileged_sink: bool,
    /// Return value depends on parameter validation
    returns_validation_of: Option<ParamIndex>,
}
```

Then during analysis:
```rust
fn call_custom(&self, ..., f: &ModuleCall, args: Vec<CapValue>) -> Option<Vec<CapValue>> {
    let summary = self.get_summary(f);
    
    if summary.validates_param.is_some() {
        // Mark that param as validated
    }
    
    if let Some(param_idx) = summary.returns_validation_of {
        // Return value is a validation check
        return Some(vec![CapValue::PendingValidation(...)]);
    }
    
    // ...
}
```

---

## Recommended Implementation Order

### Phase 1: Quick Wins (1-2 days)

1. **Type-based sink detection** - Replace name heuristics with type queries
2. **Guard pattern detection** - Track `assert!` and `if` conditions

Expected impact: ~50% reduction in false negatives

### Phase 2: Rich Tracking (3-5 days)

1. **PendingValidation state** - Track validation results
2. **Flow-through guards** - Upgrade pending → validated when used in guard
3. **Improved error messages** - Show what validation was expected

Expected impact: Clippy-level precision for single-function analysis

### Phase 3: Inter-procedural (1-2 weeks)

1. **Function summaries** - Pre-compute validation effects
2. **Cross-module tracking** - Follow capabilities across module boundaries
3. **Annotation support** - `#[validates]` attribute

Expected impact: Full Clippy-level analysis

---

## Comparison with Sui's share_owned Lint

Sui's `share_owned.rs` demonstrates good patterns we should adopt:

| Feature | share_owned | Our lints | Gap |
|---------|-------------|-----------|-----|
| Value enum | `FreshObj`, `NotFreshObj`, `Other` | `Used`, `Unused` | Richer states |
| Pack tracking | ✓ Returns `FreshObj` for Pack | ✗ | Need to track object creation |
| Call returns | ✓ Type-based (`is_obj_type`) | ✗ Name heuristics | Need type queries |
| Guard detection | N/A (not needed for this lint) | ✗ | Need for cap validation |

---

## Test Plan for Improvements

For each improvement, add tests:

```move
// Test: Validation through guard
#[test]
fun test_guard_validation() {
    // Should NOT warn - cap validated in assert
    public fun withdraw(cap: &AdminCap, pool: &mut Pool) {
        assert!(cap.pool_id == object::id(pool), E_WRONG);
        pool.withdraw()
    }
}

// Test: Validation through if
#[test]  
fun test_if_validation() {
    // Should NOT warn - cap validated in if condition
    public fun withdraw(cap: &AdminCap, pool: &mut Pool) {
        if (cap.pool_id == object::id(pool)) {
            pool.withdraw()
        }
    }
}

// Test: Dead read (should warn)
#[test]
fun test_dead_read_warns() {
    // SHOULD warn - cap read but result discarded
    public fun withdraw(cap: &AdminCap, pool: &mut Pool) {
        cap.pool_id;  // Dead read
        pool.withdraw()  
    }
}

// Test: Validation result discarded (should warn)
#[test]
fun test_discarded_validation() {
    // SHOULD warn - validation computed but not used
    public fun withdraw(cap: &AdminCap, pool: &mut Pool) {
        let valid = cap.pool_id == object::id(pool);
        // valid not checked!
        pool.withdraw()
    }
}
```

---

## Conclusion

Our dataflow analysis is functional but weak. The main gaps are:

1. **Binary state tracking** - Need richer abstract values
2. **No guard detection** - Need to track flow into conditionals
3. **Name-based heuristics** - Need type-based detection

The recommended path is:
1. Quick wins (type-based sinks, basic guard detection) - 2 days
2. Rich value tracking - 1 week
3. Inter-procedural analysis - 2 weeks

This would bring us from "Python linter" to "Clippy-level" quality for security lints.
