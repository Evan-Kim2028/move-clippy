# Systematic Fix Plan for Critical Blockers

**Goal:** Fix 31 broken lints to make move-clippy production-ready for v1.0  
**Timeline:** 1-2 weeks  
**Approach:** Quick wins first, then systematic type-based rewrites

---

## Phase 1: Quick Wins (1-2 days) - Fix Naming Lints

### Priority 1A: Deprecate Wrong Convention Lints (2 hours)

These lints enforce conventions that conflict with actual Sui code:

#### 1. `capability_naming` (22 FPs)
**Problem:** Enforces `_cap` suffix but Sui uses `Cap` suffix  
**Fix:** Deprecate the lint

```rust
// In src/semantic.rs
pub static CAPABILITY_NAMING: LintDescriptor = LintDescriptor {
    name: "capability_naming",
    category: LintCategory::Naming,
    description: "[DEPRECATED] Use Cap suffix (not _cap) per Sui conventions",
    group: RuleGroup::Deprecated,  // Change from Stable
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
};
```

**Justification:**
- Sui framework uses `AdminCap`, `TreasuryCap`, `UpgradeCap`
- NOT `admin_cap`, `treasury_cap`
- Our lint is backwards

#### 2. `event_naming` (11 FPs)
**Problem:** Enforces `_event` suffix but Sui doesn't use it  
**Fix:** Deprecate the lint

```rust
pub static EVENT_NAMING: LintDescriptor = LintDescriptor {
    name: "event_naming",
    category: LintCategory::Naming,
    description: "[DEPRECATED] Sui events don't use _event suffix",
    group: RuleGroup::Deprecated,  // Change from Stable
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
};
```

**Justification:**
- Sui events: `Transferred`, `PoolCreated`, `SwapExecuted`
- NOT `transferred_event`, `pool_created_event`

#### 3. `getter_naming` (5 FPs)
**Problem:** Flags `get_` prefix but Sui uses it everywhere  
**Fix:** Deprecate the lint

```rust
pub static GETTER_NAMING: LintDescriptor = LintDescriptor {
    name: "getter_naming",
    category: LintCategory::Naming,
    description: "[DEPRECATED] get_ prefix is standard in Sui",
    group: RuleGroup::Deprecated,  // Change from Stable
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBased,
};
```

**Justification:**
- Sui uses `get_price()`, `get_balance()`, `get_total_supply()`
- The lint is enforcing anti-patterns

**Implementation:**
```bash
# Single file edit
vim src/semantic.rs
# Change RuleGroup::Stable -> RuleGroup::Deprecated for all 3 lints
# That's it!
```

**Result:** 38 false positives eliminated in 2 hours

---

## Phase 2: Type-Based Rewrites (3-5 days)

### Strategy: Use `type_classifier.rs` for ALL type checks

The pattern is simple:
1. **DON'T** use `name.ends_with("Cap")` or `name.contains("withdraw")`  
2. **DO** use type abilities from Move compiler

### Template for Type-Based Detection

```rust
// BAD - Name-based heuristic
if struct_name.ends_with("Cap") {
    // This catches: "Recap", "Handicap", "NightCap"
}

// GOOD - Type-based detection
use crate::type_classifier;

if type_classifier::is_capability_type(&abilities) {
    // Only structs with key+store, no copy/drop
}
```

### Priority 2A: Add Missing Type Classifiers (1 day)

**File:** `src/type_classifier.rs`

Add these missing classifiers:

```rust
use move_compiler::expansion::ast::AbilitySet;
use move_compiler::naming::ast as N;
use move_compiler::parser::ast::Ability_;

// Already exists (good!)
pub fn is_key_store_type(ty: &N::Type_) -> bool { ... }
pub fn is_event_like_type(ty: &N::Type_) -> bool { ... }

// ADD THESE:

/// Capability: key + store, no copy/drop
pub fn is_capability_type(abilities: &AbilitySet) -> bool {
    abilities.has_ability_(Ability_::Key)
        && abilities.has_ability_(Ability_::Store)
        && !abilities.has_ability_(Ability_::Copy)
        && !abilities.has_ability_(Ability_::Drop)
}

/// Hot Potato: NO abilities (flash loan receipts)
pub fn is_hot_potato_type(abilities: &AbilitySet) -> bool {
    !abilities.has_ability_(Ability_::Key)
        && !abilities.has_ability_(Ability_::Store)
        && !abilities.has_ability_(Ability_::Copy)
        && !abilities.has_ability_(Ability_::Drop)
}

/// Resource: key+store for value-bearing objects
pub fn is_resource_type(abilities: &AbilitySet) -> bool {
    abilities.has_ability_(Ability_::Key)
        && abilities.has_ability_(Ability_::Store)
        && !abilities.has_ability_(Ability_::Copy)
        // May or may not have drop
}

/// Event: copy+drop only
pub fn is_event_type(abilities: &AbilitySet) -> bool {
    abilities.has_ability_(Ability_::Copy)
        && abilities.has_ability_(Ability_::Drop)
        && !abilities.has_ability_(Ability_::Key)
        && !abilities.has_ability_(Ability_::Store)
}

/// Configuration struct: copy+drop+store (but NOT key)
pub fn is_config_type(abilities: &AbilitySet) -> bool {
    abilities.has_ability_(Ability_::Copy)
        && abilities.has_ability_(Ability_::Drop)
        && abilities.has_ability_(Ability_::Store)
        && !abilities.has_ability_(Ability_::Key)
}

/// Droppable (has drop ability)
pub fn has_drop_ability(abilities: &AbilitySet) -> bool {
    abilities.has_ability_(Ability_::Drop)
}
```

**Why this works:**
- ✅ Type abilities are **compiler-verified**
- ✅ Can't be fooled by naming
- ✅ Matches Move's semantic model
- ✅ Zero false positives from name collisions

---

### Priority 2B: Fix Security Lints (2-3 days)

#### Fix 1: `droppable_hot_potato` - Remove Keyword Check

**Current (name-based):**
```rust
// In src/rules/security.rs
const HOT_POTATO_KEYWORDS: &[&str] = &[
    "receipt", "loan", "flash", "promise", "ticket", ...
];

fn check_droppable_hot_potato(...) {
    if HOT_POTATO_KEYWORDS.iter().any(|kw| name.contains(kw)) 
       && has_drop { ... }
}
```

**Fixed (type-based):**
```rust
use crate::type_classifier;

fn check_droppable_hot_potato_semantic(
    sdef: &StructDef,
    sname: &StructName,
) -> bool {
    let abilities = &sdef.abilities;
    
    // Hot potato = NO abilities
    if !type_classifier::is_hot_potato_type(abilities) {
        return false;  // Not a hot potato
    }
    
    // Check if it incorrectly has drop
    if abilities.has_ability_(Ability_::Drop) {
        return true;  // BUG: Hot potato should have NO abilities!
    }
    
    false
}
```

**Why this works:**
- ✅ Detects actual hot potatoes (zero abilities)
- ✅ Flags ones with drop (security bug)
- ❌ Won't flag random structs with "receipt" in name
- ❌ Won't miss hot potatoes with different naming

**Testing:**
```rust
#[test]
fn test_true_hot_potato_with_drop() {
    let source = r#"
        // BUG - has drop when it shouldn't
        struct FlashLoanReceipt has drop { }
    "#;
    assert!(lint_detects(source, "droppable_hot_potato"));
}

#[test]
fn test_config_struct_with_drop_ok() {
    let source = r#"
        // OK - Config structs SHOULD have drop
        struct PoolReceipt has copy, drop, store { }
    "#;
    assert!(lint_passes(source));
}
```

---

#### Fix 2: Deprecate `shared_capability` (name-based)

**Current Problem:**
```rust
// Checks if struct name contains "Cap" and is being shared
if struct_name.contains("Cap") && is_shared { ... }
// False positives: "Capacity", "Capital", "Recap"
```

**Solution:** Already have type-based replacement!

```rust
// We have share_owned_authority which uses type analysis
pub static SHARE_OWNED_AUTHORITY: LintDescriptor = LintDescriptor {
    name: "share_owned_authority",
    category: LintCategory::Security,
    description: "Sharing key+store object makes it publicly accessible",
    group: RuleGroup::Stable,  // This one is GOOD!
    ...
};
```

**Action:**
```rust
// Deprecate the old one
pub static SHARED_CAPABILITY: LintDescriptor = LintDescriptor {
    name: "shared_capability",
    group: RuleGroup::Deprecated,  // Mark deprecated
    description: "[DEPRECATED] Use share_owned_authority instead",
    ...
};
```

---

#### Fix 3: `unchecked_withdrawal` - Make Type-Based

**Current (broken):**
```rust
const WITHDRAWAL_PATTERNS: &[&str] = &["withdraw", "take", "extract"];

if func_name.contains("withdraw") && !has_assert { ... }
// FPs: "withdraw_protocol_fee", "set_withdrawal_cap"
```

**Fixed (type-based):**
```rust
fn lint_unchecked_withdrawal_v2(
    fdef: &T::Function,
    fname: &FunctionName,
) -> Option<Diagnostic> {
    // Look for functions that RETURN a resource (key+store)
    let return_ty = &fdef.signature.return_type.value;
    
    if !type_classifier::is_resource_type_from_typing(return_ty) {
        return None;  // Not returning a valuable resource
    }
    
    // Check function body for balance validation
    let has_balance_check = scan_for_assertion(
        &fdef.body,
        |assertion| is_balance_comparison(assertion)
    );
    
    if !has_balance_check {
        Some(Diagnostic {
            message: format!(
                "Function `{}` returns a resource without balance validation",
                fname
            ),
            ...
        })
    } else {
        None
    }
}

fn is_balance_comparison(assert_cond: &Exp) -> bool {
    // Look for: assert!(balance >= amount, ...)
    // or: assert!(has_balance(...), ...)
    matches_pattern(assert_cond, &[
        Pattern::Comparison { op: ">=" },
        Pattern::FunctionCall { name: "has_balance" },
        Pattern::FunctionCall { name: "can_withdraw" },
    ])
}
```

**Why better:**
- ✅ Only checks functions returning valuable resources
- ✅ No false positives from name matching
- ✅ Detects pattern regardless of function name
- ⚠️ May have false negatives (validation in caller)
  - But that's OK - better than 181 false positives!

---

### Priority 2C: Fix Semantic Lints (1-2 days)

#### Fix: `unused_capability_param` (209 FPs)

**Current (broken):**
```rust
// Checks if parameter name ends with "_cap"
if param_name.ends_with("_cap") && !is_used { ... }
```

**Fixed (type-based):**
```rust
fn lint_unused_capability_param_v2(
    fdef: &T::Function,
) -> Vec<Diagnostic> {
    let mut diagnostics = vec![];
    
    for (param_var, param_type) in &fdef.signature.parameters {
        // Check if parameter is a capability (type-based)
        if !type_classifier::is_capability_type_from_typing(param_type) {
            continue;  // Not a capability
        }
        
        // Scan function body for usage
        let is_used = scan_body_for_var_usage(&fdef.body, param_var);
        
        if !is_used {
            diagnostics.push(Diagnostic {
                message: format!(
                    "Capability parameter is unused - may indicate missing access control"
                ),
                help: Some("Remove the parameter or add authorization checks".to_string()),
                ...
            });
        }
    }
    
    diagnostics
}
```

---

## Phase 3: Remove Experimental Lints (1 day)

### Action: Delete from Registry

**Simple approach:**

```rust
// In src/lint.rs - REMOVE these from SEMANTIC_LINT_NAMES:
pub const SEMANTIC_LINT_NAMES: &[&str] = &[
    // REMOVE THESE:
    // "transitive_capability_leak",  // 302 FPs
    // "unused_capability_param_v2",  // 209 FPs  
    // "flashloan_without_repay",     // 40 FPs
    
    // KEEP WORKING ONES:
    "share_owned",
    "self_transfer",
    "event_emit_type_sanity",
    ...
];
```

**Alternative: Keep but mark experimental**

```rust
pub static TRANSITIVE_CAPABILITY_LEAK: LintDescriptor = LintDescriptor {
    name: "transitive_capability_leak",
    group: RuleGroup::Experimental,  // Requires --experimental
    description: "⚠️ HIGH FALSE POSITIVE RATE - use with caution",
    ...
};
```

**Add warning to docs:**
```markdown
## Experimental Lints (Not Recommended)

These lints have known high false positive rates (>30%) and use heuristic
detection. They are useful for security audits but NOT recommended for CI.

Enable with: `move-clippy --experimental`

| Lint | FP Rate | Status |
|------|---------|--------|
| `transitive_capability_leak` | 38% | Research only |
| `unused_capability_param_v2` | 26% | Needs rewrite |
| `flashloan_without_repay` | 18% | Needs rewrite |
```

---

## Implementation Checklist

### Week 1: Quick Wins + Foundation

**Day 1: Naming Lints** (2 hours)
- [ ] Deprecate `capability_naming` 
- [ ] Deprecate `event_naming`
- [ ] Deprecate `getter_naming`
- [ ] Update tests to expect deprecated warnings
- [ ] Run ecosystem validation → expect 38 fewer FPs

**Day 2: Type Classifier** (6 hours)
- [ ] Add `is_capability_type()`
- [ ] Add `is_hot_potato_type()`
- [ ] Add `is_resource_type()`
- [ ] Add `is_event_type()`
- [ ] Add `is_config_type()`
- [ ] Add comprehensive unit tests

**Day 3-4: Fix `droppable_hot_potato`** (2 days)
- [ ] Implement type-based detection
- [ ] Add test cases for true positives
- [ ] Add test cases for false positive prevention
- [ ] Run on ecosystem repos
- [ ] Verify 0 false positives

**Day 5: Fix `shared_capability`** (4 hours)
- [ ] Deprecate old name-based version
- [ ] Point users to `share_owned_authority`
- [ ] Update documentation

### Week 2: Semantic Rewrites

**Day 6-7: Fix `unchecked_withdrawal`** (2 days)
- [ ] Implement type-based resource detection
- [ ] Add assertion pattern matching
- [ ] Test on real contracts
- [ ] Validate < 5% FP rate

**Day 8-9: Fix `unused_capability_param`** (2 days)
- [ ] Implement type-based capability detection
- [ ] Add usage scanning in function body
- [ ] Test extensively
- [ ] Validate < 5% FP rate

**Day 10: Cleanup** (1 day)
- [ ] Remove experimental lints OR mark clearly
- [ ] Update all documentation
- [ ] Run full ecosystem validation
- [ ] Generate quality report

---

## Success Metrics

**Before:**
- 54 total lints
- 28 use heuristics (52%)
- 792 FPs on Sui framework
- Quality score: 6.5/10

**After Week 1:**
- 51 total lints (3 deprecated)
- 25 use heuristics (49%)
- ~400 FPs (50% reduction)
- Quality score: 7.5/10

**After Week 2:**
- 45-48 total lints
- 5-10 use heuristics (11-22%)
- < 50 FPs (93% reduction)
- Quality score: 9/10 ✅ Production ready!

---

## Code Review Guidelines

For each rewritten lint, verify:

1. **No Name Matching**
   ```rust
   // ❌ BAD
   if name.contains("cap") { ... }
   
   // ✅ GOOD
   if type_classifier::is_capability_type(&abilities) { ... }
   ```

2. **Use Type Abilities**
   ```rust
   // ✅ GOOD - Check what the type CAN do
   abilities.has_ability_(Ability_::Key)
   abilities.has_ability_(Ability_::Store)
   ```

3. **Test Both Sides**
   ```rust
   #[test]
   fn test_true_positive() { ... }  // Should detect
   
   #[test]
   fn test_false_positive_prevention() { ... }  // Should NOT detect
   ```

4. **Clear Error Messages**
   ```rust
   format!(
       "Function returns resource `{}` without balance check",
       type_name
   )
   // Better than: "may be unsafe"
   ```

---

## Questions & Answers

**Q: Why deprecate instead of fix naming lints?**  
A: They enforce conventions that conflict with Sui's actual style. Better to remove than confuse users.

**Q: What about CFG-aware lints (Phase II/III)?**  
A: Those require more research. Focus on quick wins first. Can tackle CFG lints in v1.1.

**Q: Won't removing experimental lints reduce our lint count?**  
A: Better to have 45 **working** lints than 54 lints with 28 broken. Quality > quantity.

**Q: How do we handle edge cases in type-based detection?**  
A: Accept some false negatives rather than many false positives. It's better to miss 5% of real bugs than flag 30% non-bugs.

**Q: Should we remove or just mark experimental?**  
A: Mark experimental for v1.0. Users can opt-in for security audits. Remove in v2.0 if not fixed.

---

## Next Steps

1. **Start with Day 1** - Deprecate 3 naming lints (2 hours)
2. **Validate** - Run on Sui framework, expect 38 fewer FPs
3. **Continue to Day 2** - Build type classifier foundation
4. **Iterate** - One lint at a time, test thoroughly

**Ready to start?** Let's begin with the quick win - deprecating the 3 naming lints!
