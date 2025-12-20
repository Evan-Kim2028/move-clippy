# Changelog

All notable changes to this project will be documented in this file.

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
