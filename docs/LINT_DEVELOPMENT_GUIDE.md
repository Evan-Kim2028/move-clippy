# Lint Development Guide

A comprehensive guide for contributing new lints to move-clippy.

## Table of Contents

1. [Quick Start](#quick-start)
2. [Lint Anatomy](#lint-anatomy)
3. [FP Prevention Checklist](#fp-prevention-checklist)
4. [Testing Requirements](#testing-requirements)
5. [Security Lint Requirements](#security-lint-requirements)
6. [Preview vs Stable](#preview-vs-stable)
7. [Code Review Checklist](#code-review-checklist)

---

## Quick Start

### 1. Choose Your Lint Mode

move-clippy has two lint modes:

| Mode | Speed | Accuracy | Use When |
|------|-------|----------|----------|
| **Fast** (tree-sitter) | 10ms | Pattern-based | Syntax patterns, naming conventions |
| **Semantic** (Move compiler) | 100ms+ | Type-aware | Type checking, value flow analysis |

**Rule of thumb:** Start with fast mode. Only use semantic if you need type information.

### 2. Create the Lint

```rust
// In src/rules/<category>.rs

/// Document what this lint detects and why it matters.
/// 
/// # Example
/// 
/// ```move
/// // BAD - explain why this is problematic
/// let x = bad_pattern();
/// 
/// // GOOD - explain the fix
/// let x = good_pattern();
/// ```
pub static MY_LINT: LintDescriptor = LintDescriptor {
    name: "my_lint",                         // snake_case
    category: LintCategory::Security,        // Security | Style | Conventions | TestQuality | Modernization
    description: "Brief description (see: audit source if security)",
    group: RuleGroup::Stable,                // Stable or Preview
    fix: FixDescriptor::none(),              // Or available("description")
};

pub struct MyLint;

impl LintRule for MyLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &MY_LINT
    }
    
    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        check_my_pattern(root, source, ctx);
    }
}

fn check_my_pattern(node: Node, source: &str, ctx: &mut LintContext<'_>) {
    // Pattern matching logic
    if node.kind() == "target_kind" {
        let text = node.utf8_text(source.as_bytes()).unwrap_or("");
        if is_violation(text) {
            ctx.report_node(
                &MY_LINT,
                node,
                "Explanation of the problem and suggested fix".to_string(),
            );
        }
    }
    
    // ALWAYS recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        check_my_pattern(child, source, ctx);
    }
}
```

### 3. Register the Lint

```rust
// In src/lint.rs

// Add to FAST_LINT_NAMES (or SEMANTIC_LINT_NAMES)
pub const FAST_LINT_NAMES: &[&str] = &[
    // ... existing lints ...
    "my_lint",
];

// Add to registry builder
"my_lint" => {
    reg = reg.with_rule(crate::rules::MyLint);
}

// Add to group mapping
"my_lint" => RuleGroup::Stable, // or Preview
```

### 4. Add Tests

```rust
// At the bottom of src/rules/<category>.rs

#[cfg(test)]
mod tests {
    use super::*;
    
    fn lint_source(source: &str) -> Vec<String> {
        let reg = LintRegistry::default_rules().with_rule(MyLint);
        let ctx = reg.lint(source);
        ctx.messages.into_iter().map(|m| m.message).collect()
    }
    
    #[test]
    fn test_my_lint_detected() {
        let source = r#"
            module example::demo {
                // BAD pattern here
            }
        "#;
        let messages = lint_source(source);
        assert!(!messages.is_empty());
        assert!(messages[0].contains("expected keyword"));
    }
    
    #[test]
    fn test_my_lint_ok_pattern() {
        let source = r#"
            module example::demo {
                // GOOD pattern here
            }
        "#;
        let messages = lint_source(source);
        assert!(messages.is_empty());
    }
}
```

---

## Lint Anatomy

### Descriptor Fields

| Field | Description | Requirements |
|-------|-------------|--------------|
| `name` | Unique identifier | snake_case, unique across all lints |
| `category` | Lint type | One of: Security, Style, Conventions, TestQuality, Modernization |
| `description` | Brief explanation | 10+ chars, security lints need "(see: source)" |
| `group` | Stability level | Stable (low FP risk) or Preview (experimental) |
| `fix` | Auto-fix info | `none()` or `available("description")` |

### Categories

| Category | Description | FP Tolerance |
|----------|-------------|--------------|
| **Security** | Potential vulnerabilities | Zero tolerance |
| **Style** | Code formatting | Very low |
| **Conventions** | Idioms and patterns | Low |
| **TestQuality** | Test best practices | Low |
| **Modernization** | Modern Move features | Very low |

---

## FP Prevention Checklist

Before submitting a lint, verify you've addressed false positive risks:

### 1. Pattern Specificity

- [ ] **Exact matches over partial:** Use `exact_name` not `name.contains("partial")`
- [ ] **Word boundaries:** Check `_cap` not `cap` (to avoid "recap", "escape")
- [ ] **Case sensitivity:** Be explicit about case matching
- [ ] **Context awareness:** Check surrounding code, not just the pattern

### 2. Common FP Patterns to Avoid

```rust
// ❌ BAD: Matches "recap", "escape", etc.
if name.to_lowercase().contains("cap") { ... }

// ✅ GOOD: Word boundary checking
fn is_capability(name: &str) -> bool {
    let lower = name.to_lowercase();
    // Check start or preceded by non-alpha
    if let Some(pos) = lower.find("cap") {
        let valid_prefix = pos == 0 || 
            !lower.chars().nth(pos - 1).unwrap().is_alphabetic();
        let valid_suffix = pos + 3 >= lower.len() || 
            !lower.chars().nth(pos + 3).unwrap().is_alphabetic();
        return valid_prefix && valid_suffix;
    }
    false
}
```

### 3. Context-Aware Filtering

For security lints, filter out benign patterns:

```rust
// Event structs are benign - don't flag them
const NON_ASSET_SUFFIXES: &[&str] = &[
    "event", "log", "created", "updated", "swapped",
];

fn is_benign_struct(name: &str) -> bool {
    let name_lower = name.to_lowercase();
    NON_ASSET_SUFFIXES.iter().any(|suffix| 
        name_lower.ends_with(suffix)
    )
}
```

### 4. FP Prevention Tests Required

Every lint MUST have tests in `tests/false_positive_prevention.rs`:

```rust
mod my_lint {
    #[test]
    fn ignores_benign_pattern_1() {
        // Test that DOESN'T fire
    }
    
    #[test]
    fn ignores_benign_pattern_2() {
        // Another edge case
    }
    
    #[test]
    fn detects_actual_violation() {
        // Ensure true positive still works
    }
}
```

---

## Testing Requirements

### Minimum Test Coverage

| Lint Type | Positive Tests | Negative Tests | FP Prevention |
|-----------|---------------|----------------|---------------|
| Security | ≥2 | ≥2 | ≥3 |
| Style | ≥1 | ≥1 | ≥1 |
| Conventions | ≥1 | ≥1 | ≥1 |
| Test Quality | ≥1 | ≥1 | ≥1 |
| Modernization | ≥1 | ≥1 | ≥1 |

### Test File Structure

```
tests/
├── false_positive_prevention.rs  # FP edge cases (MUST have)
├── lint_quality.rs               # Meta-tests on lints themselves
├── cli.rs                        # CLI integration tests
└── ecosystem_snapshots.rs        # Real-world code snapshots
```

### Running Tests

```bash
# All tests
cargo test

# Specific test file
cargo test --test false_positive_prevention

# Specific test
cargo test test_my_lint_detected
```

---

## Security Lint Requirements

Security lints have additional requirements:

### 1. Audit Source Citation

Every stable security lint MUST cite its source:

```rust
description: "Pattern enables X attack (see: Audit Name 2024, CVE-XXXX)",
```

### 2. Doc Comments with References

```rust
/// Detects vulnerable pattern X.
///
/// # Security References
///
/// - **Audit Name (YYYY-MM)**: "Finding Title"
///   URL: https://example.com/audit
///   Verified: YYYY-MM-DD
///
/// # Why This Matters
///
/// Explanation of the attack vector and consequences.
```

### 3. FP Risk Assessment

Document the FP risk level:

| Risk Level | Criteria | Action |
|------------|----------|--------|
| **Zero** | Exact pattern match, no ambiguity | → Stable |
| **Low** | Pattern + context check | → Stable |
| **Medium** | Heuristic-based detection | → Preview with --preview flag |
| **High** | Broad keyword matching | → Do not merge, refine first |

---

## Preview vs Stable

### When to Use Preview

Put lints in Preview when:

1. **Heuristic detection:** Uses keyword matching without full context
2. **New pattern:** Not yet validated against real-world code
3. **Known FP cases:** Has documented false positive scenarios
4. **Experimental:** Testing a new detection approach

### Graduation Criteria

Move from Preview → Stable when:

1. ✅ Zero FPs in ecosystem snapshot tests
2. ✅ ≥3 FP prevention tests covering edge cases
3. ✅ 30-day soak period with no user-reported FPs
4. ✅ Reviewed by security team (for security lints)

### Marking Preview Lints

```rust
pub static MY_EXPERIMENTAL_LINT: LintDescriptor = LintDescriptor {
    name: "my_experimental_lint",
    category: LintCategory::Security,
    description: "Description here",
    group: RuleGroup::Preview,  // ← Requires --preview flag
    fix: FixDescriptor::none(),
};
```

---

## Code Review Checklist

Before submitting a PR, verify:

### Lint Implementation

- [ ] Name is unique and snake_case
- [ ] Category matches lint purpose
- [ ] Description is meaningful (10+ chars)
- [ ] Security lints have audit citations
- [ ] Recursion into child nodes is present

### Testing

- [ ] Positive test (detection works)
- [ ] Negative test (doesn't fire on valid code)
- [ ] FP prevention tests in false_positive_prevention.rs
- [ ] Security lints: ≥3 FP prevention tests

### Documentation

- [ ] Doc comments explain the lint
- [ ] Example code shows bad/good patterns
- [ ] Security lints cite audit sources

### Registration

- [ ] Added to FAST_LINT_NAMES or SEMANTIC_LINT_NAMES
- [ ] Added to registry builder match arm
- [ ] Added to group mapping
- [ ] Group set correctly (Stable/Preview)

---

## Examples

### Good Lint Example: droppable_hot_potato

```rust
/// Detects hot potato structs with `drop` ability.
///
/// # Security References
/// - **Trail of Bits 2025**: Hot potato pattern analysis
///
/// # Why This Matters
/// Hot potatoes must NOT have drop, or they can be silently discarded.
pub static DROPPABLE_HOT_POTATO: LintDescriptor = LintDescriptor {
    name: "droppable_hot_potato",
    category: LintCategory::Security,
    description: "Hot potato struct has `drop` ability, enabling theft (see: Trail of Bits 2025)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
};
```

**Why it's good:**
- Clear description with audit citation
- Zero FP risk (exact pattern match on struct abilities)
- Comprehensive doc comments
- Proper group assignment (Stable)

### Lint That Needed FP Fixes: excessive_token_abilities

Original version flagged event structs like `AssetSwapEvent` as token violations.

**Fix:** Added context-aware filtering:
```rust
const NON_ASSET_SUFFIXES: &[&str] = &[
    "event", "created", "swapped", // ... etc
];
```

**Lesson:** Always consider naming conventions and context.

---

## Getting Help

- **Questions:** Open a GitHub discussion
- **Bugs:** File an issue with reproduction steps
- **FP Reports:** Include the code that triggered the false positive
