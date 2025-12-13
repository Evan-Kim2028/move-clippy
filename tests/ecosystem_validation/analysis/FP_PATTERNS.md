# False Positive Patterns

**Last Updated:** 2025-12-13  
**Purpose:** Document recurring FP patterns to improve lint rules

---

## Overview

This document tracks false positive patterns discovered during triage. Each pattern includes:
- Description of the FP
- Code examples
- Recommended fix for the lint

---

## Summary by Lint

| Lint | Total FPs | Primary Pattern | Recommendation |
|------|-----------|-----------------|----------------|
| event_suffix | TBD | DTOs with copy+drop | Exclude *Info, *Data structs |
| unbounded_vector_growth | TBD | Implicitly bounded | Detect nearby MAX_ asserts |
| hardcoded_address | TBD | Test files | Exclude test modules |
| ... | ... | ... | ... |

---

## Detailed Patterns

### event_suffix

**Total FPs:** TBD  
**Pattern:** Structs with `copy, drop` that are NOT events

#### Pattern 1: Data Transfer Objects

**Description:** DTOs used for returning multiple values from functions are flagged as missing Event suffix.

**Example:**
```move
// FP: PriceInfo is a DTO, not an event
public struct PriceInfo has copy, drop {
    price: u64,
    timestamp: u64,
    confidence: u64,
}

public fun get_price_info(): PriceInfo { ... }
```

**Why it's FP:** The struct is used as a return type, not emitted as an event.

**Recommendation:** 
- Exclude structs ending in `Info`, `Data`, `Result`, `Config`, `Params`
- Or: Check if struct is used as function return type (requires semantic analysis)

#### Pattern 2: Internal State Structs

**Description:** Internal structs used for temporary calculations.

**Example:**
```move
// FP: Calculation intermediate value
struct CalculationState has copy, drop {
    numerator: u128,
    denominator: u128,
}
```

**Recommendation:** Consider only flagging structs that are public

---

### unbounded_vector_growth

**Total FPs:** TBD  
**Pattern:** Vectors with implicit bounds

#### Pattern 1: User-Limited Collections

**Description:** Vectors are bounded by external constraints checked elsewhere.

**Example:**
```move
// FP: User can only have MAX_POSITIONS positions
// The check happens in a different function
public fun add_position(user: &mut User, pos: Position) {
    vector::push_back(&mut user.positions, pos);  // Flagged
}

// But this exists elsewhere:
const MAX_POSITIONS: u64 = 10;
public fun can_add_position(user: &User): bool {
    vector::length(&user.positions) < MAX_POSITIONS
}
```

**Why it's FP:** The bound exists, just not in the same function.

**Recommendation:**
- Look for `MAX_` constants in the same module
- Check for `length() <` patterns nearby
- Consider making this an "informational" rather than warning

#### Pattern 2: Admin-Only Functions

**Description:** Functions that can only be called by trusted admins.

**Example:**
```move
// FP: Only admin can call this
public fun admin_add_asset(cap: &AdminCap, assets: &mut Assets, asset: Asset) {
    vector::push_back(&mut assets.list, asset);  // Flagged
}
```

**Recommendation:** Exclude functions with capability parameters

---

### hardcoded_address

**Total FPs:** TBD  
**Pattern:** Legitimate use of literal addresses

#### Pattern 1: Test Files

**Description:** Test files legitimately use hardcoded addresses.

**Example:**
```move
#[test_only]
module test::my_tests {
    const TEST_ADMIN: address = @0x123;  // Flagged
    
    #[test]
    fun test_something() {
        let sender = @0xCAFE;  // Flagged
    }
}
```

**Recommendation:** Exclude `#[test_only]` modules and files in `tests/` directory

#### Pattern 2: Well-Known System Addresses

**Description:** References to well-known framework addresses.

**Example:**
```move
// Referencing Sui system address
const SUI_SYSTEM: address = @0x2;  // Flagged but legitimate
```

**Recommendation:** Allowlist `@0x0`, `@0x1`, `@0x2`, `@0x3` (system addresses)

---

### excessive_token_abilities

**Total FPs:** TBD  
**Pattern:** Internal structs with copy+drop that aren't tokens

#### Pattern 1: Receipt/Proof Structs

**Description:** Internal proof-of-action structs that need copy+drop.

**Example:**
```move
// FP: Receipt for tracking, not a token
public struct DepositReceipt has copy, drop {
    pool_id: ID,
    amount: u64,
    timestamp: u64,
}
```

**Recommendation:** 
- Only flag if struct name contains "Token", "Coin", "Balance"
- Or: Exclude if struct doesn't have `store` ability

---

## How to Add New Patterns

When you mark a finding as `false_positive`:

1. Add detailed notes explaining why:
   ```bash
   move-clippy triage update <id> --status false_positive \
     --notes "DTO struct used as return type, not an event"
   ```

2. Run the FP analysis script:
   ```bash
   python scripts/analyze_fp.py triage.json -o analysis/FP_PATTERNS_AUTO.md
   ```

3. Update this document with the new pattern and recommendation.

4. Consider opening an issue/PR to fix the lint.

---

## Lint Improvement Tracking

| Lint | Pattern | Issue/PR | Status |
|------|---------|----------|--------|
| event_suffix | DTO exclusion | #TBD | Proposed |
| unbounded_vector_growth | MAX_ detection | #TBD | Proposed |
| hardcoded_address | Test exclusion | #TBD | Proposed |

---

## Change Log

| Date | Changes |
|------|---------|
| 2025-12-13 | Initial template created |
| ... | ... |
