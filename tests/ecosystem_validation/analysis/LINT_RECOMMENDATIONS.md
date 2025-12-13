# Lint Improvement Recommendations

**Last Updated:** 2025-12-13  
**Purpose:** Track lint improvements based on triage findings

---

## Priority Queue

### High Priority (Security Impact)

| Lint | Issue | Recommendation | Effort |
|------|-------|----------------|--------|
| - | - | - | - |

### Medium Priority (FP Reduction)

| Lint | Issue | Recommendation | Effort |
|------|-------|----------------|--------|
| event_suffix | ~15% FP | Exclude DTO patterns | 2-3h |
| unbounded_vector_growth | ~25% FP | Detect implicit bounds | 4-6h |
| hardcoded_address | ~30% FP | Exclude test files | 1-2h |

### Low Priority (Nice to Have)

| Lint | Issue | Recommendation | Effort |
|------|-------|----------------|--------|
| - | - | - | - |

---

## Detailed Recommendations

### event_suffix Improvements

**Current Behavior:** Flags all structs with `copy, drop` without an `Event` suffix.

**Problem:** Many legitimate structs have `copy, drop`:
- DTOs for returning data
- Configuration structs
- Internal calculation state

**Proposed Fix:**

```rust
// In rules/style.rs - EventSuffixLint

fn should_skip_struct(name: &str) -> bool {
    // Skip common non-event patterns
    let skip_suffixes = ["Info", "Data", "Config", "Params", "Result", "State", "Context"];
    skip_suffixes.iter().any(|s| name.ends_with(s))
}

fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
    // ... existing code ...
    
    if has_copy_drop && !name.ends_with("Event") {
        // NEW: Skip common non-event patterns
        if should_skip_struct(name) {
            return;
        }
        
        ctx.report_node(...);
    }
}
```

**Expected Impact:** Reduce FP rate from ~15% to <5%

---

### unbounded_vector_growth Improvements

**Current Behavior:** Flags all `vector::push_back` without adjacent size checks.

**Problem:** Many vectors have implicit bounds:
- User-limited (MAX_POSITIONS checked elsewhere)
- Admin-only functions (trusted input)
- Fixed-size collections (e.g., always exactly N items)

**Proposed Fix:**

```rust
// In rules/security.rs - UnboundedVectorGrowthLint

fn has_implicit_bound(source: &str, module_root: Node) -> bool {
    // Check for MAX_ constants in module
    let has_max_constant = /* ... search for const MAX_* ... */;
    
    // Check for length comparison patterns
    let has_length_check = source.contains(".length() <") 
        || source.contains(".length() <=");
    
    has_max_constant || has_length_check
}

fn is_admin_function(func: Node, source: &str) -> bool {
    // Check if function has capability parameter
    let params = /* ... get function params ... */;
    params.iter().any(|p| p.contains("Cap") || p.contains("Admin"))
}
```

**Expected Impact:** Reduce FP rate from ~25% to <10%

---

### hardcoded_address Improvements

**Current Behavior:** Flags all literal addresses.

**Problem:** Test files legitimately use hardcoded addresses.

**Proposed Fix:**

```rust
// In rules/security.rs - HardcodedAddressLint

fn is_test_context(source: &str, node: Node) -> bool {
    // Check if in #[test_only] module
    let module_attrs = /* ... get module attributes ... */;
    if module_attrs.contains("test_only") {
        return true;
    }
    
    // Check if in #[test] function
    let func_attrs = /* ... get enclosing function attributes ... */;
    if func_attrs.contains("test") {
        return true;
    }
    
    false
}

fn is_system_address(addr: &str) -> bool {
    // Allow well-known system addresses
    matches!(addr, "@0x0" | "@0x1" | "@0x2" | "@0x3" | "@0x5" | "@0x6")
}
```

**Expected Impact:** Reduce FP rate from ~30% to <5%

---

## Implementation Tracking

| Lint | Recommendation | Branch | PR | Status |
|------|----------------|--------|-------|--------|
| event_suffix | DTO exclusion | - | - | 📋 Proposed |
| unbounded_vector_growth | Implicit bounds | - | - | 📋 Proposed |
| hardcoded_address | Test exclusion | - | - | 📋 Proposed |

---

## Promotion Candidates

Lints ready to move from Preview → Stable based on triage:

| Lint | Current | FP Rate | Findings | Recommendation |
|------|---------|---------|----------|----------------|
| modern_method_syntax | Stable | 0% | 1,242 | ✅ Already Stable |
| prefer_vector_methods | Stable | 0% | 746 | ✅ Already Stable |
| droppable_hot_potato | Stable | 0% | 4 | ✅ Validated |
| shared_capability | Stable | TBD | 8 | ⏳ Needs review |

---

## Change Log

| Date | Changes |
|------|---------|
| 2025-12-13 | Initial recommendations based on ecosystem validation |
