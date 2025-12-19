# Semantic Module Split Plan

## Overview

This document outlines the plan to split `src/semantic.rs` (3,907 lines) into a modular structure.

**Branch:** `refactor/split-semantic-module`
**Status:** Prepared, ready for implementation

## Target Structure

```
src/semantic/
├── mod.rs                    # Public API + lint_package orchestration
├── descriptors.rs            # 27 LintDescriptor statics + registry
├── util.rs                   # Shared helpers (diag_from_loc, push_diag, etc.)
└── lints/
    ├── mod.rs               # Re-exports all lint functions
    ├── sui_delegated.rs     # lint_sui_visitors()
    ├── ability.rs           # droppable_hot_potato_v2, copyable_capability, droppable_capability
    ├── capability.rs        # capability_transfer_v2, shared_capability_object, capability_transfer_literal_address
    ├── entry.rs             # entry_function_returns_value, private_entry_function
    ├── value_flow.rs        # unused_return_value, share_owned_authority
    ├── event.rs             # event_emit_type_sanity
    ├── random.rs            # public_random_access_v2
    ├── witness.rs           # missing_witness_drop_v2, generic_type_witness_unused
    ├── oracle.rs            # stale_oracle_price_v2
    ├── fungible.rs          # non_transferable_fungible_object
    └── iteration.rs         # mut_key_param_missing_authority, unbounded_iteration_over_param_vector
```

## File Line Mappings (from semantic.rs)

### descriptors.rs (Lines 1-604)
- Lines 1-17: Module-level comments and imports
- Lines 28-117: Sui-delegated lint descriptors (9 descriptors)
- Lines 118-540: Security lint descriptors (18 descriptors)
- Lines 545-604: DESCRIPTORS array and registry functions

### util.rs (Lines 831-930)
- `convert_compiler_diagnostic()` - Lines 834-866
- `diag_from_loc()` - Lines 868-887
- `push_diag()` - Lines 889-915
- `position_from_byte_offset()` - Lines 917-930

### lints/ability.rs (Lines 1055-1305)
- `lint_droppable_hot_potato_v2()` - Lines 1055-1138
- `lint_copyable_capability()` - Lines 1140-1215
- `lint_droppable_capability()` - Lines 1217-1305

### lints/capability.rs (Lines 2220-2768)
- `lint_shared_capability_object()` - Lines 2220-2430
- `lint_capability_transfer_literal_address()` - Lines 2431-2590
- `lint_capability_transfer_v2()` - Lines 2591-2768

### lints/entry.rs (Lines 2770-3023)
- `lint_entry_function_returns_value()` - Lines 2770-2890
- `lint_private_entry_function()` - Lines 2892-3023

### lints/value_flow.rs (Lines 3025-3260)
- `lint_unused_return_value()` - Lines 3025-3150
- `lint_share_owned_authority()` - Lines 3152-3260

### lints/event.rs (Lines 3261-3403)
- `lint_event_emit_type_sanity()` - Lines 3261-3403

### lints/random.rs (Lines 3405-3516)
- `lint_public_random_access_v2()` - Lines 3405-3516

### lints/witness.rs (Lines 3518-3714)
- `lint_missing_witness_drop_v2()` - Lines 3518-3588
- `lint_generic_type_witness_unused()` - Lines 3589-3714

### lints/oracle.rs (Lines 3716-3895)
- `lint_stale_oracle_price_v2()` - Lines 3716-3895
- `ORACLE_MODULES` constant

### lints/fungible.rs (Lines 1307-1500)
- `lint_non_transferable_fungible_object()` - Lines 1307-1500

### lints/iteration.rs (Lines 1502-2218)
- `lint_mut_key_param_missing_authority()` - Lines 1502-1800
- `lint_unbounded_iteration_over_param_vector()` - Lines 1802-2218

### lints/sui_delegated.rs (Lines 3895-?)
- `lint_sui_visitors()` - Sui compiler integration

## Implementation Strategy

### Phase 1: Create Skeleton (Completed)
Files created but reverted for now:
- `semantic/mod.rs`
- `semantic/descriptors.rs`
- `semantic/util.rs`
- `semantic/lints/mod.rs`

### Phase 2: Migrate Descriptors
1. Move all 27 `pub static LintDescriptor` definitions to `descriptors.rs`
2. Move `DESCRIPTORS` array
3. Move `descriptors()` and `find_descriptor()` functions
4. Update `mod.rs` to re-export

### Phase 3: Migrate Utilities
1. Move `convert_compiler_diagnostic()`
2. Move `diag_from_loc()`
3. Move `push_diag()`
4. Move `position_from_byte_offset()`
5. Move `append_unfulfilled_expectations()`

### Phase 4: Migrate Lint Functions (One file at a time)
For each lint file:
1. Create the file with proper imports
2. Move lint function(s)
3. Update `lints/mod.rs` to export
4. Update `mod.rs` to call the lint
5. Run tests to verify
6. Commit

### Phase 5: Cleanup
1. Remove old `semantic.rs`
2. Update any remaining references
3. Run full test suite
4. Run clippy

## Common Imports Needed

### For descriptors.rs
```rust
use crate::lint::{
    AnalysisKind, FixDescriptor, LintCategory, LintDescriptor, RuleGroup, TypeSystemGap,
};
```

### For util.rs
```rust
use crate::diagnostics::{Diagnostic, Position, Span};
use crate::level::LintLevel;
use crate::lint::{LintDescriptor, LintSettings};
use move_compiler::shared::files::MappedFiles;
use move_ir_types::location::Loc;
```

### For lint modules
```rust
use crate::diagnostics::{Diagnostic, Span};
use crate::lint::{LintDescriptor, LintSettings, RuleGroup};
use crate::level::LintLevel;
use super::util::{diag_from_loc, push_diag};
use super::descriptors::*;  // Specific descriptors needed
use move_compiler::typing::ast as T;
use move_compiler::shared::files::MappedFiles;
use move_compiler::shared::program_info::TypingProgramInfo;
use move_compiler::parser::ast::TargetKind;
```

## Testing Strategy

After each lint migration:
1. `cargo check --features full`
2. `cargo test --features full` (specific tests for that lint)
3. `cargo clippy --features full -- -D warnings`

## Notes

- The `#[cfg(feature = "full")]` gating should be at the module level where possible
- Most lint functions take `&mut Vec<Diagnostic>, &LintSettings, &MappedFiles` plus AST/info
- Some lints use `TypingProgramInfo`, others use `T::Program` directly
- The `append_unfulfilled_expectations` function is complex and should stay in mod.rs

## Estimated Time

- Phase 1: 30 min (completed)
- Phase 2: 30 min
- Phase 3: 30 min
- Phase 4: 3-4 hours (11 lint files)
- Phase 5: 30 min
- Total: ~5-6 hours of focused work
