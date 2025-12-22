# AST Investigation Results

## Tree-Sitter Grammar Analysis

**Grammar Repository:** https://github.com/tzakian/tree-sitter-move  
**Commit:** `640ee15e4a7b0d09a4bc95dcc71336c28d97999b`  
**Tree-Sitter Version:** `0.20.10`

## Move 2024 Macro Support

✅ **CONFIRMED:** The grammar DOES support Move 2024 macro syntax!

### Macro AST Nodes

**Node Types:**
- `macro_call_expression` - Macro invocation (e.g., `assert!(...)`, `assert_eq!(...)`)
- `macro_module_access` - Macro name with `!` (e.g., `assert!`)
- `macro_function_definition` - Macro function definitions
- `macro_identifier` - Identifier ending with `!`

**Example AST for `assert!(x == y, 0)`:**
```
macro_call_expression  "assert!(x == y, 0)"
  macro_module_access  "assert!"
    module_access  "assert"
      identifier  "assert"
    !  "!"
  arg_list  "(x == y, 0)"
    (  "("
    binary_expression  "x == y"
      name_expression  "x"
      ==  "=="
      name_expression  "y"
    ,  ","
    number_expression  "0"
    )  ")"
```

## Comment Support

✅ **CONFIRMED:** Both comment types are parsed and preserved in AST!

### Comment AST Nodes

**Node Types:**
- `block_comment` - Block comments `/* ... */` and `/** ... */`
- `line_comment` - Line comments `//` and `///`

**Important:** Comments are treated as **extras** (siblings of other nodes, not children)

**Example AST:**
```
module_body  "{ ... }"
  block_comment  "/** JavaDoc style */"
  function_definition  "fun foo() {}"
  line_comment  "/// Triple slash"
  function_definition  "fun bar() {}"
```

Comments appear as **siblings** of functions, not as part of attributes or preceding nodes.

## Root Cause Analysis

### 1. `equality_in_assert` Lint

**Expected Node:** `macro_invocation` (❌ WRONG)  
**Actual Node:** `macro_call_expression` (✅ CORRECT)

**Fix Required:** Change lint to check for `macro_call_expression` instead of `macro_invocation`

### 2. `manual_option_check` Lint

**Expected Pattern:** `if_expression` with method calls  
**Actual Pattern:** Works correctly - this lint should work!

**Investigation Needed:** Why isn't it firing? Need to check test files.

### 3. `doc_comment_style` Lint

**Expected Node:** `block_comment` (✅ CORRECT)  
**Actual Behavior:** Comments are parsed correctly as `block_comment`

**Issue:** The lint uses `precedes_documentable_item()` which checks `next_sibling()`. This works because comments and functions are siblings in the module body.

**Investigation Needed:** Test files might not have valid syntax or comments might be stripped before parsing.

## Key Findings

1. ✅ Macros ARE supported - node type is `macro_call_expression`
2. ✅ Block comments ARE parsed - node type is `block_comment`  
3. ✅ Line comments ARE parsed - node type is `line_comment`
4. ⚠️ Comments are "extras" (siblings, not attributes)
5. ⚠️ Lints are looking for WRONG node types (`macro_invocation` vs `macro_call_expression`)

## Next Steps

1. Fix `equality_in_assert`: Change `macro_invocation` → `macro_call_expression`
2. Test `manual_option_check` with correct Move syntax
3. Test `doc_comment_style` with correct Move syntax
4. Update golden test files to use valid Move syntax

## Testing Tools

Created `dump_ast` binary for debugging:
```bash
cargo run --bin dump_ast -- file.move
```

Dumps complete AST tree with node types and text snippets.
