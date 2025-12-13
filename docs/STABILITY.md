# Rule Stability Policy

Move Clippy uses a stability classification system inspired by [Ruff](https://docs.astral.sh/ruff/) to ensure high-quality, low-false-positive linting rules.

## Rule Groups

### Stable

**Definition:** Battle-tested rules with minimal false positives, enabled by default.

**Characteristics:**
- Low false-positive rate (< 1%)
- Clear, actionable error messages
- Well-documented behavior
- Consistent behavior across Move codebases
- FP prevention tests in `tests/false_positive_prevention.rs`

**Example usage:**
```bash
# Stable rules are enabled by default
move-clippy lint src/
```

### Preview

**Definition:** New rules that need community validation before becoming stable.

**Characteristics:**
- May have higher false-positive rates
- Behavior may change between versions
- Require explicit opt-in

**Example usage:**
```bash
# Enable preview rules via CLI
move-clippy lint --preview src/

# Or via config file (move-clippy.toml)
[lints]
preview = true
```

### Deprecated

**Definition:** Rules scheduled for removal in the next major version.

**Characteristics:**
- Emit warnings when explicitly enabled
- Will be removed in next major version
- Usually replaced by better alternatives

## Promotion Criteria: Preview → Stable

A rule can be promoted from Preview to Stable when it meets ALL of the following criteria:

### 1. Low False-Positive Rate
- < 1% false-positive rate across ecosystem test suite
- No open issues reporting false positives for > 2 weeks

### 2. Ecosystem Validation
- Run against major Move repositories without regressions:
  - deepbookv3
  - openzeppelin-sui
  - Other community projects
- Baseline established and maintained

### 3. Documentation
- Clear description of what the rule checks
- Examples of compliant and non-compliant code
- Explanation of why the pattern is problematic

### 4. User Feedback
- At least 2 weeks of preview availability
- Positive feedback from community users
- No major objections from Move experts

## Per-Lint FP Risk Assessment

### Security Lints (Stable)

| Lint | FP Risk | Detection Strategy | FP Prevention |
|------|---------|-------------------|---------------|
| `droppable_hot_potato` | **Zero** | Exact ability check | Name contains "hot_potato" AND has `drop` |
| `excessive_token_abilities` | **Low** | Ability + keyword + context | Filters event patterns (past-tense names) |
| `shared_capability` | **Low** | share_object + Cap keyword | Word boundary check (won't match "recap") |
| `stale_oracle_price` | **Zero** | Exact function name | `get_price_unsafe` only |
| `single_step_ownership_transfer` | **Low** | Function name + field assignment | Pattern match `.admin = ` |
| `missing_witness_drop` | **Zero** | OTW naming (ALL_CAPS) + no drop | Exact struct naming convention |
| `public_random_access` | **Low** | Public function + Random param | Type-based detection |

### Security Lints (Preview)

| Lint | FP Risk | Why Preview | Graduation Path |
|------|---------|-------------|-----------------|
| `pure_function_transfer` | **Medium** | May flag intentional internal transfers | Needs allowlist for common patterns |
| `unsafe_arithmetic` | **High** | Many arithmetic ops are safe | Needs data-flow analysis |
| `suspicious_overflow_check` | **Medium** | Heuristic-based detection | Needs more ecosystem validation |
| `unchecked_coin_split` | **Medium** | May flag checked splits | Needs balance tracking |
| `unbounded_vector_growth` | **Medium** | May flag bounded loops | Needs loop analysis |
| `hardcoded_address` | **Medium** | Test addresses are benign | Needs test context detection |

### Style/Modernization Lints (All Stable - Zero FP Risk)

| Lint | FP Risk | Reason |
|------|---------|--------|
| `modern_module_syntax` | **Zero** | Exact syntax pattern match |
| `modern_method_syntax` | **Zero** | Allowlisted function names only |
| `prefer_vector_methods` | **Zero** | Exact function signature match |
| `while_true_to_loop` | **Zero** | Exact `while (true)` pattern |
| `abilities_order` | **Zero** | Exact ability ordering check |
| `constant_naming` | **Zero** | Regex pattern for SCREAMING_SNAKE_CASE |

## Fix Safety

Auto-fixes are classified by their safety level:

### Safe Fixes

**Definition:** Fixes that preserve runtime behavior exactly.

**Characteristics:**
- Applied automatically with `--fix` (when implemented)
- No semantic changes
- Examples: formatting, import organization

### Unsafe Fixes

**Definition:** Fixes that may change runtime behavior.

**Characteristics:**
- Require `--unsafe-fixes` flag to apply
- May change error messages, side effects, or execution order
- Should be reviewed before committing

**Example:**
```bash
# Only apply safe fixes
move-clippy lint --fix src/

# Also apply unsafe fixes (review changes carefully!)
move-clippy lint --fix --unsafe-fixes src/
```

## Adding New Rules

When adding a new lint rule:

1. **Start in Preview:** All new rules begin in the `Preview` group
2. **Document thoroughly:** Include examples and rationale
3. **Test against ecosystem:** Establish baselines
4. **Monitor feedback:** Track issues and user reports
5. **Graduate when ready:** Promote to Stable after meeting criteria

### Code Example

```rust
// New rule starts as Preview
pub static MY_NEW_LINT: LintDescriptor = LintDescriptor {
    name: "my_new_lint",
    category: LintCategory::Style,
    description: "Description of what this rule checks",
    group: RuleGroup::Preview,  // Start in preview
    fix: FixDescriptor::none(), // Or FixDescriptor::safe("...") if auto-fixable
};
```

## Deprecation Policy

1. **Announce deprecation:** Mark rule as `Deprecated` with warning message
2. **Provide migration path:** Document alternative rules or patterns
3. **Grace period:** Keep deprecated rules for at least one minor version
4. **Remove:** Delete rule in next major version

## Version Guarantees

- **Patch versions (0.1.x):** Bug fixes only, no rule changes
- **Minor versions (0.x.0):** New preview rules, promotions, deprecations
- **Major versions (x.0.0):** Removal of deprecated rules, breaking changes

## Configuration Reference

```toml
# move-clippy.toml

[lints]
# Enable preview rules
preview = true

# Apply unsafe fixes (when --fix is used)
unsafe_fixes = true

# Disable specific lints
disabled = ["some_lint"]

# Override lint levels
modern_module_syntax = "error"
prefer_to_string = "warn"
```
