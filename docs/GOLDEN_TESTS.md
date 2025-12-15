# Golden Test Framework

The golden test framework provides systematic, comprehensive testing for all lint rules in move-clippy.

## Overview

Golden tests use the **positive/negative** pattern to verify lint behavior:

- **Positive tests** (`positive.move`) - Code that **SHOULD** trigger the lint
- **Negative tests** (`negative.move`) - Code that should **NOT** trigger the lint

This pattern ensures:
1. Lints correctly detect problematic code
2. Lints don't produce false positives on correct code
3. Zero false-positive rate for negative cases

## Directory Structure

```
tests/golden/
├── abilities_order/
│   ├── positive.move      # Should trigger lint
│   └── negative.move      # Should NOT trigger lint
├── empty_vector_literal/
│   ├── positive.move
│   └── negative.move
├── experimental/          # Experimental tier lints
│   ├── unchecked_coin_split/
│   │   ├── positive.move
│   │   └── negative.move
│   └── ...
└── ...
```

## Test Structure

Each test file follows this pattern:

### Positive Test Example

```move
// Golden test: lint_name - POSITIVE (should trigger lint)
// Description: Brief explanation of what pattern triggers the lint

module 0x1::test {
    // BAD: Explanation of why this is bad
    public fun bad_example() {
        // Code that should trigger the lint
    }

    // BAD: Another bad example
    public fun another_bad_example() {
        // More code that should trigger
    }
}
```

### Negative Test Example

```move
// Golden test: lint_name - NEGATIVE (should NOT trigger lint)
// Description: Brief explanation of correct patterns

module 0x1::test {
    // GOOD: Explanation of why this is correct
    public fun good_example() {
        // Code that should NOT trigger the lint
    }

    // GOOD: Another correct example
    public fun another_good_example() {
        // More correct code
    }
}
```

## Test Implementation

Golden tests are implemented in `tests/golden_tests.rs`:

```rust
#[test]
fn golden_lint_name_positive() {
    let engine = create_default_engine();
    let src = include_str!("golden/lint_name/positive.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "lint_name");

    assert!(
        !filtered.is_empty(),
        "lint_name should trigger on bad code.\nAll diagnostics: {}",
        format_diags(&diags)
    );
}

#[test]
fn golden_lint_name_negative() {
    let engine = create_default_engine();
    let src = include_str!("golden/lint_name/negative.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "lint_name");

    assert!(
        filtered.is_empty(),
        "lint_name should NOT trigger on good code.\nGot: {}",
        format_diags(&filtered.into_iter().cloned().collect::<Vec<_>>())
    );
}
```

## Experimental Lint Tests

Experimental lints require additional tests to verify tier gating:

```rust
#[test]
fn experimental_lint_name_not_enabled_by_default() {
    // Verify experimental lint does NOT fire with default engine
    let engine = create_default_engine();
    let src = include_str!("golden/experimental/lint_name/positive.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "lint_name");

    assert!(
        filtered.is_empty(),
        "lint_name should NOT fire without --experimental flag"
    );
}

#[test]
fn experimental_lint_name_positive() {
    // Verify lint fires with experimental engine
    let engine = create_experimental_engine();
    let src = include_str!("golden/experimental/lint_name/positive.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "lint_name");

    assert!(
        !filtered.is_empty(),
        "lint_name should trigger with --experimental flag"
    );
}
```

## Adding a New Golden Test

### Step 1: Create Test Files

```bash
mkdir -p tests/golden/my_new_lint
```

Create `tests/golden/my_new_lint/positive.move`:

```move
// Golden test: my_new_lint - POSITIVE (should trigger lint)
// Description: What this lint detects

module 0x1::test {
    // BAD: Why this is bad
    public fun bad_pattern() {
        // Code that should trigger
    }
}
```

Create `tests/golden/my_new_lint/negative.move`:

```move
// Golden test: my_new_lint - NEGATIVE (should NOT trigger lint)
// Description: Correct patterns

module 0x1::test {
    // GOOD: Why this is correct
    public fun good_pattern() {
        // Code that should NOT trigger
    }
}
```

### Step 2: Add Test Functions

In `tests/golden_tests.rs`, add:

```rust
#[test]
fn golden_my_new_lint_positive() {
    let engine = create_default_engine();
    let src = include_str!("golden/my_new_lint/positive.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "my_new_lint");

    assert!(
        !filtered.is_empty(),
        "my_new_lint should trigger on problematic code.\nAll diagnostics: {}",
        format_diags(&diags)
    );
}

#[test]
fn golden_my_new_lint_negative() {
    let engine = create_default_engine();
    let src = include_str!("golden/my_new_lint/negative.move");
    let diags = engine.lint_source(src).expect("linting should succeed");
    let filtered = filter_lint(&diags, "my_new_lint");

    assert!(
        filtered.is_empty(),
        "my_new_lint should NOT trigger on correct code.\nGot: {}",
        format_diags(&filtered.into_iter().cloned().collect::<Vec<_>>())
    );
}
```

### Step 3: Run Tests

```bash
cargo test --test golden_tests
```

## Coverage Summary

Run the summary test to see coverage across all golden tests:

```bash
cargo test --test golden_tests golden_test_summary -- --nocapture
```

Output shows:
- Number of positive triggers per lint
- Number of false positives (should be 0)
- False positive rate across all tests

## Best Practices

### 1. Cover Edge Cases

Include edge cases in both positive and negative tests:

```move
// Positive test edge cases
public struct EmptyStruct {}           // Empty struct
public struct SingleField { x: u64 }  // Single field
public struct ManyFields { ... }       // Many fields

// Negative test edge cases  
public struct ValidPattern { ... }    // Correct pattern
public struct BoundaryCase { ... }    // Boundary condition
```

### 2. Test Suppression

Include suppression examples in negative tests:

```move
// GOOD: Lint suppressed with allow attribute
#[allow(lint::my_new_lint)]
public fun intentionally_bad() {
    // This would normally trigger, but is suppressed
}
```

### 3. Document Why

Always explain WHY code is good or bad:

```move
// BAD: coin::split without balance check can abort unexpectedly
public fun bad_split(coin: &mut Coin<SUI>, amt: u64): Coin<SUI> {
    coin::split(coin, amt)
}

// GOOD: Balance check provides clear error message
public fun good_split(coin: &mut Coin<SUI>, amt: u64): Coin<SUI> {
    assert!(coin::value(coin) >= amt, E_INSUFFICIENT_BALANCE);
    coin::split(coin, amt)
}
```

### 4. Use Realistic Code

Write tests that resemble real-world Move code:

```move
// GOOD: Realistic function signature and logic
public fun withdraw(
    account: &mut Account,
    amount: u64,
    ctx: &TxContext
): Coin<SUI> {
    assert!(account.owner == ctx.sender(), E_NOT_OWNER);
    assert!(account.balance >= amount, E_INSUFFICIENT_BALANCE);
    account.balance = account.balance - amount;
    coin::take(&mut account.coin_balance, amount, ctx)
}
```

## Debugging Test Failures

### Positive Test Failing (Lint Not Triggering)

1. **Check lint implementation** - Is the pattern you expect actually detected?
2. **Verify test file syntax** - Does the Move code parse correctly?
3. **Check for suppressions** - Is there an accidental `#[allow(...)]` attribute?
4. **Run manually**:
   ```bash
   cargo run -- lint tests/golden/my_lint/positive.move
   ```

### Negative Test Failing (False Positive)

1. **This is a bug!** - Negative tests should NEVER trigger
2. **Check test file** - Is the code actually correct?
3. **Review lint logic** - Does the lint have a false positive?
4. **Update lint or test** - Fix the lint or adjust the test pattern

### Test Won't Compile

1. **Check Move syntax** - Use `module 0x1::test {  }` block syntax
2. **Check empty structs** - Use `{}` not `{ }` with newlines for some lints
3. **Verify imports** - Include necessary `use` statements

## Integration with CI

Golden tests run automatically in CI:

```bash
# All tests including golden
cargo test

# Golden tests only
cargo test --test golden_tests

# Specific golden test
cargo test --test golden_tests golden_my_new_lint
```

CI fails if:
- Any positive test doesn't trigger
- Any negative test produces false positives
- Experimental lint fires without flag

## Future Enhancements

Planned improvements:
- Auto-generation of test templates
- FP rate tracking per lint
- Ecosystem validation integration
- Snapshot testing for diagnostic messages
