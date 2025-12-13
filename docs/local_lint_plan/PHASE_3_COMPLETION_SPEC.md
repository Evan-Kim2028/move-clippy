# Phase 3 Completion Specification

**Status:** Analysis & Planning  
**Date:** 2025-12-13  
**Purpose:** Analyze current Move Clippy implementation and propose actionable next steps for ecosystem validation, auto-fix, and FP refinement

---

## Executive Summary

### What's Working Well ✅

1. **Infrastructure Complete**
   - ✅ Ruff-inspired stability system (Stable/Preview/Deprecated)
   - ✅ 52 total lints (43 fast + 9 semantic)
   - ✅ Preview flag for opt-in experimental lints
   - ✅ Comprehensive test infrastructure (55 tests passing)
   - ✅ Snapshot testing with insta
   - ✅ Ecosystem test repos cloned (14 repositories)

2. **Documentation Quality**
   - ✅ Audit-backed security lints with verified sources
   - ✅ SECURITY_LINTS.md with vulnerable/correct patterns
   - ✅ STABILITY.md documenting stability guarantees
   - ✅ Test fixtures with positive/negative examples

3. **Semantic Lints (Phase 2)**
   - ✅ 6 semantic security lints implemented
   - ✅ Conservative heuristics to minimize FP
   - ✅ Typed AST analysis for deep inspection

### Current Gaps 🔍

1. **No Real-World Validation**
   - ⚠️ Semantic lints not tested against ecosystem repos
   - ⚠️ Unknown actual FP rate on production code
   - ⚠️ No pre/post audit snapshot validation

2. **Limited Auto-Fix Coverage**
   - ⚠️ Only 4 lints have auto-fix capability
   - ⚠️ No auto-fix for any security lints

3. **FP Rate Unknown**
   - ⚠️ Theoretical FP estimates, not measured
   - ⚠️ No data-driven promotion from Preview → Stable

---

## Phase 3 Remaining Tasks: Detailed Analysis

### Task 1: Ecosystem Validation on Real Repos

#### Objective
Run all lints against 10+ production Move repositories and measure:
1. **True Positives (TP):** Actual bugs found
2. **False Positives (FP):** Incorrect warnings
3. **FP Rate:** `FP / (TP + FP)` for each lint

#### Available Ecosystem Repos

| Repository | Type | LOC | Status | Audit History |
|------------|------|-----|--------|---------------|
| AlphaLend | Lending | ~3K | ✅ Cloned | Pre/post audit available |
| Scallop | Lending | ~5K | ✅ Cloned | OtterSec + MoveBit audits |
| Suilend | Lending | ~4K | ✅ Cloned | OtterSec audit (commit 07d8e84) |
| DeepBookV3 | DEX | ~8K | ✅ Cloned | Trail of Bits audit |
| Cetus CLMM | DEX | ~6K | ✅ Cloned | Post-hack code available |
| Bluefin (3 repos) | Perps | ~10K | ✅ Cloned | MoveBit Contest 2024 |
| OpenZeppelin Sui | Library | ~2K | ✅ Cloned | Production-grade reference |
| Steamm | AMM | ~2K | ✅ Cloned | Unknown audit status |

**Total:** 14 repos, ~40K lines of production Move code

#### Proposed Validation Methodology

```bash
# 1. Baseline run - capture all findings
for repo in ecosystem-test-repos/*; do
  move-clippy check $repo --preview --json > results/$repo.json
done

# 2. Manual triage - classify each finding
# - TP: Real bug (even if already fixed in later commit)
# - FP: Incorrect warning
# - TN: Correct pattern, no warning (expected)
# - Informational: Useful but not a bug

# 3. Calculate metrics per lint
# FP Rate = FP / (TP + FP)
# Precision = TP / (TP + FP)
# Recall = TP / (TP + FN) -- harder to measure, requires known bugs

# 4. Pre/post audit validation
# For repos with pre-audit commits:
# - Run on pre-audit commit (should fire)
# - Run on post-audit commit (should be silent)
```

#### Expected Outcomes

**Tier 1: Stable Lints** (Target: <10% FP)
- `droppable_hot_potato` - Already validated on AlphaLend (1 TP, 0 FP)
- `excessive_token_abilities` - Needs event exclusion refinement
- `shared_capability` - Not yet implemented (Phase 4)

**Tier 2: Preview Lints** (Target: 10-25% FP)
- `suspicious_overflow_check` - Based on Cetus hack
- `unchecked_division` - Conservative heuristic
- `oracle_zero_price` - **NEEDS VALIDATION**
- `unused_return_value` - **NEEDS VALIDATION**
- `missing_access_control` - **NEEDS VALIDATION**

#### Predicted Issues

1. **`oracle_zero_price`**
   - **Risk:** High FP if oracle library has built-in zero checks
   - **Mitigation:** Check for library-specific safe functions (e.g., `pyth::get_price_checked`)
   - **Expected FP rate:** 20-30% without refinement

2. **`unused_return_value`**
   - **Risk:** False positives on internal helper functions
   - **Mitigation:** Exclude functions with `#[test_only]` or in test files
   - **Expected FP rate:** 10-15%

3. **`missing_access_control`**
   - **Risk:** High FP on getter functions and witness patterns
   - **Mitigation:** Already excludes `get_`, `is_`, etc. - may need to expand
   - **Expected FP rate:** 15-20%

#### Implementation Plan

**Week 1: Infrastructure Setup**
```bash
# Create validation harness
tests/ecosystem_validation/
├── run_all.sh              # Execute linter on all repos
├── triage.toml             # Manual FP/TP classifications
├── calculate_metrics.py    # Compute FP rates, precision, recall
└── results/
    ├── alphalend.json
    ├── scallop.json
    └── ...
```

**Week 2: Manual Triage**
- Run linter on all 14 repos (fast lints only first)
- Manually classify each finding as TP/FP/Informational
- Document reasons for each classification
- Target: 100% of findings triaged

**Week 3: Semantic Lint Validation**
- Run semantic lints (requires full compilation)
- May hit compilation errors on older repos - document and skip
- Focus on 3 new lints: `oracle_zero_price`, `unused_return_value`, `missing_access_control`

**Week 4: Metrics & Refinement**
- Calculate actual FP rates
- Compare to theoretical estimates
- Propose lint refinements to reduce FP
- Update stability classifications

---

### Task 2: Auto-Fix Implementation for Selected Lints

#### Current Auto-Fix Coverage

**Implemented (4 lints):**
1. `modern_module_syntax` - Safe auto-fix
2. `redundant_self_import` - Safe auto-fix
3. `abilities_order` - Safe auto-fix
4. `constant_naming` - Safe auto-fix (SCREAMING_SNAKE_CASE conversion)

**Not Implemented (48 lints):**
- All security lints lack auto-fix
- Most style lints lack auto-fix

#### Auto-Fix Safety Classification

**Safe Auto-Fixes** (Can always apply without changing semantics):
- Style/formatting changes
- Import cleanup
- Renaming that preserves meaning

**Unsafe Auto-Fixes** (Require user review, enabled with `--unsafe-fixes`):
- Adding `assert!()` statements
- Changing function signatures
- Modifying control flow

**Not Auto-Fixable** (Requires human judgment):
- Semantic security issues
- Logic bugs
- Design pattern violations

#### Proposed Auto-Fix Targets (Low-Hanging Fruit)

**Priority 1: Style Lints (Safe)**

1. **`modern_method_syntax`** - Convert `coin::split(&mut coin, ...)` → `coin.split(...)`
   ```rust
   // Detection: Already implemented
   // Fix: Tree-sitter edit to rewrite call syntax
   // Safety: Safe (preserves semantics)
   // Effort: 2-3 hours
   ```

2. **`merge_test_attributes`** - Merge `#[test] #[expected_failure]` → `#[test, expected_failure]`
   ```rust
   // Detection: Already implemented
   // Fix: Text replacement, safe
   // Safety: Safe
   // Effort: 1 hour
   ```

3. **`doc_comment_style`** - Convert `// comment` → `/// comment` for public items
   ```rust
   // Detection: Already implemented
   // Fix: Text replacement
   // Safety: Safe
   // Effort: 1 hour
   ```

**Priority 2: Security Lints (Unsafe, Requires `--unsafe-fixes`)**

4. **`missing_witness_drop`** - Add `has drop` to OTW structs
   ```rust
   // Before:
   struct COIN {}
   
   // After:
   struct COIN has drop {}
   
   // Safety: Unsafe (changes struct ABI)
   // Effort: 3-4 hours
   ```

5. **`unchecked_coin_split`** - Add balance assertion before coin::split
   ```rust
   // Before:
   coin::split(&mut coin, amount, ctx);
   
   // After:
   assert!(coin::value(&coin) >= amount, E_INSUFFICIENT_BALANCE);
   coin::split(&mut coin, amount, ctx);
   
   // Safety: Unsafe (adds runtime check)
   // Effort: 4-5 hours
   ```

#### Auto-Fix Architecture

**Current Implementation (in `src/fix.rs`):**
```rust
pub struct Fix {
    pub span: Span,
    pub replacement: String,
    pub safety: FixSafety,  // Safe or Unsafe
}

pub enum FixSafety {
    Safe,    // Always apply
    Unsafe,  // Requires --unsafe-fixes
}
```

**Proposed Enhancement:**
```rust
pub enum FixSafety {
    Safe,           // Always apply (formatting, imports)
    DisplayOnly,    // Show suggestion but don't apply (complex changes)
    Unsafe,         // Requires --unsafe-fixes (semantic changes)
}

pub struct Fix {
    pub span: Span,
    pub replacement: String,
    pub safety: FixSafety,
    pub description: String,  // Human-readable explanation
}
```

#### Implementation Effort Estimate

| Lint | Auto-Fix Type | Effort | Priority |
|------|---------------|--------|----------|
| `modern_method_syntax` | Safe | 2-3h | High |
| `merge_test_attributes` | Safe | 1h | High |
| `doc_comment_style` | Safe | 1h | Medium |
| `missing_witness_drop` | Unsafe | 3-4h | Medium |
| `unchecked_coin_split` | Unsafe | 4-5h | Low |

**Total Effort:** ~12-15 hours for 5 lints

#### Success Metrics

1. **Coverage:** At least 10 lints with auto-fix (currently 4)
2. **Safety:** 80%+ of auto-fixes are "Safe" (no `--unsafe-fixes` needed)
3. **Accuracy:** <1% of auto-fixes introduce compilation errors
4. **Adoption:** Developers use `--fix` on 50%+ of lint runs

---

### Task 3: FP Rate Refinement Based on Ecosystem Testing

#### Current Theoretical FP Estimates

| Lint | Theoretical FP | Stability | Notes |
|------|----------------|-----------|-------|
| `droppable_hot_potato` | <5% | Stable | Validated: 1 TP on AlphaLend |
| `excessive_token_abilities` | <5% | Stable | Needs event exclusion |
| `unchecked_division` | 15-20% | Preview | Conservative heuristic |
| `oracle_zero_price` | 20-30% | Preview | **NOT VALIDATED** |
| `unused_return_value` | 10-15% | Preview | **NOT VALIDATED** |
| `missing_access_control` | 15-20% | Preview | **NOT VALIDATED** |

#### Refinement Strategy

**Phase 1: Measure Actual FP Rate**
1. Run ecosystem validation (Task 1)
2. Manual triage of all findings
3. Calculate actual FP rate per lint
4. Compare to theoretical estimate

**Phase 2: Root Cause Analysis**
For each FP, ask:
- Why did the lint fire incorrectly?
- Is this a pattern we can detect and exclude?
- Can we refine the heuristic?
- Should we add configuration options?

**Phase 3: Implement Refinements**

**Example: `excessive_token_abilities` Refinement**

```rust
// Current: Flags ANY struct with copy + drop
// Problem: Event structs legitimately have these abilities

// Refinement:
fn is_event_struct(struct_name: &str, file_path: &Path) -> bool {
    // 1. File name contains "event" or "events"
    file_path.to_str().map_or(false, |s| 
        s.contains("event") || s.contains("events")
    )
    ||
    // 2. Struct name ends with "Event" or "Emitted"
    struct_name.ends_with("Event") || 
    struct_name.ends_with("Emitted") ||
    struct_name.ends_with("Updated") ||
    struct_name.ends_with("Created")
}

// Usage in lint:
if has_copy && has_drop && !is_event_struct(name, file) {
    // Flag as excessive abilities
}
```

**Example: `missing_access_control` Refinement**

```rust
// Current: Excludes getters by function name prefix
// Problem: Some getter patterns not covered

// Refinement: Add more patterns
fn is_getter_function(func_name: &str, func_body: &T::FunctionBody) -> bool {
    // Existing checks
    let name_suggests_getter = func_name.starts_with("get_") ||
        func_name.starts_with("is_") ||
        func_name.starts_with("has_") ||
        func_name.starts_with("view_") ||
        func_name.starts_with("check_");
    
    // New: Check if function only reads fields (no assignments)
    let only_reads = !contains_assignment(func_body);
    
    // New: Check if return type is non-unit (likely a getter)
    let returns_value = !is_unit_return(func_body);
    
    name_suggests_getter || (only_reads && returns_value)
}
```

#### Refinement Targets by FP Rate

**If FP > 30%: Demote to Research or Skip**
- Too noisy for practical use
- Requires major redesign or more evidence

**If FP 20-30%: Add Configuration Options**
```toml
[lints.missing_access_control]
level = "warn"
exclude_getters = true           # Default: true
require_cap_keyword = ["cap", "Cap", "admin", "witness"]
```

**If FP 10-20%: Refine Heuristics**
- Add more exclusion patterns
- Improve detection logic
- Stay in Preview tier

**If FP < 10%: Promote to Stable**
- Document known edge cases
- Enable by default
- Add to stable tier

#### Expected Refinement Outcomes

**Pessimistic Case:**
- `oracle_zero_price`: 35% FP → Demote to Research
- `missing_access_control`: 25% FP → Add config options
- `unused_return_value`: 18% FP → Refine heuristics

**Optimistic Case:**
- `oracle_zero_price`: 15% FP → Refine heuristics, keep in Preview
- `missing_access_control`: 12% FP → Refine, consider for Stable
- `unused_return_value`: 8% FP → **Promote to Stable!**

**Realistic Case:**
- 1-2 lints refined and kept in Preview
- 1 lint promoted to Stable
- 0-1 lints demoted to Research

---

## Proposed Implementation Timeline

### Week 1-2: Ecosystem Validation Infrastructure
- [ ] Create `tests/ecosystem_validation/` directory structure
- [ ] Write `run_all.sh` script to execute linter on all repos
- [ ] Design triage spreadsheet/TOML for FP/TP classification
- [ ] Run initial pass on 5 repos (AlphaLend, Scallop, Suilend, DeepBook, Cetus)

### Week 3-4: Manual Triage & Metrics
- [ ] Manually classify all findings from fast lints
- [ ] Run semantic lints (requires full compilation - may hit errors)
- [ ] Calculate FP rates for all lints
- [ ] Generate metrics report: `ECOSYSTEM_VALIDATION_REPORT.md`

### Week 5: FP Refinement
- [ ] Root cause analysis for high-FP lints
- [ ] Implement refinements for 2-3 lints
- [ ] Re-run ecosystem validation to measure improvement
- [ ] Update stability classifications

### Week 6: Auto-Fix Implementation
- [ ] Implement auto-fix for `modern_method_syntax`
- [ ] Implement auto-fix for `merge_test_attributes`
- [ ] Test auto-fixes on ecosystem repos
- [ ] Measure auto-fix accuracy (% that compile without errors)

### Week 7: Documentation & Release
- [ ] Update `SECURITY_LINTS.md` with ecosystem validation results
- [ ] Document auto-fix capabilities in README
- [ ] Create `ECOSYSTEM_VALIDATION_REPORT.md`
- [ ] Tag v0.4.0 release

---

## Success Criteria

### Minimum Viable (Ship v0.4.0)
- ✅ At least 5 repos validated
- ✅ FP rates measured for all Preview lints
- ✅ 1-2 lints promoted to Stable or refined
- ✅ Ecosystem validation report published

### Target (Ideal v0.4.0)
- ✅ All 14 repos validated
- ✅ 2-3 lints promoted to Stable
- ✅ 3+ lints with auto-fix
- ✅ FP rates < 15% for all Preview lints

### Stretch Goals (v0.5.0+)
- ✅ 10+ lints with auto-fix
- ✅ Pre/post audit snapshot validation
- ✅ Automated FP regression testing in CI
- ✅ Published research paper on lint effectiveness

---

## Risk Assessment

### High Risk 🔴
1. **Compilation Errors on Ecosystem Repos**
   - Older repos may not compile with latest Sui framework
   - **Mitigation:** Pin to specific commit hashes, document failures

2. **Manual Triage Bottleneck**
   - Triaging 100+ findings manually is time-consuming
   - **Mitigation:** Prioritize high-value lints, use sampling for low-priority

3. **High FP Rates Invalidate Lints**
   - If FP > 30%, may need to discard months of work
   - **Mitigation:** Conservative heuristics already in place, expect 10-20% FP

### Medium Risk 🟡
1. **Auto-Fix Introduces Bugs**
   - Even "safe" auto-fixes could break code in edge cases
   - **Mitigation:** Extensive testing, conservative scope

2. **Insufficient Audit History**
   - Not all repos have clear pre/post audit commits
   - **Mitigation:** Focus on AlphaLend, Scallop, Bluefin with known audit history

### Low Risk 🟢
1. **Timeline Slippage**
   - Ecosystem validation may take longer than 2 weeks
   - **Mitigation:** Timebox to 4 weeks, ship with partial results

---

## Recommended Next Steps

### Immediate (This Week)
1. **Create ecosystem validation harness**
   - Write `run_all.sh` script
   - Design triage format (JSON or TOML)

2. **Pilot on 1 repo (AlphaLend)**
   - Run all lints
   - Manually triage findings
   - Validate that `droppable_hot_potato` fires on known bug

### Short-Term (Next 2 Weeks)
3. **Expand to 5 repos**
   - AlphaLend, Scallop, Suilend, DeepBook, Cetus
   - Focus on fast lints first (semantic lints may hit compilation issues)

4. **Calculate initial FP rates**
   - Generate metrics report
   - Identify 1-2 lints for refinement

### Medium-Term (Next 4-6 Weeks)
5. **Implement refinements**
   - Based on ecosystem data
   - Re-validate improved FP rates

6. **Add 2-3 auto-fixes**
   - Start with safe style lints
   - Test on ecosystem repos

7. **Publish ecosystem validation report**
   - Document methodology
   - Share FP rates and findings

---

## Conclusion

Move Clippy has a **solid foundation** with 52 lints, comprehensive testing, and audit-backed security rules. The remaining Phase 3 work focuses on **real-world validation** to:

1. **Measure actual FP rates** (not just theoretical estimates)
2. **Refine high-FP lints** to reduce noise
3. **Add auto-fixes** for common issues
4. **Promote proven lints** from Preview to Stable

**Recommended approach:** Start with a **pilot on 1-2 repos**, learn from that experience, then scale to all 14 repos. This de-risks the validation effort and allows for mid-course corrections.

**Estimated total effort:** 6-8 weeks for complete Phase 3 validation and refinement.

**Ship threshold:** Ecosystem validation on 5+ repos with documented FP rates is sufficient for a v0.4.0 release.
