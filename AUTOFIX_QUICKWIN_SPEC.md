# Auto-Fix Quick Win Sprint - Implementation Specification

## 📋 Overview

This document specifies the implementation plan for 4 high-impact, low-complexity auto-fixes that will significantly improve move-clippy's automation capabilities.

**Target Lints:**
1. `prefer_vector_methods` - Function to method syntax for vectors
2. `modern_method_syntax` - Function to method syntax for 50+ stdlib functions
3. `public_mut_tx_context` - Add `mut` to TxContext parameters
4. `unnecessary_public_entry` - Remove redundant `public` modifier

**Success Criteria:**
- All 4 lints have working auto-fixes
- 100% test coverage (fix_tests + golden tests)
- Zero false positives in ecosystem validation
- All existing tests continue passing

---

## 🎯 1. prefer_vector_methods

### Current State
- ✅ Detection logic complete and accurate
- ✅ Already extracts receiver variable
- ❌ No auto-fix implementation

### Transformation Examples

#### Case 1: push_back
```move
// Before
vector::push_back(&mut v, item);

// After
v.push_back(item);
```

#### Case 2: length
```move
// Before
let len = vector::length(&v);

// After
let len = v.length();
```

### Implementation Strategy

**Step 1: Helper Function**
```rust
/// Generate method call fix for vector operations
fn generate_vector_method_fix(
    call_text: &str,
    receiver: &str,
    method_name: &str,
    remaining_args: Vec<&str>
) -> Option<Suggestion> {
    let args_str = if remaining_args.is_empty() {
        String::new()
    } else {
        remaining_args.join(", ")
    };
    
    let replacement = if args_str.is_empty() {
        format!("{}.{}()", receiver, method_name)
    } else {
        format!("{}.{}({})", receiver, method_name, args_str)
    };
    
    Some(Suggestion {
        message: format!("Use method syntax: {}", replacement),
        replacement,
        applicability: Applicability::MachineApplicable,
    })
}
```

**Step 2: Integration Points**

In `PreferVectorMethodsLint::check()`:
- Already parses `vector::push_back(&mut v, x)` → extracts `v` and `x`
- Already parses `vector::length(&v)` → extracts `v`
- Just need to call helper and report diagnostic with suggestion

**Step 3: Edge Cases**
- ✅ Handles complex receiver expressions (already validated via `parse_ref_mut_ident`)
- ✅ Preserves spacing in remaining arguments
- ❌ **Potential Issue**: Complex expressions like `vector::push_back(&mut get_vec(), x)`
  - **Solution**: Only apply fix when receiver is a simple identifier
  - Check: `is_simple_ident(receiver)` before generating fix

### Test Plan

**Fix Tests (tests/fix_tests.rs):**
```rust
#[test]
fn prefer_vector_methods_push_back() {
    let source = r#"
        module test::m {
            fun test(v: &mut vector<u64>, x: u64) {
                vector::push_back(v, x);
            }
        }
    "#;
    
    let fix = get_first_fix(source);
    assert!(fix.is_some());
    assert!(fix.unwrap().contains("v.push_back(x)"));
}

#[test]
fn prefer_vector_methods_length() {
    let source = r#"
        module test::m {
            fun test(v: &vector<u64>): u64 {
                vector::length(v)
            }
        }
    "#;
    
    let fix = get_first_fix(source);
    assert!(fix.is_some());
    assert!(fix.unwrap().contains("v.length()"));
}
```

**Golden Tests:**
- Already exist: `tests/golden/prefer_vector_methods/positive.move`
- Just need to verify auto-fix generates correct suggestions

**Ecosystem Validation:**
```bash
# Run against real codebases
./target/debug/move-clippy packages/openzeppelin-sui/**/*.move
./target/debug/move-clippy packages/gaussian/**/*.move
```

### Complexity Estimate
- **Time**: 2-3 hours
- **Difficulty**: Low (similar to existing auto-fixes)
- **Risk**: Low (transformation is pure syntactic)

---

## 🎯 2. modern_method_syntax

### Current State
- ✅ Detection logic complete with 50+ function allowlist
- ✅ Already parses call expressions
- ❌ No auto-fix implementation

### Transformation Examples

#### Case 1: Option methods
```move
// Before
option::is_some(&opt)

// After
opt.is_some()
```

#### Case 2: Multi-argument methods
```move
// Before
option::get_with_default(&opt, default_val)

// After
opt.get_with_default(default_val)
```

#### Case 3: Transfer operations
```move
// Before
transfer::transfer(obj, recipient)

// After
obj.transfer(recipient)
```

### Implementation Strategy

**Step 1: Reuse Vector Method Logic**

The algorithm is IDENTICAL to `prefer_vector_methods`:
1. Parse call expression: `module::function(arg1, arg2, ...)`
2. Extract receiver from first argument
3. Build method call: `arg1.function(arg2, ...)`

**Step 2: Helper Function** (can share with vector methods!)

```rust
/// Generate method call fix for any allowlisted function
fn generate_method_call_fix(
    module: &str,
    function: &str,
    receiver: &str,
    remaining_args: Vec<&str>
) -> Option<Suggestion> {
    // EXACTLY THE SAME as generate_vector_method_fix
    // Just different message format
    
    let args_str = if remaining_args.is_empty() {
        String::new()
    } else {
        remaining_args.join(", ")
    };
    
    let replacement = if args_str.is_empty() {
        format!("{}.{}()", receiver, function)
    } else {
        format!("{}.{}({})", receiver, function, args_str)
    };
    
    Some(Suggestion {
        message: format!("Use method syntax: {}", replacement),
        replacement,
        applicability: Applicability::MachineApplicable,
    })
}
```

**Step 3: Integration**

In `ModernMethodSyntaxLint::check()`:
- Already has `KNOWN_METHOD_TRANSFORMS` lookup table
- Already parses arguments
- Just need to extract receiver and call helper

**Step 4: Edge Cases**
- Complex receivers: Only apply to simple identifiers
- Nested expressions: Skip if receiver contains parentheses/brackets
- Method name conflicts: Already validated via allowlist

### Test Plan

**Fix Tests:**
```rust
#[test]
fn modern_method_syntax_option() {
    let source = r#"
        module test::m {
            use std::option::Option;
            fun test(opt: &Option<u64>): bool {
                option::is_some(opt)
            }
        }
    "#;
    
    let fix = get_first_fix(source);
    assert!(fix.unwrap().contains("opt.is_some()"));
}

#[test]
fn modern_method_syntax_transfer() {
    let source = r#"
        module test::m {
            fun test(obj: Obj, addr: address) {
                transfer::transfer(obj, addr);
            }
        }
    "#;
    
    let fix = get_first_fix(source);
    assert!(fix.unwrap().contains("obj.transfer(addr)"));
}
```

**Coverage**: Test 5-6 different modules from `KNOWN_METHOD_TRANSFORMS`

### Complexity Estimate
- **Time**: 2-3 hours
- **Difficulty**: Low (nearly identical to prefer_vector_methods)
- **Risk**: Low (allowlist is already validated)

---

## 🎯 3. public_mut_tx_context

### Current State
- ✅ Detection logic complete
- ✅ Already identifies exact problematic type node
- ❌ No auto-fix implementation

### Transformation Example

```move
// Before
public entry fun mint(ctx: &TxContext)

// After
public entry fun mint(ctx: &mut TxContext)
```

### Implementation Strategy

**Step 1: Helper Function**

```rust
/// Generate fix to add `mut` to TxContext reference
fn generate_mut_tx_context_fix(
    type_text: &str
) -> Option<Suggestion> {
    let trimmed = type_text.trim_start();
    
    // Pattern: &TxContext or & TxContext
    if !trimmed.starts_with('&') {
        return None;
    }
    
    let after_ref = trimmed[1..].trim_start();
    
    // Already has mut?
    if after_ref.starts_with("mut") {
        return None;
    }
    
    // Check it's actually TxContext
    if !after_ref.starts_with("TxContext") {
        return None;
    }
    
    // Insert "mut " after the "&"
    let replacement = format!("&mut {}", after_ref);
    
    Some(Suggestion {
        message: "Add `mut` to TxContext parameter".to_string(),
        replacement,
        applicability: Applicability::MachineApplicable,
    })
}
```

**Step 2: Integration**

In `PublicMutTxContextLint::check()`:
- Already finds the exact type node
- Already has `needs_mut_tx_context()` check
- Replace `ctx.report_node()` with `ctx.report_diagnostic()` + suggestion

**Step 3: Edge Cases**
- Spacing variations: `&TxContext` vs `& TxContext` vs `&  TxContext`
  - **Solution**: Use `trim_start()` to normalize spacing
- Module-qualified: `&sui::tx_context::TxContext`
  - **Solution**: Pattern match on "TxContext" anywhere in string
- Already has `mut`: Don't generate fix (edge case already handled)

### Test Plan

**Fix Tests:**
```rust
#[test]
fn public_mut_tx_context_simple() {
    let source = r#"
        module test::m {
            public entry fun foo(ctx: &TxContext) {}
        }
    "#;
    
    let fix = get_first_fix(source);
    assert!(fix.unwrap().contains("&mut TxContext"));
}

#[test]
fn public_mut_tx_context_with_spacing() {
    let source = r#"
        module test::m {
            entry fun bar(ctx: & TxContext) {}
        }
    "#;
    
    let fix = get_first_fix(source);
    assert!(fix.unwrap().contains("&mut"));
}

#[test]
fn public_mut_tx_context_qualified() {
    let source = r#"
        module test::m {
            public fun baz(ctx: &tx_context::TxContext) {}
        }
    "#;
    
    let fix = get_first_fix(source);
    assert!(fix.unwrap().contains("&mut tx_context::TxContext"));
}
```

### Complexity Estimate
- **Time**: 1-2 hours
- **Difficulty**: Very Low (simplest of all 4 fixes)
- **Risk**: Very Low (pure text insertion)

---

## 🎯 4. unnecessary_public_entry

### Current State
- ✅ Detection logic complete
- ✅ Already identifies functions with both modifiers
- ❌ No auto-fix implementation

### Transformation Example

```move
// Before
public entry fun mint(ctx: &mut TxContext)

// After
entry fun mint(ctx: &mut TxContext)
```

### Implementation Strategy

**Challenge**: We need to remove the `public` keyword from the function node, which requires AST node-level manipulation.

**Step 1: Locate `public` Token**

```rust
/// Find and extract the `public` modifier from function definition
fn find_public_modifier(node: Node, source: &str) -> Option<Node> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "visibility_modifier" {
            let text = slice(source, child);
            if text.trim() == "public" {
                return Some(child);
            }
        }
    }
    None
}
```

**Step 2: Generate Fix**

```rust
/// Generate fix to remove `public` modifier
fn generate_remove_public_fix(
    function_text: &str,
    public_node: Node,
    source: &str
) -> Option<Suggestion> {
    // Get the exact span of "public " (including trailing space)
    let public_text = slice(source, public_node);
    let public_start = function_text.find(public_text.trim())?;
    
    // Remove "public " by replacing it with empty string
    // Need to handle spacing carefully
    let before = &function_text[..public_start];
    let after_public = public_start + public_text.len();
    let after = &function_text[after_public..];
    
    // Skip whitespace after "public"
    let after_trimmed = after.trim_start();
    let replacement = format!("{}{}", before, after_trimmed);
    
    Some(Suggestion {
        message: "Remove redundant `public` modifier".to_string(),
        replacement,
        applicability: Applicability::MachineApplicable,
    })
}
```

**Step 3: Alternative Simpler Approach**

Since we're replacing the entire function node anyway, we can just do string manipulation:

```rust
fn generate_remove_public_fix_simple(
    function_text: &str
) -> Option<Suggestion> {
    // Find "public entry" and replace with just "entry"
    let replacement = function_text.replace("public entry", "entry");
    
    Some(Suggestion {
        message: "Remove redundant `public` modifier".to_string(),
        replacement,
        applicability: Applicability::MachineApplicable,
    })
}
```

**Step 4: Edge Cases**
- Spacing: `public  entry` (multiple spaces)
  - **Solution**: Regex or normalize to single space
- Comments between: `public /* comment */ entry`
  - **Risk**: Medium - might break
  - **Mitigation**: Use AST nodes instead of regex
- Newlines: `public\nentry`
  - **Solution**: Handle whitespace generically

### Test Plan

**Fix Tests:**
```rust
#[test]
fn unnecessary_public_entry_simple() {
    let source = r#"
        module test::m {
            public entry fun foo() {}
        }
    "#;
    
    let fix = get_first_fix(source);
    let fixed = fix.unwrap();
    assert!(!fixed.contains("public entry"));
    assert!(fixed.contains("entry fun"));
}

#[test]
fn unnecessary_public_entry_with_params() {
    let source = r#"
        module test::m {
            public entry fun bar(x: u64, ctx: &mut TxContext) {
                // body
            }
        }
    "#;
    
    let fix = get_first_fix(source);
    assert!(!fix.unwrap().contains("public entry"));
}
```

### Complexity Estimate
- **Time**: 2-3 hours
- **Difficulty**: Medium (needs careful whitespace handling)
- **Risk**: Medium (AST node manipulation is trickier)

---

## 🏗️ Implementation Order

### Phase 1: Foundation (30 minutes)
1. Add shared helper utilities to `src/rules/util.rs`:
   - `generate_method_call_fix()` (shared by #1 and #2)
   - `is_simple_receiver()` (safety check)

### Phase 2: Easy Wins (3-4 hours)
2. **public_mut_tx_context** (1-2h) - Simplest transformation
3. **prefer_vector_methods** (2h) - Uses shared helper
4. **modern_method_syntax** (2h) - Nearly identical to #3

### Phase 3: Moderate Challenge (2-3 hours)
5. **unnecessary_public_entry** (2-3h) - Needs AST node handling

### Phase 4: Testing & Validation (2-3 hours)
6. Write comprehensive fix_tests for all 4 lints
7. Update golden tests to verify suggestions
8. Ecosystem validation against real code
9. Fix any bugs found

**Total Estimated Time**: 8-10 hours

---

## 📊 Success Metrics

### Code Quality
- [ ] All 4 lints have `FixDescriptor::safe()` or `unsafe_fix()`
- [ ] Each lint has 2-3 fix_tests covering edge cases
- [ ] Golden tests updated with auto-fix validation
- [ ] Zero new warnings in `cargo build`

### Test Coverage
- [ ] All new tests passing (100%)
- [ ] All existing tests still passing (149/149)
- [ ] Ecosystem validation: 0 false positives
- [ ] Edge cases documented in test names

### Performance
- [ ] No performance regression (check with `cargo bench` if available)
- [ ] Fix generation adds <10ms overhead per diagnostic

### User Experience
- [ ] Auto-fix messages are clear and actionable
- [ ] Applicability correctly set (MachineApplicable vs MaybeIncorrect)
- [ ] Fixes preserve code formatting where possible

---

## 🚨 Risk Mitigation

### Risk 1: Complex Receiver Expressions
**Example**: `vector::push_back(&mut get_vec(), x)`

**Mitigation**:
- Only apply auto-fix when receiver is a simple identifier
- Check: `is_simple_ident(receiver)` before generating suggestion
- Document limitation in help text

### Risk 2: Whitespace Handling
**Example**: `public   entry` (multiple spaces)

**Mitigation**:
- Use AST nodes for token positions (not regex)
- Test with various spacing patterns
- Preserve original formatting where possible

### Risk 3: False Positives in Ecosystem Code
**Example**: Edge cases not covered by our test suite

**Mitigation**:
- Run against 5+ real codebases before finalizing
- Add any failures as regression tests
- Mark as `MaybeIncorrect` if uncertain

### Risk 4: Breaking Existing Tests
**Example**: Changes to shared utilities break other lints

**Mitigation**:
- Run full test suite after each lint implementation
- Keep changes isolated (no shared state)
- Use feature flags if needed for gradual rollout

---

## 🎯 Acceptance Criteria

### For Each Individual Lint:
✅ Fix generation helper function implemented  
✅ Descriptor updated to `FixDescriptor::safe()` or `unsafe_fix()`  
✅ Check method updated to call `ctx.report_diagnostic()` with suggestion  
✅ 2-3 fix_tests written and passing  
✅ Golden test verification complete  
✅ Ecosystem validation clean (zero false positives)  

### For Overall Sprint:
✅ All 4 lints complete  
✅ 149+ tests passing (no regressions)  
✅ Documentation updated (comments + help text)  
✅ Code review ready (clean diffs, clear commits)  
✅ Ready to merge to main branch  

---

## 📝 Implementation Checklist

### Pre-Work
- [ ] Review existing auto-fix implementations (equality_in_assert, manual_option_check)
- [ ] Understand fix_tests.rs helper utilities
- [ ] Set up ecosystem test repos for validation

### Implementation
- [ ] Create shared utilities in util.rs
- [ ] Implement public_mut_tx_context auto-fix
- [ ] Implement prefer_vector_methods auto-fix
- [ ] Implement modern_method_syntax auto-fix
- [ ] Implement unnecessary_public_entry auto-fix

### Testing
- [ ] Write fix_tests for all 4 lints (8-12 tests total)
- [ ] Update golden tests
- [ ] Run ecosystem validation
- [ ] Fix any bugs found
- [ ] Run full test suite

### Documentation
- [ ] Update lint descriptions with auto-fix info
- [ ] Add comments to complex logic
- [ ] Update CHANGELOG.md
- [ ] Prepare commit messages

### Review & Merge
- [ ] Self-review all changes
- [ ] Run `cargo clippy` and fix warnings
- [ ] Format code (`cargo fmt`)
- [ ] Create clean commits with detailed messages
- [ ] Push and verify CI passes

---

## 🎓 Lessons from Previous Auto-Fixes

### What Worked Well:
1. **Line-based parsing** for complex transformations (manual_option_check)
2. **Helper functions** that return `Option<Suggestion>` for clean integration
3. **Applicability levels** (MachineApplicable vs MaybeIncorrect) for user safety
4. **Comprehensive test coverage** catches edge cases early
5. **Ecosystem validation** finds real-world issues

### What to Avoid:
1. **Don't use regex** on complex AST structures - use tree-sitter nodes
2. **Don't assume spacing** - always trim/normalize
3. **Don't apply fixes blindly** - validate receiver is simple
4. **Don't skip edge cases** - add tests for weird spacing, comments, etc.
5. **Don't break existing tests** - run full suite after each change

### Best Practices:
1. Start with simplest lint first (public_mut_tx_context)
2. Build shared utilities for reusable logic
3. Test incrementally (don't write all 4 at once)
4. Validate against real code early and often
5. Keep commits atomic (one lint per commit)

---

## 🚀 Ready to Start!

This specification provides a clear roadmap for implementing 4 high-impact auto-fixes. The total effort is estimated at 8-10 hours with low risk and high user value.

**Next Step**: Begin with `public_mut_tx_context` as it's the simplest transformation and will establish patterns for the other 3 lints.
