# Move Clippy Architecture Review: Rust Best Practices Audit

**Date:** 2025-12-19  
**Reviewer:** Droid (Factory AI)  
**Version:** v0.1.0  
**Codebase Size:** ~19,300 lines of Rust

---

## Executive Summary

Move Clippy is a well-structured Rust project that follows many Rust idioms correctly. However, there are several opportunities to make it more "Rust-native" - improving type safety, error handling, API ergonomics, and maintainability.

**Priority Legend:**
- **P0 (Critical)**: Blocks production use, potential bugs, security concerns
- **P1 (High)**: Significant ergonomics/maintainability issues
- **P2 (Medium)**: Improvements aligned with Rust best practices

---

## P0: Critical Issues

### P0.1: Error Type Fragmentation

**Current State:** Mixed use of `anyhow::Result`, `ClippyResult<T>`, and `Result<T, MoveClippyError>`.

```rust
// src/lib.rs - uses anyhow
pub fn lint_source(&self, source: &str) -> Result<Vec<Diagnostic>> { ... }

// src/error.rs - defines custom error
pub type ClippyResult<T> = Result<T, MoveClippyError>;

// src/semantic.rs - uses anyhow
pub fn lint_package(...) -> anyhow::Result<Vec<Diagnostic>> { ... }
```

**Problem:** 
- Inconsistent error handling makes it hard to know what errors can be returned
- `MoveClippyError` exists but is barely used (only in error.rs macros)
- Library code using `anyhow` leaks implementation details

**Recommendation:**
```rust
// For library APIs (src/lib.rs, src/semantic.rs):
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("parse error: {0}")]
    Parse(#[from] ParseError),
    
    #[error("semantic analysis failed: {0}")]
    Semantic(#[from] SemanticError),
    
    #[error("configuration error: {0}")]
    Config(#[from] ConfigError),
    
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

// For binary (src/main.rs): anyhow is fine
fn main() -> anyhow::Result<ExitCode> { ... }
```

**Files to change:** `src/error.rs`, `src/lib.rs`, `src/semantic.rs`, `src/config.rs`

---

### P0.2: Large File: `src/semantic.rs` (3,900+ lines)

**Current State:** Single file contains 30+ lint implementations, visitors, and helpers.

**Problem:**
- Hard to navigate and maintain
- Merge conflicts likely when multiple contributors work on lints
- Compile times suffer (entire file recompiles for any change)

**Recommendation:** Split into a module structure:

```
src/semantic/
├── mod.rs              # Re-exports, lint_package() entry point
├── types.rs            # LintDescriptor definitions for semantic lints
├── visitors.rs         # Shared visitor patterns
├── sui_lints.rs        # Sui compiler delegated lints
├── security/
│   ├── mod.rs
│   ├── random.rs       # public_random_access_v2
│   ├── witness.rs      # missing_witness_drop_v2
│   └── oracle.rs       # stale_oracle_price_v2
├── capability/
│   ├── mod.rs
│   ├── copyable.rs
│   ├── droppable.rs
│   └── transfer.rs
└── value_flow/
    ├── mod.rs
    └── unused_return.rs
```

---

### P0.3: Missing `#[must_use]` on Important Return Values

**Current State:** Functions returning important values lack `#[must_use]`.

```rust
// src/lint.rs
pub fn lint_source(&self, source: &str) -> Result<Vec<Diagnostic>>

// src/config.rs
pub fn load_config(...) -> Result<Option<(PathBuf, MoveClippyConfig)>>
```

**Problem:** Callers can silently ignore results, leading to bugs.

**Recommendation:**
```rust
#[must_use = "diagnostics should be processed or reported"]
pub fn lint_source(&self, source: &str) -> Result<Vec<Diagnostic>>

#[must_use = "configuration may contain important settings"]
pub fn load_config(...) -> Result<Option<(PathBuf, MoveClippyConfig)>>
```

**Also add to:** `LintEngine::new()`, `LintRegistry::default_rules()`, `Diagnostic` constructors

---

## P1: High Priority Improvements

### P1.1: Implement Standard Traits Consistently

**Current State:** Some types missing standard trait implementations.

```rust
// src/diagnostics.rs
pub struct Diagnostic { ... }  // No PartialEq, Eq

// src/lint.rs
pub struct LintSettings { ... }  // No Debug on fields
```

**Recommendation:**
```rust
// Diagnostic should be comparable for testing
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic { ... }

// All public types should derive common traits
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LintSettings { ... }
```

**Standard trait checklist for public types:**
- `Debug` - Always (required for error messages)
- `Clone` - When ownership transfer is common
- `PartialEq/Eq` - For types used in tests or collections
- `Default` - When sensible default exists
- `Hash` - When used as map keys

---

### P1.2: Builder Pattern for Complex Constructors

**Current State:** `LintRegistry` uses chained `with_rule()` calls but `LintSettings` uses field assignment.

```rust
// Inconsistent construction patterns
let settings = LintSettings::default()
    .with_config_levels(levels)
    .disable(disabled);

// vs direct field access elsewhere
let ctx = LintContext::new(source, settings);
```

**Recommendation:** Standardize on builder pattern for configurable types:

```rust
pub struct LintEngineBuilder {
    registry: Option<LintRegistry>,
    settings: LintSettings,
}

impl LintEngineBuilder {
    pub fn new() -> Self { Self::default() }
    
    pub fn registry(mut self, registry: LintRegistry) -> Self {
        self.registry = Some(registry);
        self
    }
    
    pub fn settings(mut self, settings: LintSettings) -> Self {
        self.settings = settings;
        self
    }
    
    pub fn preview(mut self, enabled: bool) -> Self {
        // Configure preview mode
        self
    }
    
    pub fn build(self) -> Result<LintEngine, BuildError> {
        let registry = self.registry.unwrap_or_else(LintRegistry::default_rules);
        Ok(LintEngine { registry, settings: self.settings })
    }
}
```

---

### P1.3: Newtype Pattern for Domain Concepts

**Current State:** Raw strings used for semantic concepts.

```rust
// src/lint.rs
pub fn level_for(&self, lint_name: &str) -> LintLevel { ... }

// src/unified.rs
lints: HashMap<&'static str, UnifiedLint>,
```

**Problem:** Easy to pass wrong string, no compile-time validation.

**Recommendation:**
```rust
/// A validated lint name that exists in the registry.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LintName(String);

impl LintName {
    pub fn new(name: &str) -> Option<Self> {
        if unified_registry().get(name).is_some() {
            Some(Self(name.to_string()))
        } else {
            None
        }
    }
    
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// Now the API is type-safe
pub fn level_for(&self, lint: &LintName) -> LintLevel { ... }
```

---

### P1.4: Use `Cow<'_, str>` for Flexible Ownership

**Current State:** Many functions take `String` or `&str` inconsistently.

```rust
// src/diagnostics.rs
pub message: String,  // Always owned

// Some functions force cloning
fn report(&mut self, ..., message: impl Into<String>) { ... }
```

**Recommendation:** Use `Cow` for messages that might be static or owned:

```rust
use std::borrow::Cow;

pub struct Diagnostic {
    pub message: Cow<'static, str>,
    pub help: Option<Cow<'static, str>>,
}

// Zero-copy for static strings
ctx.report(lint, span, "static message");  // No allocation
ctx.report(lint, span, format!("dynamic {}", value));  // Allocation only when needed
```

---

### P1.5: Iterator Return Types Instead of `Vec`

**Current State:** Many functions return `Vec` when iteration would suffice.

```rust
// src/unified.rs
pub fn by_tier(&self, tier: RuleGroup) -> Vec<&UnifiedLint> { ... }
pub fn by_category(&self, category: LintCategory) -> Vec<&UnifiedLint> { ... }
```

**Problem:** Forces allocation even when caller just iterates.

**Recommendation:**
```rust
pub fn by_tier(&self, tier: RuleGroup) -> impl Iterator<Item = &UnifiedLint> {
    self.by_tier
        .get(&tier)
        .into_iter()
        .flatten()
        .filter_map(|n| self.lints.get(n))
}
```

---

## P2: Medium Priority Improvements

### P2.1: Documentation Improvements

**Current State:** Good doc comments on public items, but inconsistent.

**Missing:**
- `# Examples` sections on key APIs
- `# Errors` sections on fallible functions
- Module-level documentation in some files

**Recommendation:** Add to all public APIs:

```rust
/// Lint a single in-memory source string.
///
/// # Examples
///
/// ```
/// use move_clippy::{LintEngine, create_default_engine};
///
/// let engine = create_default_engine();
/// let diagnostics = engine.lint_source("module example::foo {}")?;
/// assert!(diagnostics.is_empty());
/// # Ok::<(), anyhow::Error>(())
/// ```
///
/// # Errors
///
/// Returns an error if the source cannot be parsed as valid Move syntax.
pub fn lint_source(&self, source: &str) -> Result<Vec<Diagnostic>>
```

---

### P2.2: Const Generics for Fixed Collections

**Current State:** Fixed-size arrays represented as slices.

```rust
const ORACLE_MODULES: &[(&str, &[&str])] = &[
    ("pyth", &["get_price_unsafe", "price_unsafe"]),
    ...
];
```

**This is fine**, but consider using const generics for type-level documentation of size:

```rust
// If the size is semantically meaningful
const ORACLE_PROVIDERS: [OracleProvider; 4] = [...];
```

---

### P2.3: Use `#[non_exhaustive]` on Public Enums

**Current State:** Enums can be exhaustively matched by external code.

```rust
pub enum RuleGroup {
    Stable,
    Preview,
    Experimental,
    Deprecated,
}
```

**Problem:** Adding variants is a breaking change.

**Recommendation:**
```rust
#[non_exhaustive]
pub enum RuleGroup {
    Stable,
    Preview,
    Experimental,
    Deprecated,
}
```

**Apply to:** `RuleGroup`, `AnalysisKind`, `LintCategory`, `TypeSystemGap`, `TriageStatus`

---

### P2.4: Clippy Allow Attributes Cleanup

**Current State:** Many clippy lints globally allowed in `lib.rs`.

```rust
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::should_implement_trait)]
// ... 8 more
```

**Recommendation:** Move to per-item allows with justification:

```rust
// lib.rs - remove global allows

// semantic.rs, where needed:
#[allow(clippy::too_many_arguments)]  // Move compiler callback signature
fn handle_typed_program(...) { ... }
```

This makes the codebase more lint-compliant over time.

---

### P2.5: Test Organization

**Current State:** Tests are well-organized but could use more structure.

**Recommendations:**
1. Add `#[cfg(test)]` modules in source files for unit tests
2. Use `mod tests` consistently
3. Add property-based tests with `proptest` for parsers
4. Add fuzzing targets in `fuzz/` directory

```rust
// In src/parser.rs
#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    
    proptest! {
        #[test]
        fn parse_never_panics(s: String) {
            let _ = parse_source(&s);  // Should return Err, not panic
        }
    }
}
```

---

### P2.6: Visibility Refinement

**Current State:** Many items are `pub` when they could be more restricted.

```rust
// src/lint.rs
pub(crate) fn effective_level_for_scopes(...) -> LintLevel { ... }  // Good!

// But some internal helpers are fully pub
pub fn all_known_lints() -> HashSet<&'static str> { ... }  // Should this be pub?
```

**Recommendation:** Audit and use:
- `pub(crate)` for crate-internal items
- `pub(super)` for module-internal items
- `pub` only for true public API

---

## Summary: Priority Action Items

### P0 (Do First)
1. Consolidate error types - Create proper `move_clippy::Error` enum
2. Split `semantic.rs` into a module structure
3. Add `#[must_use]` to key functions

### P1 (Next Sprint)
4. Implement standard traits (`Debug`, `Clone`, `PartialEq`) consistently
5. Add builder pattern for `LintEngine`
6. Introduce `LintName` newtype
7. Use `Cow<'_, str>` for messages
8. Return iterators instead of `Vec` where appropriate

### P2 (Ongoing)
9. Add doc examples and `# Errors` sections
10. Add `#[non_exhaustive]` to public enums
11. Clean up global clippy allows
12. Add property-based tests
13. Refine visibility (`pub(crate)`)

---

## Appendix: Files by Priority

| File | Size | Priority Issues |
|------|------|-----------------|
| `src/semantic.rs` | 3,901 | P0.2 (split) |
| `src/rules/security.rs` | 2,171 | P2.4 (clippy) |
| `src/lint.rs` | ~800 | P0.3, P1.1, P1.3 |
| `src/main.rs` | 1,391 | P0.1 (errors) |
| `src/unified.rs` | 472 | P1.5 (iterators) |
| `src/diagnostics.rs` | 60 | P1.1 (traits), P1.4 (Cow) |
| `src/error.rs` | 63 | P0.1 (consolidate) |
| `src/config.rs` | 75 | P0.3 (must_use) |

---

## References

- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Rust Design Patterns](https://rust-unofficial.github.io/patterns/)
- [Effective Rust](https://www.lurklurk.org/effective-rust/)
- [The Rust Performance Book](https://nnethercote.github.io/perf-book/)
