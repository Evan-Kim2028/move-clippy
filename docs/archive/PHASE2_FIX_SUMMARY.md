# Phase 2: Fix Broken Lints - Summary

## Session Progress

**Test Results: 58/64 passing (90.6%)**

### ✅ Completed Fixes

#### 1. AST Node Type Corrections (`macro_invocation` → `macro_call_expression`)

**Root Cause:** Tree-sitter grammar uses `macro_call_expression` for Move 2024 macros, not `macro_invocation`.

**Files Modified:**
- `src/rules/modernization.rs` - `equality_in_assert` lint (line 42)
- `src/rules/test_quality.rs` - `test_abort_code` lint (line 39)
- `src/rules/style.rs` - `typed_abort_code` lint (line 476)
- `src/rules/security.rs` - `ignored_boolean_return` parent check (line 1176)

**Tests Fixed:**
- ✅ `golden_equality_in_assert_positive` - NOW PASSING
- ✅ `golden_equality_in_assert_negative` - NOW PASSING

#### 2. Lint Registry Additions

**Issue:** `unchecked_coin_split` missing from fast lint match statement

**Fix:** Added to `src/lint.rs` line ~911:
```rust
"unchecked_coin_split" => {
    reg = reg.with_rule(crate::rules::UncheckedCoinSplitLint);
}
```

#### 3. Experimental Lint Grouping

**Issue:** Three lints marked as `Deprecated` but should be `Experimental`

**Fix:** Moved to `RuleGroup::Experimental` in `get_lint_group()`:
- `unchecked_coin_split` - Name-based, high FP
- `capability_leak` - Name-based, needs type-based rewrite
- `unchecked_withdrawal` - Name-based, needs CFG-based rewrite

**Tests Fixed:**
- ✅ `experimental_unchecked_coin_split_not_enabled_by_default` - NOW PASSING
- ✅ `experimental_capability_leak_not_enabled_by_default` - NOW PASSING
- ✅ `experimental_unchecked_withdrawal_not_enabled_by_default` - NOW PASSING

---

## Remaining Failures (6 tests)

### Tests Not Firing (Lints Not Detecting Issues)

#### 1. `golden_doc_comment_style_positive` ❌
**Expected:** Lint should trigger on `positive.move`
**Issue:** Grammar DOES parse `/** */` comments as `block_comment` nodes
**Next Steps:** Need to investigate test file syntax or lint logic

#### 2. `golden_event_suffix_positive` ❌
**Expected:** Lint should trigger on `positive.move`
**Next Steps:** Check test file and lint pattern matching

#### 3. `golden_manual_option_check_positive` ❌
**Expected:** Lint should trigger on `positive.move`
**Note:** AST uses `if_expression` nodes (correct), likely test file syntax issue
**Next Steps:** Validate test file Move syntax

#### 4. `golden_public_mut_tx_context_negative` ❌
**Expected:** Lint should NOT trigger on `negative.move`
**Issue:** False positive - lint firing when it shouldn't
**Next Steps:** Check lint logic for over-detection

#### 5. `golden_redundant_test_prefix_positive` ❌
**Expected:** Lint should trigger on `positive.move`
**Next Steps:** Check pattern matching logic

#### 6. `golden_test_abort_code_positive` ❌
**Expected:** Lint should trigger on `positive.move`
**Note:** Fixed `macro_call_expression` node type, but still not detecting
**Next Steps:** Debug with `dump_ast` tool on positive.move file

---

## Key Discoveries

### 1. Tree-Sitter Grammar Capabilities (Verified)

✅ **Move 2024 Macros ARE Supported:**
- Node type: `macro_call_expression` (NOT `macro_invocation`)
- Macro name: `macro_module_access` with `!` suffix
- Arguments: `arg_list`

✅ **Comments ARE Parsed:**
- Block comments: `block_comment` node (includes `/* */` and `/** */`)
- Line comments: `line_comment` node (includes `//` and `///`)
- Comments are AST **siblings** of functions (not children)

### 2. AST Investigation Tools Created

**Tool:** `src/bin/dump_ast.rs`
**Usage:**
```bash
cargo run --bin dump_ast -- file.move
```

Dumps complete tree-sitter AST with node types and text snippets.

---

## Commits Made This Session

1. **`7abcc1d`** - Add AST investigation tools and documentation
   - Created `dump_ast` binary
   - Created `docs/AST_INVESTIGATION.md`
   - Identified root causes for all broken lints

2. **`cbacecb`** - Fix macro_invocation → macro_call_expression throughout codebase
   - Fixed 4 lint implementations
   - Added unchecked_coin_split to fast lint registry
   - **Result:** 55/64 tests passing (85.9%)

3. **`e5fb718`** - Move experimental lints from Deprecated to Experimental group
   - Fixed lint grouping for 3 lints
   - Changed RuleGroup::Preview to RuleGroup::Experimental
   - **Result:** 58/64 tests passing (90.6%)

---

## Next Steps (Phase 2 Continuation)

### Immediate Priorities

1. **Debug remaining 6 test failures** using `dump_ast` tool:
   - Run `dump_ast` on each positive/negative test file
   - Verify AST structure matches lint expectations
   - Fix test files if syntax is invalid
   - Fix lint logic if patterns are wrong

2. **Specific Investigation Needed:**

**`test_abort_code`:**
- Fixed `macro_call_expression` but still not detecting
- Likely checking wrong test context (module vs function)
- Check `is_inside_test_function()` logic

**`doc_comment_style`:**
- Comments ARE in AST as `block_comment`
- Check `precedes_documentable_item()` sibling logic
- May need to handle comment-function spacing

**`manual_option_check`:**
- AST shows `if_expression` with `is_some()` calls
- Check condition extraction and body pattern matching
- Verify `destroy_some()` detection

### Testing Strategy

For each failing test:
1. Run `dump_ast` on the test file
2. Manually trace through lint logic with AST structure
3. Add debug prints to identify where lint returns early
4. Fix either test file syntax or lint detection logic

---

## Success Metrics

**Current: 58/64 (90.6%)**
**Target: 64/64 (100%)**

**Blockers:** 6 tests requiring AST-level debugging

---

## Documentation Status

✅ Complete:
- `docs/AST_INVESTIGATION.md` - Root cause analysis
- `docs/PHASE2_FIX_SUMMARY.md` - This file

📋 TODO:
- Update `docs/GOLDEN_TESTS.md` after 100% pass rate achieved
- Update main `README.md` with new lint count and pass rate
