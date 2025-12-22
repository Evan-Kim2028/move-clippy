# False Positive Analysis and Recommended Fixes

**Date**: 2024-12-19
**Based on**: sui-framework semantic validation (27 diagnostics, 44% FP rate)

---

## Executive Summary

| Lint | FP Rate | Root Cause | Fix Complexity |
|------|---------|------------|----------------|
| `unused_return_value` | 75% (6/8) | Doesn't track return propagation | Medium |
| `droppable_hot_potato_v2` | 67% (4/6) | Too aggressive on drop-only structs | Easy |
| `share_owned_authority` | 100% (2/2) | Doesn't understand intentional sharing | Medium |

---

## 1. `unused_return_value` (75% FP Rate)

### Current Implementation (semantic.rs:2778-2870)

```rust
fn check_unused_return_in_seq_item(...) {
    match &item.value {
        T::SequenceItem_::Seq(exp) => {
            // If a Seq item is a function call, its return value is discarded
            if let T::UnannotatedExp_::ModuleCall(call) = &exp.exp.value {
                // ... emit warning
            }
        }
        T::SequenceItem_::Bind(_, _, exp) => {
            // Bound expressions are using their return value
            check_unused_return_in_exp(exp, ...);  // <-- Recurses, but doesn't track context
        }
        _ => {}
    }
}
```

### Root Cause

The lint only checks `T::SequenceItem_::Seq` for discarded return values. However, it **doesn't distinguish** between:

1. **Truly discarded values** (in middle of sequence):
   ```move
   fun bad() {
       coin::split(&mut c, 100);  // Discarded! Bug!
       do_something_else();
   }
   ```

2. **Return-propagated values** (last expression in function):
   ```move
   fun withdraw_all(self: &mut Balance<T>): Balance<T> {
       let value = self.value;
       split(self, value)  // NOT discarded - this IS the return value!
   }
   ```

The issue is that Move's AST represents **tail expressions** the same way as **discarded expressions** in a sequence. Both appear as `SequenceItem_::Seq`.

### The Fix

Track whether we're at the **tail position** of a function. If the function returns non-unit and the last sequence item calls an important function, it's NOT discarded.

```rust
fn lint_unused_return_value(...) -> Result<()> {
    for (fname, fdef) in mdef.functions.key_cloned_iter() {
        let T::FunctionBody_::Defined((_use_funs, seq_items)) = &fdef.body.value else {
            continue;
        };

        // Check if function returns non-unit
        let returns_value = !matches!(&fdef.signature.return_type.value, T::Type_::Unit);

        let len = seq_items.len();
        for (idx, item) in seq_items.iter().enumerate() {
            let is_tail = idx == len - 1;
            
            // If this is tail position AND function returns value, skip warning
            if is_tail && returns_value {
                // The return value IS being used as the function's return
                continue;
            }
            
            check_unused_return_in_seq_item(...);
        }
    }
}
```

### Additional Improvement: Tail Position in Nested Blocks

For complete correctness, we need to track tail position through nested blocks:

```move
fun example(): Coin<SUI> {
    if (condition) {
        coin::split(&mut c, 100)  // Tail of if-branch -> function return
    } else {
        coin::split(&mut c, 200)  // Tail of else-branch -> function return
    }
}
```

This requires passing an `is_tail: bool` parameter through the recursive checks.

### Estimated Effort: 2-4 hours

---

## 2. `droppable_hot_potato_v2` (67% FP Rate)

### Current Implementation (semantic.rs:1070-1135)

```rust
fn lint_droppable_hot_potato_v2(...) -> Result<()> {
    for (sname, sdef) in minfo.structs.key_cloned_iter() {
        let has_only_drop = has_drop_ability(abilities)
            && !has_copy_ability(abilities)
            && !has_key_ability(abilities)
            && !has_store_ability(abilities);

        if !has_only_drop { continue; }

        // Skip empty structs (witness types)
        let is_empty = match &sdef.fields {
            N::StructFields::Defined(_, fields) => fields.is_empty(),
            N::StructFields::Native(_) => true,
        };
        if is_empty { continue; }

        // Emit warning for all non-empty drop-only structs
    }
}
```

### Root Cause

The lint correctly identifies:
- Empty drop-only structs = witness types (OK)
- Non-empty drop-only structs = potential broken hot potatoes (WARN)

But it **doesn't distinguish** between:
1. **Legitimate drop-only data structs**: `PCREntry`, `NitroAttestationDocument`, `RandomGenerator`
2. **Broken hot potatoes**: Structs that SHOULD enforce consumption but accidentally have `drop`

### The False Positives

| Struct | Why It's FP |
|--------|-------------|
| `PCREntry` | Attestation data, meant to be examined and dropped |
| `NitroAttestationDocument` | Return type from entry function, intentionally droppable |
| `RandomGenerator` | PRNG state, consumed or dropped after use |
| `Receiving<T>` | Receipt wrapper, dropped after receiving |

### The Fix: Heuristics for Legitimate Drop-Only Structs

Add heuristics to skip structs that are likely intentional:

```rust
fn lint_droppable_hot_potato_v2(...) -> Result<()> {
    // ... existing checks ...

    // NEW: Skip structs in framework/stdlib modules
    let module_name = mident.value.module.value().as_str();
    const FRAMEWORK_MODULES: &[&str] = &[
        "random", "transfer", "nitro_attestation", "funds_accumulator",
        "bcs", "sui_system", // ... etc
    ];
    if FRAMEWORK_MODULES.contains(&module_name) {
        continue;
    }

    // NEW: Skip structs with "result/document/entry/generator" in name
    let name_lower = name_str.to_lowercase();
    const RESULT_PATTERNS: &[&str] = &[
        "result", "document", "entry", "generator", "receipt", "receiving"
    ];
    if RESULT_PATTERNS.iter().any(|p| name_lower.contains(p)) {
        continue;
    }

    // NEW: Skip structs that are return types of public functions
    // (They're meant to be consumed by callers, not enforced internally)
    if is_public_function_return_type(sname, mdef) {
        continue;
    }
}
```

### Alternative: Change from Warning to Note

Since the distinction between "broken hot potato" and "legitimate drop struct" depends on **intent**, consider:

1. Downgrade from `warning` to `note` (informational)
2. Add a suggestion: "If this is intentional, suppress with `#[lint(allow(droppable_hot_potato_v2))]`"

### Estimated Effort: 1-2 hours

---

## 3. `share_owned_authority` (100% FP Rate)

### Current Implementation (semantic.rs:3033-3140)

```rust
fn lint_share_owned_authority(...) -> Result<()> {
    if is_share_call && is_key_store_type(&type_arg.value) {
        // Emit warning for ANY share of key+store type
    }
}
```

### Root Cause

The lint warns about sharing ANY `key + store` type, treating all of them as "authority objects". But many `key + store` types are **intentionally shared**:

| Type | Purpose | Sharing Correct? |
|------|---------|------------------|
| `Kiosk` | NFT marketplace | YES - public marketplace |
| `TransferPolicy<T>` | Transfer rules | YES - public policy |
| `Pool` | DEX liquidity pool | YES - public pool |
| `AdminCap` | Admin capability | NO - should stay owned |
| `TreasuryCap` | Mint authority | NO - should stay owned |

### The Fix: Semantic Allowlist

Add a list of types that are **designed** to be shared:

```rust
fn lint_share_owned_authority(...) -> Result<()> {
    // Types that are intentionally shared
    const SHARED_BY_DESIGN: &[(&str, &str)] = &[
        ("kiosk", "Kiosk"),
        ("transfer_policy", "TransferPolicy"),
        ("pool", "Pool"),
        ("amm", "Pool"),
        // Add more as needed
    ];

    if is_share_call && is_key_store_type(&type_arg.value) {
        let type_name = get_type_name(&type_arg.value);
        
        // Skip if type is in the allowlist
        if SHARED_BY_DESIGN.iter().any(|(_, name)| type_name.contains(name)) {
            continue;
        }

        // Also skip if the struct name contains "shared", "public", "pool", "market"
        let name_lower = type_name.to_lowercase();
        const SHARED_PATTERNS: &[&str] = &["shared", "public", "pool", "market", "kiosk"];
        if SHARED_PATTERNS.iter().any(|p| name_lower.contains(p)) {
            continue;
        }

        // Emit warning only for types that look like authority objects
        const AUTHORITY_PATTERNS: &[&str] = &["cap", "admin", "owner", "authority", "treasury"];
        if AUTHORITY_PATTERNS.iter().any(|p| name_lower.contains(p)) {
            // This is likely a capability being shared - WARN
            push_diag(...);
        }
    }
}
```

### Alternative: Flip the Logic

Instead of warning about ALL shares and whitelisting some, only warn about shares of types that **look like capabilities**:

```rust
// Only warn if type name matches authority patterns
const AUTHORITY_PATTERNS: &[&str] = &["cap", "admin", "owner", "authority", "treasury", "mint"];

if is_share_call && is_key_store_type(&type_arg.value) {
    let name_lower = type_name.to_lowercase();
    if AUTHORITY_PATTERNS.iter().any(|p| name_lower.contains(p)) {
        push_diag(...);
    }
}
```

This reduces FP rate dramatically while still catching the dangerous cases.

### Estimated Effort: 1-2 hours

---

## Implementation Priority

| Priority | Lint | Impact | Effort |
|----------|------|--------|--------|
| P0 | `unused_return_value` tail fix | Eliminates 75% FP | 2-4h |
| P1 | `share_owned_authority` flip logic | Eliminates 100% FP | 1-2h |
| P2 | `droppable_hot_potato_v2` heuristics | Eliminates 67% FP | 1-2h |

**Total estimated effort**: 4-8 hours

---

## Testing Strategy

After implementing fixes, re-run on sui-framework baseline:

```bash
./scripts/validate-ecosystem.sh --mode full

# Expected results after fixes:
# - unused_return_value: 0-2 findings (down from 8)
# - droppable_hot_potato_v2: 0-2 findings (down from 6)  
# - share_owned_authority: 0 findings (down from 2)
```

Create regression tests for each fix:
1. `tests/fixtures/semantic/unused_return_tail_position.move`
2. `tests/fixtures/semantic/droppable_legitimate_structs.move`
3. `tests/fixtures/semantic/share_intentional_objects.move`
