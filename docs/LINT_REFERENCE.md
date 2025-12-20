# Move-Clippy Lint Reference

**Status:** Generated (do not edit by hand)

This file is generated from the unified lint registry.

Regenerate with:

```bash
cargo run --features full --bin gen_lint_reference > docs/LINT_REFERENCE.md
```

## Summary

- Total: 82
- Stable: 51
- Preview: 8
- Experimental: 20
- Deprecated: 3

## Lints

| Lint | Tier | Phase | Category | Analysis | Requires | Description |
|------|------|-------|----------|----------|----------|-------------|
| `abilities_order` | stable | syntactic | style | syntactic | `fast` | Struct abilities should be ordered: key, copy, drop, store |
| `admin_cap_position` | stable | syntactic | style | syntactic | `fast` | Capability parameters should be first (or second after TxContext) |
| `capability_antipatterns` | stable | semantic | security | type-based | `--mode full` | Capability struct has copy ability, public constructor, or missing key - security vulnerability (type-based) |
| `capability_leak` | deprecated | syntactic | security | syntactic | `--experimental` | [DEPRECATED] Superseded by capability_transfer_v2 which uses type-based detection |
| `capability_transfer_literal_address` | preview | semantic | security | type-based | `--mode full --preview` | Capability-like object transferred to a literal address - likely authorization leak (type-based, preview) |
| `capability_transfer_v2` | experimental | semantic | security | type-based | `--mode full --experimental` | Capability transferred to non-sender address (type-based, requires --mode full --experimental) |
| `coin_field` | stable | semantic | suspicious | type-based | `--mode full` | [Sui Linter] Use Balance instead of Coin in struct fields (from sui_mode::linters) |
| `collection_equality` | stable | semantic | suspicious | type-based | `--mode full` | [Sui Linter] Avoid equality checks on collections (from sui_mode::linters) |
| `constant_naming` | stable | syntactic | naming | syntactic | `fast` | Error constants should use EPascalCase; other constants should be SCREAMING_SNAKE_CASE |
| `copyable_capability` | stable | semantic | security | type-based | `--mode full` | Struct is key+store+copy - transferable authority/asset can be duplicated (type-based, zero FP) |
| `copyable_fungible_type` | experimental | semantic | security | type-based | `--mode full --experimental` | Copyable fungible value type can be duplicated (type-based, experimental) |
| `custom_state_change` | stable | semantic | suspicious | type-based | `--mode full` | [Sui Linter] Custom transfer/share/freeze should call private variants (from sui_mode::linters) |
| `destroy_zero_unchecked` | experimental | syntactic | security | syntactic | `--experimental` | destroy_zero called without verifying value is zero - may abort unexpectedly (needs CFG for low FP) |
| `destroy_zero_unchecked_v2` | preview | absint | security | type-based-cfg | `--mode full --preview` | destroy_zero called without verifying value is zero (CFG-aware, requires --mode full --preview) |
| `digest_as_randomness` | experimental | syntactic | security | syntactic | `--experimental` | tx_context::digest used as randomness source - predictable and manipulable (needs taint analysis for low FP) |
| `divide_by_zero_literal` | stable | syntactic | security | syntactic | `fast` | Division or modulo by literal zero - will always abort |
| `doc_comment_style` | stable | syntactic | style | syntactic | `fast` | Use `///` for doc comments, not `/** */` or `/* */` |
| `droppable_capability` | stable | semantic | security | type-based | `--mode full` | Struct is key+store+drop (and not copy) - transferable authority/asset can be silently discarded (type-based, zero FP) |
| `droppable_flash_loan_receipt` | experimental | semantic | security | type-based | `--mode full --experimental` | Function returns Coin/Balance with a droppable receipt struct (type-based, experimental) |
| `droppable_hot_potato_v2` | experimental | semantic | security | type-based | `--mode full --experimental` | Struct has only `drop` ability - likely a broken hot potato (type-based) |
| `empty_vector_literal` | stable | syntactic | modernization | syntactic | `fast` | Prefer `vector[]` over `vector::empty()` |
| `entry_function_returns_value` | stable | semantic | suspicious | type-based | `--mode full` | Entry function returns a value that will be discarded by the runtime (type-based) |
| `equality_in_assert` | stable | syntactic | style | syntactic | `fast` | Prefer `assert_eq!(a, b)` over `assert!(a == b)` for clearer failure messages |
| `error_const_naming` | stable | syntactic | style | syntactic | `fast` | Error constants should use EPascalCase (e.g., `ENotAuthorized`) |
| `event_emit_type_sanity` | stable | semantic | security | type-based | `--mode full` | Emitting non-event-like type via event::emit (type-based, requires --mode full) |
| `event_past_tense` | stable | semantic | style | type-based | `--mode full` | Event name uses present tense instead of past tense (type-based, requires --mode full) |
| `event_suffix` | stable | syntactic | naming | syntactic | `fast` | Event structs should end with `Event` suffix |
| `explicit_self_assignments` | stable | syntactic | style | syntactic | `fast` | Use `..` to ignore multiple struct fields instead of explicit `: _` bindings |
| `flashloan_without_repay` | experimental | cross-module | security | cross-module | `--mode full --experimental` | Flashloan borrowed but not repaid on all paths (type-based cross-module, requires --mode full --experimental) |
| `freeze_wrapped` | stable | semantic | suspicious | type-based | `--mode full` | [Sui Linter] Do not freeze objects containing wrapped objects (from sui_mode::linters) |
| `freezing_capability` | stable | semantic | suspicious | type-based | `--mode full` | [Sui Linter] Avoid freezing capability objects (from sui_mode::linters) |
| `fresh_address_reuse` | experimental | syntactic | security | syntactic | `--experimental` | fresh_object_address result appears to be reused - each UID needs a fresh address (needs usage tracking for low FP) |
| `fresh_address_reuse_v2` | preview | absint | security | type-based-cfg | `--mode full --preview` | fresh_object_address result used multiple times (CFG-aware, requires --mode full --preview) |
| `generic_type_witness_unused` | experimental | semantic | security | type-based | `--mode full --experimental` | Generic function takes TypeName witness but never uses it (type-based, experimental) |
| `ignored_boolean_return` | stable | syntactic | security | syntactic | `fast` | Boolean-returning function result is ignored, may indicate missing authorization check (see: Typus Finance hack) |
| `invalid_otw` | stable | semantic | security | type-based | `--mode full` | One-time witness violates Sui Adapter rules - has wrong abilities, fields, or is generic (type-based) |
| `manual_loop_iteration` | stable | syntactic | modernization | syntactic | `fast` | Prefer loop macros (`do_ref!`, `fold!`) over manual while loops with index |
| `manual_option_check` | stable | syntactic | modernization | syntactic | `fast` | Prefer option macros (`do!`, `destroy_or!`) over manual `is_some()` + `destroy_some()` patterns |
| `merge_test_attributes` | stable | syntactic | test_quality | syntactic | `fast` | Merge stacked #[test] and #[expected_failure] into a single attribute list |
| `missing_key` | stable | semantic | suspicious | type-based | `--mode full` | [Sui Linter] Shared/transferred object missing key ability (from sui_mode::linters) |
| `missing_witness_drop` | stable | syntactic | security | syntactic | `fast` | One-time witness struct missing `drop` ability (see: Sui OTW pattern docs) |
| `missing_witness_drop_v2` | stable | semantic | security | type-based | `--mode full` | OTW struct name doesn't match module name or missing drop (type-based, requires --mode full) |
| `modern_method_syntax` | stable | syntactic | modernization | syntactic | `fast` | Prefer Move 2024 method call syntax for common allowlisted functions |
| `modern_module_syntax` | stable | syntactic | modernization | syntactic | `fast` | Prefer Move 2024 module label syntax (module x::y;) over block form (module x::y { ... }) |
| `mut_key_param_missing_authority` | preview | semantic | security | type-based | `--mode full --preview` | Public entry takes &mut key object without explicit authority param (type-based, preview) |
| `non_transferable_fungible_object` | stable | semantic | security | type-based | `--mode full` | Struct is key without store but has copy/drop - incoherent non-transferable fungible object (type-based, zero FP) |
| `otw_pattern_violation` | experimental | syntactic | security | syntactic | `--experimental` | One-time witness type name doesn't match module name - will fail at runtime (needs better module name handling) |
| `phantom_capability` | experimental | absint | security | type-based-cfg | `--mode full --experimental` | Capability parameter unused or not validated - may be phantom security (type-based CFG-aware, requires --mode full --experimental) |
| `prefer_to_string` | stable | syntactic | style | syntactic | `fast` | Prefer b"...".to_string() over std::string::utf8(b"...") (import-only check) |
| `prefer_vector_methods` | stable | syntactic | modernization | syntactic | `fast` | Prefer method syntax on vectors (e.g., v.push_back(x), v.length()) |
| `private_entry_function` | stable | semantic | suspicious | type-based | `--mode full` | Private entry function is unreachable - remove `entry` or make it public (type-based) |
| `public_mut_tx_context` | stable | syntactic | modernization | syntactic | `fast` | TxContext parameters should be `&mut TxContext`, not `&TxContext` |
| `public_random` | stable | semantic | suspicious | type-based | `--mode full` | [Sui Linter] Random state should remain private (from sui_mode::linters) |
| `public_random_access` | stable | syntactic | security | syntactic | `fast` | Public function exposes Random object, enabling front-running (see: Sui randomness docs) |
| `public_random_access_v2` | stable | semantic | security | type-based | `--mode full` | Public function exposes sui::random::Random object - enables front-running (type-based, requires --mode full) |
| `pure_function_transfer` | experimental | syntactic | suspicious | syntactic | `--experimental` | Non-entry functions should not call transfer internally; return the object instead (experimental - many legitimate patterns) |
| `receipt_missing_phantom_type` | experimental | semantic | security | type-based | `--mode full --experimental` | Receipt returned without phantom coin type enables type confusion (type-based, experimental) |
| `redundant_self_import` | stable | syntactic | style | syntactic | `fast` | Use `pkg::mod` instead of `pkg::mod::{Self}` |
| `redundant_test_prefix` | stable | syntactic | test_quality | syntactic | `fast` | In `*_tests` modules, omit redundant `test_` prefix from test functions |
| `self_transfer` | experimental | semantic | suspicious | type-based | `--mode full --experimental` | [Sui Linter] Transferring object to self - consider returning instead (from sui_mode::linters) |
| `share_owned` | experimental | semantic | suspicious | type-based | `--mode full --experimental` | [Sui Linter] Possible owned object share (from sui_mode::linters) |
| `share_owned_authority` | experimental | semantic | security | type-based | `--mode full --experimental` | Sharing key+store object makes it publicly accessible - dangerous for authority objects (type-based) |
| `shared_capability_object` | preview | semantic | security | type-based | `--mode full --preview` | Capability-like object is shared - potential authorization leak (type-based, preview) |
| `single_step_ownership_transfer` | stable | syntactic | security | syntactic | `fast` | Single-step ownership transfer is dangerous - use two-step pattern (see: OpenZeppelin Ownable2Step) |
| `stale_oracle_price` | stable | syntactic | security | syntactic | `fast` | Using get_price_unsafe may return stale prices (see: Bluefin Audit 2024, Pyth docs) |
| `stale_oracle_price_v2` | stable | semantic | security | type-based | `--mode full` | Using get_price_unsafe from known oracle may return stale prices (type-based, requires --mode full) |
| `suspicious_overflow_check` | stable | syntactic | security | syntactic | `fast` | Manual overflow check detected - these are error-prone. Consider using built-in checked arithmetic (see Cetus $223M hack) |
| `tainted_transfer_recipient` | preview | absint | security | type-based-cfg | `--mode full --preview` | Entry function address parameter flows to transfer recipient without validation (type-based CFG-aware, requires --mode full --preview) |
| `test_abort_code` | stable | syntactic | test_quality | syntactic | `fast` | Avoid numeric abort codes in test assertions; they may collide with application error codes |
| `transitive_capability_leak` | experimental | cross-module | security | cross-module | `--mode full --experimental` | Capability leaks across module boundary (type-based cross-module analysis, requires --mode full --experimental) |
| `typed_abort_code` | stable | syntactic | style | syntactic | `fast` | Prefer named error constants over numeric abort codes |
| `unbounded_iteration_over_param_vector` | preview | semantic | security | type-based | `--mode full --preview` | Loop bound depends on vector parameter length - add explicit bound (type-based, preview) |
| `unchecked_coin_split` | deprecated | syntactic | security | syntactic | `--experimental` | [DEPRECATED] Sui runtime already enforces balance checks - coin::split panics on insufficient balance |
| `unchecked_division` | experimental | semantic | security | type-based | `--mode full --experimental` | Division without zero-check may abort transaction (type-based) |
| `unchecked_division_v2` | preview | absint | security | type-based-cfg | `--mode full --preview` | Division without zero-check (type-based CFG-aware, requires --mode full --preview) |
| `unchecked_withdrawal` | deprecated | syntactic | security | syntactic | `--experimental` | [DEPRECATED] Business logic bugs require formal verification, not linting - name-based heuristics have high FP rate |
| `unnecessary_public_entry` | stable | syntactic | modernization | syntactic | `fast` | Use either `public` or `entry`, but not both on the same function |
| `unneeded_return` | stable | syntactic | style | syntactic | `fast` | Avoid trailing `return` statements; let the final expression return implicitly |
| `unsafe_arithmetic` | experimental | syntactic | suspicious | syntactic | `--experimental` | Detect potentially unsafe arithmetic operations (experimental, requires dataflow analysis) |
| `unused_return_value` | experimental | semantic | security | type-based | `--mode full --experimental` | Important return value is ignored, may indicate bug (type-based) |
| `while_true_to_loop` | stable | syntactic | modernization | syntactic | `fast` | Prefer `loop { ... }` over `while (true) { ... }` |
| `witness_antipatterns` | stable | semantic | security | type-based | `--mode full` | Witness struct has copy/store/key ability or public constructor - may defeat proof pattern (type-based) |
