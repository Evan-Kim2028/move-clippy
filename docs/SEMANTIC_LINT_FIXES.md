# Semantic-Only Lint Fixes (Zero Heuristics)

This document describes how to fix the high-FP lints using **only** semantic information from the Move compiler, with **zero** name-based heuristics.

## Principle

> A lint should only warn when the **type system and dataflow** prove there's an issue.
> If correctness depends on programmer **intent** (not observable from types), the lint should not exist.

---

## 1. `unused_return_value` - Pure Dataflow Fix

### Current Problem

The lint checks if a call is in `SequenceItem_::Seq` position and assumes the value is discarded. But `Seq` is also used for the **last expression** in a function, which IS the return value.

### The Fix: Track Expression Context

Pass context through the recursive checks to know if a value is consumed.

```rust
/// Context for tracking value consumption
struct ValueContext {
    /// Is this expression's value used? (assigned, returned, passed to call, etc.)
    is_consumed: bool,
    /// Is this the tail position of a function that returns non-unit?
    is_tail_of_returning_fn: bool,
}

fn lint_unused_return_value(
    out: &mut Vec<Diagnostic>,
    settings: &LintSettings,
    file_map: &MappedFiles,
    prog: &T::Program,
) -> Result<()> {
    for (mident, mdef) in prog.modules.key_cloned_iter() {
        // ... skip non-root packages ...
        
        for (fname, fdef) in mdef.functions.key_cloned_iter() {
            let T::FunctionBody_::Defined((_use_funs, seq_items)) = &fdef.body.value else {
                continue;
            };

            // Check if function returns non-unit
            let returns_value = !is_unit_type(&fdef.signature.return_type);

            let len = seq_items.len();
            for (idx, item) in seq_items.iter().enumerate() {
                let is_last = idx == len - 1;
                
                // Context: tail position of returning function = value IS consumed
                let ctx = ValueContext {
                    is_consumed: is_last && returns_value,
                    is_tail_of_returning_fn: is_last && returns_value,
                };
                
                check_unused_return_in_seq_item(
                    item,
                    ctx,
                    IMPORTANT_FUNCTIONS,
                    out,
                    settings,
                    file_map,
                    fname.value().as_str(),
                );
            }
        }
    }
    Ok(())
}

fn check_unused_return_in_seq_item(
    item: &T::SequenceItem,
    ctx: ValueContext,
    important_fns: &[(&str, &str)],
    out: &mut Vec<Diagnostic>,
    settings: &LintSettings,
    file_map: &MappedFiles,
    func_name: &str,
) {
    match &item.value {
        T::SequenceItem_::Seq(exp) => {
            // KEY CHANGE: If context says value is consumed, don't warn
            if ctx.is_consumed {
                // Value is used (returned, etc.) - only recurse into subexpressions
                check_unused_return_in_exp(exp, ValueContext { is_consumed: true, ..ctx }, ...);
                return;
            }
            
            // Value is truly discarded - check if it's an important function
            if let T::UnannotatedExp_::ModuleCall(call) = &exp.exp.value {
                // ... existing check logic ...
                push_diag(...);
            }
        }
        T::SequenceItem_::Bind(_, _, exp) => {
            // Bound = value is consumed
            check_unused_return_in_exp(exp, ValueContext { is_consumed: true, ..ctx }, ...);
        }
        _ => {}
    }
}

fn check_unused_return_in_exp(
    exp: &T::Exp,
    ctx: ValueContext,
    important_fns: &[(&str, &str)],
    out: &mut Vec<Diagnostic>,
    settings: &LintSettings,
    file_map: &MappedFiles,
    func_name: &str,
) {
    match &exp.exp.value {
        // If-else: both branches inherit context (if result is consumed, branches are consumed)
        T::UnannotatedExp_::IfElse(cond, if_body, else_opt) => {
            // Condition is always consumed (used for branching)
            check_unused_return_in_exp(cond, ValueContext { is_consumed: true, ..ctx }, ...);
            // Branches inherit parent context
            check_unused_return_in_exp(if_body, ctx, ...);
            if let Some(else_body) = else_opt {
                check_unused_return_in_exp(else_body, ctx, ...);
            }
        }
        
        // Block: only last expression might be consumed
        T::UnannotatedExp_::Block((_, seq)) => {
            let len = seq.len();
            for (idx, item) in seq.iter().enumerate() {
                let is_last = idx == len - 1;
                let item_ctx = if is_last { ctx } else { ValueContext { is_consumed: false, ..ctx } };
                check_unused_return_in_seq_item(item, item_ctx, ...);
            }
        }
        
        // Struct pack: all field values are consumed
        T::UnannotatedExp_::Pack(_, _, _, fields) => {
            for (_, _, (_, (_, field_exp))) in fields.iter() {
                check_unused_return_in_exp(field_exp, ValueContext { is_consumed: true, ..ctx }, ...);
            }
        }
        
        // Function call: all arguments are consumed
        T::UnannotatedExp_::ModuleCall(call) => {
            check_unused_return_in_exp(&call.arguments, ValueContext { is_consumed: true, ..ctx }, ...);
        }
        
        // ... etc for other expression types ...
    }
}
```

### Why This Is Pure Semantic

- **No name matching** - we don't check function names to decide if value is used
- **Pure dataflow** - we track where values flow based on AST structure
- **Type-based** - we check if function returns unit to determine tail behavior

### Complexity: Medium (2-4 hours)

---

## 2. `droppable_hot_potato_v2` - Remove or Downgrade

### Current Problem

The lint flags all non-empty structs with only `drop` ability. But we **cannot** distinguish:
- "Broken hot potato" (should NOT have drop)
- "Legitimate data struct" (intentionally has drop)

Both have identical type signatures: `struct Foo has drop { ... }`

### Why Semantic Analysis Can't Help

The difference is **programmer intent**, not type information:
- Hot potato: "I want to force consumption"
- Data struct: "I want to allow ignoring"

The type system encodes the RESULT of the decision, not the reasoning.

### The Fix: Remove or Make Advisory

**Option A: Remove the lint entirely**

```rust
// Delete lint_droppable_hot_potato_v2 function
// Delete DROPPABLE_HOT_POTATO_V2 descriptor
```

Rationale: A lint with 67% FP rate does more harm than good.

**Option B: Downgrade to note/suggestion**

```rust
pub static DROPPABLE_HOT_POTATO_V2: LintDescriptor = LintDescriptor {
    name: "droppable_hot_potato_v2",
    category: LintCategory::Pedantic,  // Changed from Security
    // Or: LintCategory::Suggestion
    description: "Struct has only `drop` ability - verify this is intentional",
    group: RuleGroup::Nursery,  // Opt-in only
    // ...
};
```

**Option C: Flip to lint the opposite**

Lint structs with NO abilities that are returned from public functions:

```rust
fn lint_unconsumed_hot_potato(...) {
    // Find public functions that return no-ability types
    for (fname, fdef) in mdef.functions.key_cloned_iter() {
        if !is_public(fdef) { continue; }
        
        let return_type = &fdef.signature.return_type;
        if is_no_ability_struct(return_type) {
            push_diag(
                "Public function returns a type with no abilities. \
                 Callers must consume this value in the same transaction. \
                 If this is unintentional, consider adding `drop` ability."
            );
        }
    }
}
```

This is semantically sound: if you return a no-ability type, you're forcing consumption.

### Complexity: Low (1 hour)

---

## 3. `share_owned_authority` - Usage Pattern Analysis

### Current Problem

The lint warns about sharing ANY `key + store` type. But:
- `Kiosk` is designed to be shared (marketplace)
- `AdminCap` should NOT be shared (authority)

Both have `key + store`.

### Why Pure Type Checking Isn't Enough

The abilities don't distinguish shared-state objects from authority objects.

### The Fix: Analyze Usage Patterns (Cross-Function)

Authority objects have a specific usage pattern:
1. Created in `init` with a one-time witness
2. Passed as first parameter to access-controlled functions
3. Never mutated directly (used as proof of authority)

```rust
fn lint_share_owned_authority(
    out: &mut Vec<Diagnostic>,
    settings: &LintSettings,
    file_map: &MappedFiles,
    prog: &T::Program,
) -> Result<()> {
    // Build a map of type -> usage pattern
    let usage_patterns = analyze_type_usage_patterns(prog);
    
    for (mident, mdef) in prog.modules.key_cloned_iter() {
        for (fname, fdef) in mdef.functions.key_cloned_iter() {
            // Find share_object calls
            visit_expressions(fdef, |exp| {
                if let T::UnannotatedExp_::ModuleCall(call) = &exp.exp.value {
                    if is_share_call(call) {
                        let shared_type = get_type_arg(call);
                        
                        // Check if this type is used as an authority
                        if is_authority_usage_pattern(&usage_patterns, shared_type) {
                            push_diag(...);
                        }
                    }
                }
            });
        }
    }
    Ok(())
}

fn analyze_type_usage_patterns(prog: &T::Program) -> HashMap<TypeId, UsagePattern> {
    let mut patterns = HashMap::new();
    
    for (mident, mdef) in prog.modules.key_cloned_iter() {
        for (fname, fdef) in mdef.functions.key_cloned_iter() {
            // Check first parameter pattern (authority objects are often first param)
            if let Some(first_param) = fdef.signature.parameters.first() {
                let param_type = &first_param.1;
                if is_ref_type(param_type) {
                    // Type is used as &T or &mut T parameter
                    patterns.entry(type_id(param_type))
                        .or_default()
                        .used_as_ref_param = true;
                }
            }
        }
        
        // Check init function for OTW pattern
        if let Some(init_fn) = mdef.functions.get(&FunctionName::INIT) {
            // Analyze what types are created and how they're disposed
            visit_expressions(init_fn, |exp| {
                if let T::UnannotatedExp_::Pack(module, name, _, _) = &exp.exp.value {
                    // Type is created in init
                    patterns.entry(type_id_from_name(module, name))
                        .or_default()
                        .created_in_init = true;
                }
                if is_transfer_call(exp) {
                    // Type is transferred (not shared) in init
                    let transferred_type = get_transferred_type(exp);
                    patterns.entry(transferred_type)
                        .or_default()
                        .transferred_in_init = true;
                }
            });
        }
    }
    
    patterns
}

struct UsagePattern {
    created_in_init: bool,
    transferred_in_init: bool,
    used_as_ref_param: bool,
    // ... other signals
}

fn is_authority_usage_pattern(patterns: &HashMap<TypeId, UsagePattern>, ty: TypeId) -> bool {
    if let Some(pattern) = patterns.get(&ty) {
        // Authority objects are typically:
        // 1. Created in init
        // 2. Transferred (not shared) to a specific address
        // 3. Used as reference parameters for access control
        pattern.created_in_init 
            && pattern.transferred_in_init 
            && pattern.used_as_ref_param
    } else {
        false
    }
}
```

### Why This Is Pure Semantic

- **No name matching** - we don't check if name contains "Cap" or "Admin"
- **Pure usage analysis** - we look at how the type is actually used
- **Cross-function** - we build a global picture of type usage

### Complexity: High (4-8 hours)

### Alternative: Opt-In Only

If cross-function analysis is too complex, make the lint opt-in:

```rust
pub static SHARE_OWNED_AUTHORITY: LintDescriptor = LintDescriptor {
    name: "share_owned_authority",
    category: LintCategory::Restriction,  // Opt-in
    group: RuleGroup::Nursery,  // Not default
    description: "Warns about sharing key+store objects (opt-in, may have FPs)",
    // ...
};
```

Users who want this check can enable it and suppress FPs manually.

---

## Summary

| Lint | Fix Strategy | Heuristics Used | Complexity |
|------|--------------|-----------------|------------|
| `unused_return_value` | Dataflow context tracking | **ZERO** | Medium |
| `droppable_hot_potato_v2` | Remove or downgrade | **ZERO** | Low |
| `share_owned_authority` | Cross-function usage analysis | **ZERO** | High |

The key insight: **If we can't prove an issue from types and dataflow alone, we shouldn't warn.**
