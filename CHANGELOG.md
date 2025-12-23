# Changelog

All notable changes to this project will be documented in this file.

## [0.5.1] - 2025-12-23
### Removed
- `divide_by_zero_literal`: Obvious issue - no developer writes `x / 0` intentionally.
- `destroy_zero_unchecked`: Obvious issue - needs CFG for accuracy.
- `event_suffix`: Not in Move Book (only past-tense naming is recommended).

### Deprecated
- `capability_antipatterns`: High false positive rate, deprecated.
- `stale_oracle_price_v2`: Superseded by `stale_oracle_price_v3`.
- `unchecked_division`: Experimental duplicate with high noise.

### Added
- Move Book citations to all lints backed by official documentation.

### Stats
- **56 total lints** (39 stable, 3 preview, 12 experimental, 2 deprecated)

## [0.5.0] - 2025-12-22
### Removed
Major cleanup removing 17 lints that were duplicating Move compiler checks, had high false positive rates, or were superseded by better implementations.

**From security.rs (9 lints):**
- `StaleOraclePriceLint`: Syntactic version superseded by CFG-based `stale_oracle_price_v3`.
- `SingleStepOwnershipTransferLint`: Pattern too narrow, many false negatives.
- `UncheckedCoinSplitLint`: Duplicates Move compiler checks.
- `MissingWitnessDropLint`: Superseded by `witness_antipatterns`.
- `PublicRandomAccessLint`: Duplicates the Move compiler's built-in `public_random` lint.
- `IgnoredBooleanReturnLint`: High false positive rate on legitimate patterns.
- `UncheckedWithdrawalLint`: Too many false positives without CFG analysis.
- `CapabilityLeakLint`: Superseded by `capability_transfer_literal_address`.
- `DigestAsRandomnessLint`: Rare pattern, low value.

**From modernization.rs (5 lints):**
- `UnnecessaryPublicEntryLint`: Subjective style preference.
- `PublicMutTxContextLint`: Duplicates Move compiler checks.
- `WhileTrueToLoopLint`: Trivial pattern, not worth linting.
- `PureFunctionTransferLint`: High false positive rate.
- `UnsafeArithmeticLint`: Too noisy without proper taint analysis.

**From absint_lints.rs (1 lint):**
- `TaintedTransferRecipient`: Dead code, 100% false positive rate.

**From semantic (2 lints):**
- `invalid_otw`: Duplicates Move compiler's OTW validation.
- `otw_pattern_violation`: Duplicates Move compiler's OTW validation.

### Stats
- 2,686 lines removed across 18 files
- **59 total lints** (41 stable, 3 preview, 13 experimental, 2 deprecated)

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
