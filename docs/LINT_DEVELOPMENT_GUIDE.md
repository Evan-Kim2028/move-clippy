# Lint Development Guide

**Status:** Developer workflow (kept current)

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
    category: LintCategory::Security,        // Security | Suspicious | Style | Modernization | Naming | TestQuality
    description: "Brief description (see: audit source if security)",
    group: RuleGroup::Stable,                // Stable | Preview | Experimental | Deprecated
    fix: FixDescriptor::none(),              // Or FixDescriptor::safe("...") / FixDescriptor::unsafe_fix("...")
    analysis: AnalysisKind::Syntactic,        // Syntactic | TypeBased | TypeBasedCFG | CrossModule
    gap: None,                               // Some(TypeSystemGap::...) for security/suspicious families
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

Move Clippy uses a unified registry:

- **Phase I (fast / syntactic)**: register the `LintRule` in `src/unified.rs` by adding it to `build_syntactic_registry()`.
- **Phase II (semantic / type-based)**: add the descriptor to the `DESCRIPTORS` slice in `src/semantic.rs`.
- **Phase III (CFG / AbsInt)**: add the descriptor to the `DESCRIPTORS` slice in `src/absint_lints.rs`.
- **Phase IV (cross-module)**: add the descriptor to the `DESCRIPTORS` slice in `src/cross_module_lints.rs`.

The unified registry is built automatically from these sources (`src/unified.rs:build_unified_registry()`), and the CLI (`list-rules`, `explain`) reads from it.

### 4. Add Tests

```rust
// At the bottom of src/rules/<category>.rs

#[cfg(test)]
mod tests {
    use super::*;
    
    fn lint_source(source: &str) -> Vec<String> {
        let reg = LintRegistry::new().with_rule(MyLint);
        let engine = LintEngine::new(reg);
        engine
            .lint_source(source)
            .expect("linting should succeed")
            .into_iter()
            .map(|d| d.message)
            .collect()
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
| `category` | Lint category | One of: security, suspicious, style, modernization, naming, test_quality |
| `description` | Brief explanation | 10+ chars, security lints need "(see: source)" |
| `group` | Stability level | Stable/Preview/Experimental/Deprecated |
| `fix` | Auto-fix metadata | `none()` / `safe(...)` / `unsafe_fix(...)` |
| `analysis` | Detection technique | Syntactic/TypeBased/TypeBasedCFG/CrossModule |
| `gap` | Type-system gap tag | Optional; used primarily for security/suspicious taxonomy |

### Categories

| Category | Description | FP Tolerance |
|----------|-------------|--------------|
| **Security** | Potential vulnerabilities | Zero tolerance |
| **Suspicious** | Potentially dangerous patterns | Very low |
| **Style** | Code clarity/idioms | Very low |
| **Modernization** | Modern Move features | Very low |
| **Naming** | Naming conventions | Very low |
| **TestQuality** | Test best practices | Low |

### Directives (testing and suppression)

Move Clippy supports allow/deny/expect directives on **lint names** and **lint categories** (e.g. `style`, `security`).

**Fast mode (tree-sitter):**

- `#[allow(lint::...)]` / `#![allow(lint::...)]`
- `#[deny(lint::...)]` / `#![deny(lint::...)]`
- `#[expect(lint::...)]` / `#![expect(lint::...)]` (emits `unfulfilled_expectation` if unmet)

Note: `#![...]` forms are treated as *move-clippy directives*, not Move language features. They may not compile under the Move compiler and are intended for fast-mode fixtures.

**Full mode (compiler-valid directives):**

Use `ext` attributes so packages still compile:

- `#[ext(move_clippy(allow(<name|category>)))]`
- `#[ext(move_clippy(deny(<name|category>)))]`
- `#[ext(move_clippy(expect(<name|category>)))]`

Attach them to the relevant item (function/struct/module). `expect` is treated as a testing invariant and overrides config-level `allow` so the expected lint still shows up in test output.

### Fixtures As Documentation

Prefer documenting lint behavior as runnable tests/fixtures (this is harder to let drift than prose):

- **Positive**: code that must trigger the lint
- **Negative**: code that must not trigger the lint
- **Directive coverage**: code that proves `allow/deny/expect` works (fast syntax or `ext(move_clippy(...))` in full mode)

See `tests/fixtures/README.md` for fixture layout and conventions.

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

Every lint MUST have explicit false-positive prevention coverage:

- **Positive**: a minimal case that must trigger.
- **Negative**: a minimal case that must not trigger.
- **Near miss**: a case that looks similar to the positive but must not trigger.

Where these tests live depends on the lint phase:

- **Fast mode (Phase I):** add unit tests colocated with the rule under `src/rules/` and/or a minimal syntactic snapshot fixture.
- **Full mode (Phases II–IV):** add a minimal fixture package under `tests/fixtures/phase{2,3,4}/...` and wire it into `tests/semantic_package_snapshots.rs` and/or add a spec invariant test in `tests/*_spec.rs`.

The guiding idea is “fixtures as documentation”: the most trustworthy documentation is code that compiles and is asserted on.

---

## Testing Requirements

### Minimum Test Coverage

| Lint Type | Positive Tests | Negative Tests | Near Misses |
|-----------|---------------|----------------|------------|
| Security | ≥2 | ≥2 | ≥3 |
| Suspicious | ≥1 | ≥1 | ≥2 |
| Style | ≥1 | ≥1 | ≥1 |
| Naming | ≥1 | ≥1 | ≥1 |
| Test Quality | ≥1 | ≥1 | ≥1 |
| Modernization | ≥1 | ≥1 | ≥1 |

### Test File Structure

```
tests/
├── syntactic_snapshots.rs            # tree-sitter snapshot tests (fast mode)
├── semantic_package_snapshots.rs     # compiler-based package snapshots (full mode)
├── *_spec.rs                         # spec-driven semantic invariants (full mode)
├── meta_invariants.rs                # meta-tests about the lint engine itself
├── ecosystem_snapshots.rs            # embedded fixtures + regression snapshots
└── support/                          # shared helpers for integration tests
```

Real-world ecosystem validation now lives in `../ecosystem-test-repos`.

### Running Tests

```bash
# Fast mode + unit tests
cargo test

# Full mode (semantic/CFG/cross-module)
cargo test --features full

# Update semantic snapshots (writes under tests/snapshots/)
INSTA_UPDATE=always cargo test --features full --test semantic_package_snapshots

# Specific test file
cargo test --features full --test semantic_package_snapshots

# Specific test
cargo test --features full spec_copyable_capability_exhaustive
```

For full-mode lints (Phases II–IV), prefer compiler-backed tests:

- `tests/semantic_package_snapshots.rs` for end-to-end fixture package snapshots.
- `tests/*_spec.rs` for spec-driven invariants (helpers live under `tests/support/`).

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

- [ ] Phase I: added to `src/unified.rs:build_syntactic_registry()` (fast lints only)
- [ ] Phase II/III/IV: added to that phase's `DESCRIPTORS` slice
- [ ] Tier set correctly (`RuleGroup::{Stable,Preview,Experimental,Deprecated}`)

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
