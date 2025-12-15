# Move-Clippy Lint Reference

**Last Updated:** 2025-12-15  
**Purpose:** Single source of truth for all lints, their tiers, analysis types, and FP risk

## Quick Reference

**Total Lints:** 59  
**Stable:** 51 | **Preview:** 3 | **Experimental:** 12 | **Deprecated:** 3

## How to Use This Reference

**Find lints by tier:**
```bash
grep "| Stable |" docs/LINT_REFERENCE.md
grep "| Preview |" docs/LINT_REFERENCE.md
grep "| Experimental |" docs/LINT_REFERENCE.md
```

**Find lints by analysis type:**
```bash
grep "| Syntactic |" docs/LINT_REFERENCE.md
grep "| TypeBasedCFG |" docs/LINT_REFERENCE.md
```

**Find lints by gap category:**
```bash
grep "| ValueFlow |" docs/LINT_REFERENCE.md
grep "| CapabilityEscape |" docs/LINT_REFERENCE.md
```

---

## Analysis Types Explained

| Type | Speed | Accuracy | Requires | Description |
|------|-------|----------|----------|-------------|
| **Syntactic** | ⚡ Fast | Pattern-based | `.move` files only | Tree-sitter parsing, no type info |
| **TypeBased** | 🐢 Slower | Type-aware | `--mode full` | Uses Move compiler's type checker |
| **TypeBasedCFG** | 🐌 Slowest | Dataflow | `--mode full --preview` | Control flow + dataflow analysis |
| **CrossModule** | 🐌 Slowest | Call graph | `--mode full --preview` | Analyzes across module boundaries |

**Why CFG matters:** Syntactic lints use pattern matching (e.g., "does `destroy_zero` appear after `== 0`?"). CFG lints track values through control flow (e.g., "does the zero-check dominate the `destroy_zero` call on all paths?"). CFG lints have **near-zero FP** but require compilation.

---

## Tier 1: Stable Lints (51)

### Syntactic - Style & Conventions (28 lints)

| Lint | Gap | FP Risk | Description |
|------|-----|---------|-------------|
| `abilities_order` | StyleConvention | Low | Enforce canonical ability ordering (key, copy, drop, store) |
| `while_true_to_loop` | StyleConvention | Low | Prefer `loop` over `while (true)` |
| `modern_module_syntax` | StyleConvention | Low | Use modern `module` syntax |
| `redundant_self_import` | StyleConvention | Low | Remove unnecessary `Self` imports |
| `prefer_to_string` | StyleConvention | Low | Prefer `to_string()` over manual formatting |
| `constant_naming` | StyleConvention | Low | Constants should be SCREAMING_SNAKE_CASE |
| `unneeded_return` | StyleConvention | Low | Remove redundant `return` keyword |
| `doc_comment_style` | StyleConvention | Low | Use `///` for documentation comments |
| `event_suffix` | StyleConvention | Low | Event structs should have `Event` suffix |
| `empty_vector_literal` | StyleConvention | Low | Prefer `vector[]` over `vector::empty()` |
| `typed_abort_code` | StyleConvention | Low | Abort codes should be constants with descriptive names |
| `test_abort_code` | StyleConvention | Low | Test expected aborts should use named constants |
| `redundant_test_prefix` | StyleConvention | Low | Don't prefix test functions with `test_` (use `#[test]`) |
| `merge_test_attributes` | StyleConvention | Low | Merge multiple `#[test]` attributes |
| `admin_cap_position` | StyleConvention | Low | Admin cap should be first parameter |
| `equality_in_assert` | StyleConvention | Low | Use `assert!(a == b)` not `assert(a == b, ...)` |
| `manual_option_check` | StyleConvention | Low | Use `option::is_some/is_none` |
| `manual_loop_iteration` | StyleConvention | Low | Use `for` loops instead of while with index |
| `prefer_vector_methods` | StyleConvention | Low | Use vector methods over manual operations |
| `modern_method_syntax` | StyleConvention | Low | Use method call syntax |
| `explicit_self_assignments` | StyleConvention | Low | Remove redundant self assignments |
| `unnecessary_public_entry` | StyleConvention | Low | Remove `public` from `entry` functions |
| `public_mut_tx_context` | StyleConvention | Low | TxContext should be `&mut` in public functions |

### Syntactic - Security (6 lints)

| Lint | Gap | FP Risk | Description |
|------|-----|---------|-------------|
| `stale_oracle_price` | TemporalOrdering | Low | Using `get_price_unsafe` without freshness check |
| `single_step_ownership_transfer` | TemporalOrdering | Low | Admin transfer without two-step confirmation |
| `missing_witness_drop` | AbilityMismatch | Low | OTW struct missing `drop` ability |
| `public_random_access` | ApiMisuse | Low | Public function exposes Random object |
| `suspicious_overflow_check` | ArithmeticSafety | Low | Manual overflow checks are error-prone |
| `ignored_boolean_return` | ValueFlow | Low | Boolean return value ignored (e.g., `vector::contains`) |
| `divide_by_zero_literal` | ArithmeticSafety | Low | Division by literal zero |

### TypeBased - Semantic (13 lints)

| Lint | Gap | FP Risk | Description |
|------|-----|---------|-------------|
| `share_owned_authority` | OwnershipViolation | Low | Sharing `key+store` objects weakens access control |
| `droppable_hot_potato_v2` | AbilityMismatch | Low | Hot potato has `drop` ability |
| `unused_return_value` | ValueFlow | Low | Important return value ignored |
| `event_emit_type_sanity` | ApiMisuse | Low | Emitting non-event type |

### TypeBased - Sui Monorepo Pass-Through (9 lints)

These wrap official Sui compiler lints for unified output.

| Lint | Gap | FP Risk | Description |
|------|-----|---------|-------------|
| `share_owned` | OwnershipViolation | Low | Sharing non-fresh object |
| `self_transfer` | OwnershipViolation | Low | Transferring to sender (should return) |
| `custom_state_change` | ApiMisuse | Low | Custom transfer should use private variants |
| `coin_field` | ApiMisuse | Low | Use `Balance` not `Coin` in structs |
| `freeze_wrapped` | OwnershipViolation | Low | Freezing object with wrapped children |
| `collection_equality` | ApiMisuse | Low | Don't compare collections with `==` |
| `public_random` | ApiMisuse | Low | Random state should be private |
| `missing_key` | AbilityMismatch | Low | Shared object missing `key` ability |
| `freezing_capability` | OwnershipViolation | Low | Don't freeze capability objects |

---

## Tier 2: Preview Lints (3)

**FP Risk:** < 1% (Near-zero with CFG analysis)  
**Usage:** `move-clippy lint --mode full --preview`

| Lint | Analysis | Gap | Description |
|------|----------|-----|-------------|
| `unchecked_division_v2` | TypeBasedCFG | ArithmeticSafety | Division without zero-check (CFG-aware) |
| `destroy_zero_unchecked_v2` | TypeBasedCFG | ValueFlow | `destroy_zero` without verifying value is zero (CFG-aware) |
| `fresh_address_reuse_v2` | TypeBasedCFG | OwnershipViolation | `fresh_object_address` result reused (CFG-aware) |

**Why Preview?** These use precise control-flow and dataflow analysis. They track values through all execution paths, resulting in near-zero false positives. They're in Preview (not Stable) to gather community feedback on ergonomics and performance.

---

## Tier 3: Experimental Lints (12)

**FP Risk:** 5-20% depending on lint  
**Usage:** `move-clippy lint --experimental` (implies `--preview`)  
**Use Case:** Security audits, research, one-time exploration

### High FP Risk - Heuristic Detection (8 lints)

| Lint | Analysis | Gap | FP Risk | Reason |
|------|----------|-----|---------|--------|
| `destroy_zero_unchecked` | Syntactic | ValueFlow | Medium | No CFG - can't see if caller guarantees zero |
| `otw_pattern_violation` | Syntactic | ApiMisuse | Medium | Module naming edge cases (underscores, etc.) |
| `digest_as_randomness` | Syntactic | ApiMisuse | Medium | Keyword-based detection (`random`, `seed`, `winner`) |
| `fresh_address_reuse` | Syntactic | OwnershipViolation | Medium | Simple counting heuristic |
| `unchecked_coin_split` | Syntactic | ValueFlow | High | Deprecated - Sui runtime enforces this |
| `unchecked_withdrawal` | Syntactic | ValueFlow | High | Deprecated - requires formal verification |
| `pure_function_transfer` | Syntactic | - | Medium-High | Many legitimate non-entry functions need to transfer |
| `unsafe_arithmetic` | Syntactic | ArithmeticSafety | High | Variable name heuristics |

### Medium FP Risk - Complex Analysis (4 lints)

| Lint | Analysis | Gap | FP Risk | Reason |
|------|----------|-----|---------|--------|
| `phantom_capability` | TypeBasedCFG | CapabilityEscape | Medium | "Privileged sink" detection uses heuristics |
| `capability_transfer_v2` | TypeBased | OwnershipViolation | Medium | Intentional cap grants are common |
| `transitive_capability_leak` | CrossModule | CapabilityEscape | Medium | Cross-module analysis has edge cases |
| `flashloan_without_repay` | CrossModule | TemporalOrdering | Medium | Naming heuristics for flashloan patterns |

**CFG Versions Available:** For `destroy_zero_unchecked`, `fresh_address_reuse`, use the `_v2` Preview versions for near-zero FP.

---

## Tier 4: Deprecated Lints (3)

**Status:** Will be removed in next major version  
**Access:** `move-clippy lint --experimental` (for backwards compatibility)

| Lint | Reason | Superseded By |
|------|--------|---------------|
| `unchecked_coin_split` | Sui runtime already enforces balance checks | (runtime check) |
| `unchecked_withdrawal` | Business logic bugs require formal verification, not linting | (formal methods) |
| `capability_leak` | Name-based heuristics superseded by type-based detection | `capability_transfer_v2` |

---

## Lint Evolution Examples

### Syntactic → CFG Upgrade

| Syntactic Version | FP Risk | CFG Version | FP Risk |
|-------------------|---------|-------------|---------|
| `destroy_zero_unchecked` | Medium | `destroy_zero_unchecked_v2` | Near-zero |
| `fresh_address_reuse` | Medium | `fresh_address_reuse_v2` | Near-zero |
| (planned) `unchecked_division` | High | `unchecked_division_v2` | Near-zero |

### Tier Promotion Path

**Experimental → Preview → Stable**

Example: `suspicious_overflow_check`
1. Started as Experimental (pattern-based detection)
2. Validated against 13 ecosystem repos (100% TP rate)
3. Promoted to Stable

---

## TypeSystemGap Categories

For detailed explanation, see [docs/TYPE_SYSTEM_GAPS.md](./TYPE_SYSTEM_GAPS.md)

| Gap | Description | Example Lints |
|-----|-------------|---------------|
| **AbilityMismatch** | Wrong ability combinations | `missing_key`, `droppable_hot_potato_v2`, `missing_witness_drop` |
| **OwnershipViolation** | Incorrect object ownership transitions | `share_owned`, `self_transfer`, `fresh_address_reuse_v2` |
| **CapabilityEscape** | Capabilities leaking scope | `phantom_capability`, `capability_transfer_v2` |
| **ValueFlow** | Values going to wrong destinations | `unused_return_value`, `ignored_boolean_return`, `destroy_zero_unchecked_v2` |
| **ApiMisuse** | Incorrect stdlib function usage | `coin_field`, `public_random`, `digest_as_randomness` |
| **TemporalOrdering** | Operations in wrong sequence | `stale_oracle_price`, `single_step_ownership_transfer`, `flashloan_without_repay` |
| **ArithmeticSafety** | Numeric operations without validation | `unchecked_division_v2`, `suspicious_overflow_check`, `divide_by_zero_literal` |
| **StyleConvention** | Style/convention issues | All style lints |

---

## Finding Lints

**By Tier:**
```bash
# List all Preview lints
grep "| Preview |" docs/LINT_REFERENCE.md

# List all Experimental lints
grep "| Experimental |" docs/LINT_REFERENCE.md
```

**By Analysis Type:**
```bash
# Find CFG-based lints
grep "TypeBasedCFG" docs/LINT_REFERENCE.md

# Find syntactic lints
grep "| Syntactic |" docs/LINT_REFERENCE.md
```

**By Gap Category:**
```bash
# Find all arithmetic safety lints
grep "ArithmeticSafety" docs/LINT_REFERENCE.md

# Find all capability escape lints
grep "CapabilityEscape" docs/LINT_REFERENCE.md
```

**By FP Risk:**
```bash
# Find medium FP risk lints
grep "| Medium |" docs/LINT_REFERENCE.md

# Find high FP risk lints
grep "| High |" docs/LINT_REFERENCE.md
```

---

## When to Use Each Tier

| Tier | CI/CD | Daily Dev | Security Audit | Research |
|------|-------|-----------|----------------|----------|
| **Stable** | ✅ Always | ✅ Always | ✅ Yes | ✅ Yes |
| **Preview** | ⚠️ Optional | ✅ Recommended | ✅ Yes | ✅ Yes |
| **Experimental** | ❌ No | ❌ No | ✅ Yes | ✅ Yes |
| **Deprecated** | ❌ No | ❌ No | ❌ No | ⚠️ Legacy only |

---

## Contributing

**To propose a new lint:**
1. Classify by TypeSystemGap (see [TYPE_SYSTEM_GAPS.md](./TYPE_SYSTEM_GAPS.md))
2. Start in Experimental tier
3. Measure FP rate on ecosystem repos
4. If FP < 5%, promote to Preview
5. If FP < 1% and community validated, promote to Stable

**To report false positives:**
- File an issue with minimal reproduction
- Include lint name, tier, and analysis type
- Help us improve or demote the lint appropriately
