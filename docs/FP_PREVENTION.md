# False Positive Prevention

This document describes the strategies and infrastructure used to prevent false positives in move-clippy.

## Overview

False positives (FPs) undermine trust in linting tools. A single FP can cause developers to:
1. Ignore the lint entirely (`#[allow(...)]` everywhere)
2. Stop using the tool
3. Miss real issues buried among noise

Our goal: **Zero false positives for stable lints.**

## FP Risk Categories

| Risk Level | Definition | Strategy | Examples |
|------------|------------|----------|----------|
| **Zero** | Exact pattern match, no ambiguity | Direct to Stable | `while true {}` → `loop {}` |
| **Low** | Pattern + context validation | Stable with FP tests | Ability ordering |
| **Medium** | Heuristic detection | Preview with --preview | Keyword-based security checks |
| **High** | Broad pattern matching | DO NOT SHIP | `*contains("admin")*` |

## Infrastructure

### 1. False Positive Prevention Tests

Location: `tests/false_positive_prevention.rs`

These tests verify lints DON'T fire on valid code:

```rust
mod security_shared_capability {
    #[test]
    fn ignores_capacity_not_capability() {
        // "capacity" contains "cap" but isn't a capability
        let source = r#"
            module example::storage {
                public fun check_capacity(capacity: u64) {
                    transfer::share_object(Storage { capacity });
                }
            }
        "#;
        let messages = lint_source(source);
        assert!(messages.is_empty());
    }
    
    #[test]
    fn ignores_recap_not_capability() {
        // "recap" contains "cap" but isn't a capability
        // ...
    }
}
```

**Requirements:**
- Every security lint MUST have ≥3 FP prevention tests
- Every stable lint MUST have ≥1 FP prevention test
- Tests should cover common edge cases

### 2. Lint Quality Self-Tests

Location: `tests/lint_quality.rs`

Meta-tests that verify lints follow best practices:

```rust
#[test]
fn all_security_lints_have_source_citations() {
    // Security lints must cite their audit source
}

#[test]
fn all_stable_lints_have_fp_prevention_tests() {
    // Check that FP tests exist
}

#[test]
fn all_lint_names_are_snake_case() {
    // Naming consistency
}
```

### 3. Ecosystem Snapshot Tests

Location: `tests/ecosystem_snapshots.rs` (planned)

Run lints against real-world Move codebases and snapshot the results:

```rust
#[test]
fn openzeppelin_sui_no_new_findings() {
    // Compare current findings against approved baseline
    insta::assert_snapshot!(lint_repo("openzeppelin-sui"));
}
```

**Benefits:**
- Catch regressions when lint behavior changes
- Validate against production code patterns
- Track FP history over time

### 4. CI Pipeline

Location: `.github/workflows/lint-quality.yml`

Automated checks on every PR:
- FP prevention tests pass
- Lint quality self-tests pass
- No regressions in ecosystem snapshots

## FP Prevention Strategies

### 1. Exact Pattern Matching

**Zero FP Risk** - Match exactly what you want to find:

```rust
// ✅ GOOD: Exact function name
if func_name == "get_price_unsafe" { ... }

// ❌ BAD: Substring matching
if func_name.contains("unsafe") { ... }  // Matches "mark_as_unsafe", etc.
```

### 2. Word Boundary Checking

**Low FP Risk** - Use word boundaries for keyword detection:

```rust
// ❌ BAD: Matches "recap", "escape", "handicap"
if name.to_lowercase().contains("cap") { ... }

// ✅ GOOD: Check word boundaries
fn is_capability_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    for suffix in &["cap", "capability"] {
        let mut pos = 0;
        while let Some(idx) = lower[pos..].find(suffix) {
            let actual_idx = pos + idx;
            
            // Check before: must be start or non-alpha
            let valid_before = actual_idx == 0 || 
                !lower.chars().nth(actual_idx - 1).unwrap().is_alphabetic();
            
            // Check after: must be end or non-alpha
            let after_idx = actual_idx + suffix.len();
            let valid_after = after_idx >= lower.len() || 
                !lower.chars().nth(after_idx).unwrap().is_alphabetic();
            
            if valid_before && valid_after {
                return true;
            }
            pos = actual_idx + 1;
        }
    }
    false
}
```

### 3. Context-Aware Filtering

**Low-Medium FP Risk** - Consider surrounding context:

```rust
// Event structs are benign, even with "token" abilities
const NON_ASSET_SUFFIXES: &[&str] = &[
    "event", "log", "key", "info", "params",
    "created", "updated", "swapped", "minted",  // Past-tense = event
];

fn is_likely_event(struct_name: &str) -> bool {
    let lower = struct_name.to_lowercase();
    NON_ASSET_SUFFIXES.iter().any(|s| lower.ends_with(s))
}
```

### 4. Preview Gate for Risky Lints

**Medium FP Risk** - Require explicit opt-in:

```rust
pub static SUSPICIOUS_LINT: LintDescriptor = LintDescriptor {
    name: "suspicious_lint",
    group: RuleGroup::Preview,  // Won't run without --preview
    // ...
};
```

## Common FP Patterns and Fixes

### Pattern: Keyword Substring Matching

**Problem:** `name.contains("admin")` matches "administer", "administration"

**Fix:** Word boundary checking or allowlist of exact names

### Pattern: Ability Detection

**Problem:** Flagging `copy + drop` on event structs

**Fix:** Filter out structs with event naming patterns

### Pattern: Function Name Heuristics

**Problem:** `set_admin` flagged for ownership transfer

**Fix:** Check for actual ownership-changing patterns (field assignment)

### Pattern: Capability Detection

**Problem:** `capacity`, `recap`, `escape` flagged as capabilities

**Fix:** Word boundary checking on both sides of "cap"

## Testing Your FP Prevention

Before submitting a lint:

1. **Write the FP prevention tests FIRST** (TDD approach)
2. Make the tests fail by checking they would catch the FP
3. Implement the fix
4. Verify tests pass

```bash
# Run just FP tests
cargo test --test false_positive_prevention

# Run specific lint's FP tests
cargo test security_my_lint --test false_positive_prevention
```

## Graduation from Preview to Stable

Lints start in Preview and graduate to Stable when:

1. ✅ Zero FPs in 30-day soak period
2. ✅ Zero FPs in ecosystem snapshot tests
3. ✅ ≥3 FP prevention tests covering edge cases
4. ✅ Security team review (for security lints)

## Reporting False Positives

If you encounter a false positive:

1. **File an issue** with:
   - The Move code that triggered the FP
   - The lint name
   - Expected behavior

2. **We will:**
   - Add an FP prevention test
   - Fix the lint
   - Consider moving to Preview if FP risk is too high

## Summary

| Strategy | FP Risk Reduction | When to Use |
|----------|-------------------|-------------|
| Exact matching | Eliminates | Always prefer |
| Word boundaries | High | Keyword detection |
| Context filtering | High | Pattern + naming |
| Preview gate | Contains | Experimental lints |
| FP tests | Prevents regression | All lints |
| Ecosystem snapshots | Validates | All lints |
