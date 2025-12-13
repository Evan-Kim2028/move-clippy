# Security Lints Reference

Move Clippy includes security lints based on real audit findings and published security research. These lints detect vulnerabilities that the Move compiler does not catch because they are semantic/intent issues rather than syntax errors.

## Overview

| Lint | Category | Detection Method | Source |
|------|----------|-----------------|--------|
| `droppable_hot_potato` | Security | Fast (tree-sitter) | Trail of Bits 2025, Mirage Audits 2025 |
| `excessive_token_abilities` | Security | Fast (tree-sitter) | Mirage Audits 2025, MoveBit 2023 |
| `shared_capability` | Security | Fast (tree-sitter) | OtterSec 2024, MoveBit 2023 |
| `stale_oracle_price` | Security | Fast (tree-sitter) | Bluefin Audit 2024 |
| `single_step_ownership_transfer` | Security | Fast (tree-sitter) | Bluefin Audit 2024 |
| `suspicious_overflow_check` | Security | Fast (tree-sitter) | Cetus $223M Hack 2024 |
| `unfrozen_coin_metadata` | Security | Semantic (--mode full) | MoveBit 2023 |
| `unused_capability_param` | Security | Semantic (--mode full) | SlowMist 2024 |

---

## Fast Lints (Syntax-based)

### `droppable_hot_potato`

**Severity:** Critical  
**Stability:** Stable  
**Auto-fix:** None

Detects flash loan receipts and hot potato structs with the `drop` ability.

#### Security References

| Source | Date | URL | Verification |
|--------|------|-----|--------------|
| Trail of Bits | 2025-09-10 | https://blog.trailofbits.com/2025/09/10/how-sui-move-rethinks-flash-loan-security/ | DeepBookV3 FlashLoan struct analysis |
| Mirage Audits | 2025-10-01 | https://www.mirageaudits.com/blog/sui-move-ability-security-mistakes | Production audit findings, "The Accidental Droppable Hot Potato" |
| Sui Official Docs | Current | https://docs.sui.io/standards/deepbookv3/flash-loans | Hot potato pattern specification |

#### Why This Matters

Adding `drop` to a hot potato silently breaks the security model. The compiler accepts it as valid syntax, but attackers can then borrow assets and simply drop the receipt without repaying.

#### Vulnerable Pattern

```move
// CRITICAL BUG - enables theft
struct FlashLoanReceipt has drop {
    pool_id: ID,
    amount: u64,
}

// Attacker can do this:
public fun exploit(pool: &mut Pool) {
    let (stolen_coins, receipt) = borrow(pool, 1_000_000);
    // Don't call repay - receipt gets dropped automatically!
    transfer::public_transfer(stolen_coins, @attacker);
}
```

#### Correct Pattern

```move
// No abilities = hot potato, must be consumed
struct FlashLoanReceipt {
    pool_id: ID,
    amount: u64,
}
```

#### Detection Keywords

The lint flags structs containing these keywords with `drop` ability:
- `receipt`, `loan`, `flash`, `promise`, `ticket`
- `potato`, `proof`, `obligation`, `voucher`, `claim`

---

### `excessive_token_abilities`

**Severity:** Critical  
**Stability:** Stable  
**Auto-fix:** None

Detects token/asset structs with both `copy` and `drop` abilities.

#### Security References

| Source | Date | URL | Verification |
|--------|------|-----|--------------|
| Mirage Audits | 2025-10-01 | https://www.mirageaudits.com/blog/sui-move-ability-security-mistakes | "The Ability Combination Nightmare" |
| MoveBit | 2023-07-07 | https://movebit.xyz/blog/post/Sui-Objects-Security-Principles-and-Best-Practices.html | "Avoid giving excessive abilities to structs" |

#### Why This Matters

A struct with both `copy` and `drop` can be:
1. **Duplicated infinitely** (via `copy`)
2. **Destroyed at will** (via `drop`)
3. **Created from thin air** by copying and modifying

This is the "infinite money glitch" for token implementations.

#### Vulnerable Pattern

```move
// CRITICAL VULNERABILITY - DO NOT USE
struct TokenCoin has copy, drop, store {
    amount: u64,
}

// Attacker can duplicate tokens:
let original = get_token();
let copy1 = original;  // copy happens
let copy2 = original;  // another copy
// Now attacker has 3x the tokens!
```

#### Correct Pattern

```move
// Assets should ONLY have key + store
struct TokenCoin has key, store {
    id: UID,
    balance: Balance,
}
```

#### Detection Keywords

The lint flags structs containing these keywords with both `copy` AND `drop`:
- `token`, `coin`, `asset`, `balance`, `share`
- `stake`, `bond`, `note`, `credit`, `fund`

---

### `shared_capability`

**Severity:** High  
**Stability:** Stable  
**Auto-fix:** None

Detects capability objects (AdminCap, TreasuryCap, etc.) being shared instead of transferred.

#### Security References

| Source | Date | URL | Verification |
|--------|------|-----|--------------|
| OtterSec | 2024 | Suilend audit report | Access control bypass findings |
| MoveBit | 2023-07-07 | https://movebit.xyz/blog/post/Sui-Objects-Security-Principles-and-Best-Practices.html | "Capabilities should not be shared" |

#### Why This Matters

Capabilities are used to gate privileged operations. If a capability is shared (publicly accessible), **anyone** can use it to perform admin actions. This defeats the entire purpose of capability-based access control.

#### Vulnerable Pattern

```move
// CRITICAL - Anyone can now mint tokens!
public fun init(witness: TOKEN, ctx: &mut TxContext) {
    let (treasury, metadata) = coin::create_currency(...);
    transfer::public_share_object(treasury);  // BAD - public minting!
}
```

#### Correct Pattern

```move
// Capability transferred to deployer
public fun init(witness: TOKEN, ctx: &mut TxContext) {
    let (treasury, metadata) = coin::create_currency(...);
    transfer::public_transfer(treasury, tx_context::sender(ctx));  // GOOD
}
```

---

### `stale_oracle_price`

**Severity:** High  
**Stability:** Stable  
**Auto-fix:** None

Detects use of `get_price_unsafe` oracle functions that return stale prices without timestamp validation.

#### Security References

| Source | Date | URL | Verification |
|--------|------|-----|--------------|
| Bluefin Audit | 2024-02 | MoveBit Audit Contest | Finding: "Oracle price can be stale" |
| Pyth Documentation | Current | https://docs.pyth.network/price-feeds/use-real-time-data/sui | "Always use `get_price_no_older_than`" |

#### Why This Matters

Stale oracle prices can lead to:
1. **Arbitrage**: Old prices allow risk-free profit
2. **Bad liquidations**: Users liquidated at wrong prices
3. **Protocol insolvency**: Undercollateralized loans

#### Vulnerable Pattern

```move
// BAD - Price could be hours or days old!
public fun get_value(price_info: &PriceInfoObject): u64 {
    let price = pyth::get_price_unsafe(price_info);
    price.price
}
```

#### Correct Pattern

```move
// GOOD - Price guaranteed fresh within 60 seconds
public fun get_value(price_info: &PriceInfoObject, clock: &Clock): u64 {
    let price = pyth::get_price_no_older_than(price_info, clock, 60);
    price.price
}
```

---

### `single_step_ownership_transfer`

**Severity:** Medium  
**Stability:** Stable  
**Auto-fix:** None

Detects single-step admin/owner transfer functions that immediately change ownership.

#### Security References

| Source | Date | URL | Verification |
|--------|------|-----|--------------|
| Bluefin Audit | 2024-02 | MoveBit Audit Contest | Finding: "Single-step admin transfer is dangerous" |
| OpenZeppelin | Best Practice | https://docs.openzeppelin.com/contracts/4.x/api/access#Ownable2Step | Two-step ownership pattern |

#### Why This Matters

Single-step ownership transfers are dangerous because:
1. **Typo risk**: Wrong address = permanent loss of control
2. **No recovery**: Once transferred, cannot undo
3. **Phishing**: Social engineering to transfer to attacker

#### Vulnerable Pattern

```move
// DANGEROUS - One typo and admin is lost forever!
public fun transfer_admin(exchange: &mut Exchange, new_admin: address) {
    exchange.admin = new_admin;
}
```

#### Correct Pattern

```move
// Two-step: propose, then accept
public fun propose_admin(exchange: &mut Exchange, new_admin: address) {
    exchange.pending_admin = option::some(new_admin);
}

public fun accept_admin(exchange: &mut Exchange, ctx: &TxContext) {
    assert!(option::is_some(&exchange.pending_admin), E_NO_PENDING);
    let pending = option::extract(&mut exchange.pending_admin);
    assert!(tx_context::sender(ctx) == pending, E_NOT_PENDING_ADMIN);
    exchange.admin = pending;
}
```

---

### `suspicious_overflow_check` ⚠️ Preview

**Severity:** Critical  
**Stability:** Preview (requires `--preview` flag)  
**Auto-fix:** None

Detects manual overflow check patterns that are error-prone.

#### Security References

| Source | Date | URL | Verification |
|--------|------|-----|--------------|
| Cetus $223M Hack | 2024-05 | https://www.halborn.com/blog/post/explained-the-cetus-exploit-may-2024 | Overflow check used wrong bit mask |
| SlowMist Analysis | 2024-05 | https://slowmist.medium.com/cetus-hack-analysis | "checked_shlw function had mask error" |

#### Why This Matters

The Cetus protocol lost $223M because their manual overflow check function had a bug. Manual bit manipulation for overflow detection is extremely error-prone and should use standard library functions.

#### Vulnerable Pattern

```move
// This is the ACTUAL BUG from Cetus (simplified)
public fun checked_shlw(n: u256, shift: u8): (u256, bool) {
    let mask = 0xffff...ffff << (256 - shift);  // WRONG MASK!
    if (n & mask != 0) {
        (0, true)  // overflow
    } else {
        (n << shift, false)
    }
}
```

#### Correct Pattern

```move
// Use standard library overflow-checked math
use std::u256::overflowing_mul;

public fun safe_mul(a: u256, b: u256): (u256, bool) {
    overflowing_mul(a, b)  // Battle-tested implementation
}
```

---

## Semantic Lints (Require `--mode full`)

These lints require full Move compilation and are only available with the `full` feature.

### `unfrozen_coin_metadata`

**Severity:** High  
**Stability:** Stable  
**Auto-fix:** None

Detects CoinMetadata being shared instead of frozen.

#### Security References

| Source | Date | URL | Verification |
|--------|------|-----|--------------|
| MoveBit | 2023-07-07 | https://movebit.xyz/blog/post/Sui-Objects-Security-Principles-and-Best-Practices.html | "The metadata in Coin should be frozen" |

#### Why This Matters

If CoinMetadata is shared instead of frozen, the admin can modify the token's name, symbol, and other metadata after creation. This can confuse users and enable phishing attacks.

#### Vulnerable Pattern

```move
public fun init(witness: MY_TOKEN, ctx: &mut TxContext) {
    let (treasury, metadata) = coin::create_currency(...);
    transfer::public_share_object(metadata);  // BAD - can be modified!
}
```

#### Correct Pattern

```move
public fun init(witness: MY_TOKEN, ctx: &mut TxContext) {
    let (treasury, metadata) = coin::create_currency(...);
    transfer::public_freeze_object(metadata);  // GOOD - immutable forever
}
```

---

### `unused_capability_param`

**Severity:** High  
**Stability:** Stable  
**Auto-fix:** None

Detects capability parameters that are passed but never used.

#### Security References

| Source | Date | URL | Verification |
|--------|------|-----|--------------|
| SlowMist | 2024-09-10 | https://github.com/slowmist/Sui-MOVE-Smart-Contract-Auditing-Primer | Section 8: "Permission Vulnerability Audit" |

#### Why This Matters

If a capability is passed to a function but never used, it indicates that the access control check is missing. Anyone can call the function by passing any capability object.

#### Vulnerable Pattern

```move
// Cap is passed but never checked - anyone can call this!
public fun admin_action(_cap: &AdminCap, pool: &mut Pool) {
    pool.value = 0;
}
```

#### Correct Pattern

```move
public fun admin_action(cap: &AdminCap, pool: &mut Pool) {
    assert!(cap.pool_id == object::id(pool), WRONG_CAP);  // Actually use the cap!
    pool.value = 0;
}
```

---

## Suppression

All security lints can be suppressed using the standard suppression mechanisms:

```move
// File-level suppression in Move 2024
#[allow(lint(droppable_hot_potato))]
module my_module;

// Or via config file (move-clippy.toml)
[lints]
droppable_hot_potato = "allow"
```

**Warning:** Suppressing security lints should be done with extreme caution and documented reasoning.

---

## Related Resources

### Sui Built-in Linters

These lints complement (do not duplicate) Sui's built-in compiler lints:
- `share_owned` - Sharing potentially owned objects
- `self_transfer` - Transferring to sender (prefer return)
- `custom_state_change` - Custom transfer/share/freeze calls
- `coin_field` - Use Balance instead of Coin in structs
- `freeze_wrapped` - Don't freeze objects with wrapped objects
- `collection_equality` - Don't compare collections
- `public_random` - Random state should be private
- `missing_key` - Objects need key ability
- `freezing_capability` - Don't store freeze capabilities

### External Audit Resources

- [SlowMist Sui Move Auditing Primer](https://github.com/slowmist/Sui-MOVE-Smart-Contract-Auditing-Primer)
- [MoveBit Security Best Practices](https://movebit.xyz/blog/post/Sui-Objects-Security-Principles-and-Best-Practices.html)
- [Sui Official Security Best Practices](https://blog.sui.io/security-best-practices/)
- [Trail of Bits Flash Loan Security](https://blog.trailofbits.com/2025/09/10/how-sui-move-rethinks-flash-loan-security/)
- [Mirage Audits Ability Mistakes](https://www.mirageaudits.com/blog/sui-move-ability-security-mistakes)
- [Cetus Hack Analysis (Halborn)](https://www.halborn.com/blog/post/explained-the-cetus-exploit-may-2024)
- [Pyth Oracle Documentation](https://docs.pyth.network/price-feeds/use-real-time-data/sui)

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 0.3.0 | 2025-12-13 | Added security lints: `shared_capability`, `stale_oracle_price`, `single_step_ownership_transfer`, `suspicious_overflow_check` (preview) |
| 0.2.0 | 2025-12-13 | Added security lints: `droppable_hot_potato`, `excessive_token_abilities`, `unfrozen_coin_metadata`, `unused_capability_param` |

## Contributing

If you find a new security pattern that should be detected, please:
1. Provide a published audit report or security research as the source
2. Include a minimal reproducing example
3. Open an issue or PR at the move-clippy repository
