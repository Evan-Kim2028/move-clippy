# HLIR Constant Handling in Move Compiler

## Research Summary

This document investigates how the Sui Move compiler handles module constants at the HLIR (High-Level Intermediate Representation) level, explaining why `unchecked_division_v2` lint has false positives on patterns like `tick_size >= MIN_TICK_SIZE`.

## Problem Statement

The `unchecked_division_v2` lint flags division operations where the divisor hasn't been validated as non-zero. It should recognize guards like:

```move
const MIN_TICK_SIZE: u64 = 1000;
...
assert!(tick_size >= MIN_TICK_SIZE && tick_size <= MAX_TICK_SIZE, ...);
// tick_size is now known to be >= 1000, so safe to use as divisor
perpetual.min_trade_price % tick_size == 0  // <-- Still flagged as unsafe!
```

**Why does this happen?** The lint sees `tick_size >= Copy(temp)` instead of `tick_size >= 1000` or `tick_size >= Constant(MIN_TICK_SIZE)`.

## Compiler Pipeline Analysis

### 1. Source → Typed AST (Typing Phase)

At the typing phase, the expression `tick_size >= MIN_TICK_SIZE` is represented as:

```
BinopExp(
    Copy { var: tick_size },
    BinOp::Ge,
    Constant(module, MIN_TICK_SIZE)
)
```

The constant is still represented as a `Constant` node with a reference to the constant name.

### 2. Typed AST → HLIR (Translation Phase)

**File:** `hlir/translate.rs`

Line 1591:
```rust
E::Constant(_m, c) => make_exp(HE::Constant(c)), // only private constants (for now)
```

At this stage, `MIN_TICK_SIZE` becomes `HE::Constant(MIN_TICK_SIZE)` - still a symbolic reference.

### 3. HLIR → CFGIR (Control Flow Graph Generation)

**File:** `cfgir/translate.rs`

During CFG generation, complex expressions are "flattened" into a sequence of statements. The key transformation happens in `process_binops()` (lines 2782-2896):

```rust
fn build_binop(context, input_block, result_type, e) -> H::Exp {
    match e {
        BinopEntry::Op { lhs, op, rhs, ... } => {
            let mut lhs_block = make_block!();
            let mut lhs_exp = build_binop(context, &mut lhs_block, *lhs);
            let mut rhs_block = make_block!();
            let rhs_exp = build_binop(context, &mut rhs_block, *rhs);
            
            // KEY: If RHS has side effects, LHS gets bound to a temp!
            if !rhs_block.is_empty() {
                lhs_exp = bind_exp(context, &mut lhs_block, lhs_exp);
            }
            ...
        }
        ...
    }
}
```

**Critical insight:** When processing `tick_size >= MIN_TICK_SIZE && tick_size <= MAX_TICK_SIZE`:
1. The `&&` operator triggers `ShortCircuitAnd` handling
2. Both sides need to be evaluated in order with short-circuit semantics
3. The LHS (`tick_size >= MIN_TICK_SIZE`) gets bound to a temp variable
4. This creates an assignment: `let temp = tick_size >= MIN_TICK_SIZE`

### 4. Constant Folding (Optimization Phase)

**File:** `cfgir/optimize/constant_fold.rs`

Lines 111-120:
```rust
e_ @ E::Constant(_) => {
    let E::Constant(name) = e_ else { unreachable!() };
    if let Some(value) = context.constants.get(name) {
        *e_ = E::Value(value.clone());  // Replace Constant with Value!
        true
    } else {
        false
    }
}
```

The constant folding pass **replaces** `E::Constant(MIN_TICK_SIZE)` with `E::Value(1000)`. However, this happens **per-expression**, and the transformation may have already created temps.

## The Root Cause

The issue is a **phase ordering problem**:

1. **CFG Generation Phase:** Creates the control flow structure
   - `tick_size >= MIN_TICK_SIZE` becomes part of a short-circuit `&&`
   - The compiler creates temps to handle evaluation order
   - Result: `let temp1 = MIN_TICK_SIZE; let temp2 = tick_size >= Copy(temp1)`

2. **Constant Folding Phase:** Inlines constant values
   - Transforms `E::Constant(MIN_TICK_SIZE)` → `E::Value(1000)`
   - But the assignment `let temp1 = 1000` has already been created
   - The comparison still sees `Copy(temp1)`, not `E::Value(1000)` directly

By the time our lint runs (which is during the CFG phase, via SimpleAbsInt), we see:

```
Block 0:
    let temp1 = 1000            // Assignment from constant
    let temp2 = tick_size       // Copy of function parameter
    JumpIf(temp2 >= temp1, ...)  // Comparison uses temps
```

Our lint's `extract_nonzero_guard` sees `BinopExp(Copy(tick_size), Ge, Copy(temp1))` and doesn't know that `temp1` holds the constant value `1000`.

## Why the Compiler Does This

### Design Rationale

1. **Evaluation Order Semantics:** Move has strict left-to-right evaluation with short-circuit semantics for `&&` and `||`. Creating temps ensures correct evaluation order.

2. **Optimization Opportunities:** Separating CFG construction from constant folding allows the optimizer to work on well-formed CFG structures.

3. **Debugging:** Temps preserve source-level semantics for debugging and error messages.

4. **Code Generation:** The bytecode compiler benefits from explicit temps - it maps directly to VM registers.

### Trade-offs

| Approach | Pros | Cons |
|----------|------|------|
| Current (temps first, fold later) | Clean phase separation, correct semantics | Loses constant info in CFG |
| Fold during CFG construction | Preserves constant info | Complicates CFG generation |
| Propagate constant info in CFG | Both benefits | Requires additional analysis pass |

## Solutions for the Lint

### Option 1: Track Constant Assignments (Implemented)

Extend the abstract interpretation to track which locals hold constant values:

```rust
enum DivisorValue {
    Unknown,
    Validated,
    Constant,  // <-- Track known constants
}
```

When we see `let temp = 1000`, mark `temp` as `DivisorValue::Constant`.
When we see `var >= Copy(temp)` where `temp` is `Constant`, recognize the guard.

**Status:** Partially implemented, but not working because the constant assignment happens in a different CFG block than where we track the state.

### Option 2: Pre-Analysis Pass

Run a separate pass before the main lint to identify constant-valued locals:

```rust
fn find_constant_locals(cfg: &CFG) -> BTreeSet<Var> {
    // Find all assignments of form: let var = Value(non_zero)
    // Return the set of vars that are constant
}
```

Then use this set in the guard extraction logic.

### Option 3: Pattern Match on CFG Structure

Instead of looking at single expressions, pattern match on the CFG block structure:

```
if block has:
    Assign(temp1, Value(K)) where K != 0
    JumpIf(var >= Copy(temp1), ...)
then:
    Mark var as validated
```

This requires understanding the CFG block structure, not just expression structure.

### Option 4: Accept the Limitation (Current State)

Document that guards using named constants aren't recognized, and suggest:

```move
// Instead of:
assert!(tick_size >= MIN_TICK_SIZE, ...);

// Use inline literals:
assert!(tick_size >= 1000, ...);  // Lint will recognize this

// Or use explicit validation:
assert!(tick_size != 0, E_DIVISION_BY_ZERO);  // Clearest intent
```

## Recommendation

**Short-term:** Accept the limitation and document it. The lint is still valuable for catching cases where there's no validation at all.

**Medium-term:** Implement Option 3 (CFG pattern matching) as it's the most robust solution that works with the compiler's design.

**Long-term:** Consider contributing upstream to expose constant information in the CFG (Option 2), which would benefit all CFG-based analyses.

## Files Analyzed

- `sui/external-crates/move/crates/move-compiler/src/hlir/translate.rs` - HLIR translation
- `sui/external-crates/move/crates/move-compiler/src/hlir/ast.rs` - HLIR AST definitions
- `sui/external-crates/move/crates/move-compiler/src/cfgir/translate.rs` - CFG generation
- `sui/external-crates/move/crates/move-compiler/src/cfgir/optimize/constant_fold.rs` - Constant folding

## Related Issues

- False positives in `unchecked_division_v2` for `var >= CONST` guards
- Same issue would affect any CFG-based lint that needs to reason about constant values

## Date

December 2024
