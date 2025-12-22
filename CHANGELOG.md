# Changelog

All notable changes to this project will be documented in this file.

## [0.4.1] - 2025-12-22
### Deprecated
- `share_owned_authority`: High false positive rate (~78%). The ability pattern `key + store + !copy + !drop` describes ALL valuable Sui objects, not just capabilities. Use Sui's built-in `share_owned` lint which uses dataflow analysis.
- `shared_capability_object`: Same issue as `share_owned_authority`. Deprecated.
- `droppable_hot_potato_v2`: The pattern "only drop ability" matches many legitimate types (comparators, builders, rule markers). Use `droppable_flash_loan_receipt` instead for accurate flash loan detection.
- `receipt_missing_phantom_type`: The heuristic "function takes Coin<T> and returns something without T" flags many legitimate patterns (pool creation, positions, game outcomes). Use `droppable_flash_loan_receipt` for actual flash loan detection.

### Changed
- Promoted `entry_function_returns_value` from Preview to Stable (zero FP possible - pure type-based).
- Promoted `capability_transfer_literal_address` from Preview to Stable (very narrow scope - literal address recipients only).

## [0.2.0] - 2025-12-20
### Added
- Added semantic lints: `event_past_tense`, `invalid_otw`, `witness_antipatterns`, `capability_antipatterns`.
- Added experimental semantic lints: `droppable_flash_loan_receipt`, `receipt_missing_phantom_type`, `copyable_fungible_type`.
- Added taint analysis lint: `tainted_transfer_recipient`.
- Added syntactic style lint: `error_const_naming`.

### Changed
- Moved high-FP semantic lints to Experimental gating: `unchecked_division`, `unused_return_value`, `share_owned_authority`, `droppable_hot_potato_v2`.
- Fixed `unused_return_value` diagnostics to report the correct lint descriptor.

## [0.1.2] - 2025-12-20
### Changed
- Split semantic lints into focused modules with shared descriptors and utilities.
- Kept full-mode semantic lint orchestration intact with the new module layout.
