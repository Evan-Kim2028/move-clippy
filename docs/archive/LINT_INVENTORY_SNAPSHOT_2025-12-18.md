# Move-Clippy Lint Inventory

**Status:** Analysis notes (may drift)

**Authoritative current inventory:**
- `docs/LINT_REFERENCE.md` (generated; see header for regen command)
- `docs/LINT_CATALOG_SUMMARY.md` (generated counts)
- `move-clippy list-rules` (authoritative for your build)

**Purpose:** Preserve analysis of lint detection mechanisms and heuristic reliance.

## How to Get the Current Inventory

This file intentionally avoids hard-coded totals and “current lint lists” (they drift). For the current catalog, use the generated references above.

## Executive Summary

This file may include counts and lint lists as part of the snapshot. Do not treat them as current; use the generated references above for up-to-date data.

| Category | Total | Stable | Preview | Experimental | Deprecated | Heuristic-Based |
|----------|-------|--------|---------|--------------|------------|-----------------|
| Syntactic (tree-sitter) | 42 | 31 | 0 | 11 | 0 | 3 (demoted) |
| TypeBased (semantic.rs) | 23 | 17 | 3 | 3 | 0 | 0 |
| TypeBasedCFG (absint) | 4 | 0 | 3 | 1 | 0 | 0 |
| CrossModule | 2 | 0 | 0 | 2 | 0 | 0 |
| **Total** | **71** | **49** | **6** | **17** | **0** | **3** |

**Note:** 4 stable lints are Sui monorepo pass-through wrappers (included in TypeBased count).

**Deprecated Lints (3):** `unchecked_coin_split`, `unchecked_withdrawal`, `capability_leak` - marked as deprecated, will be removed in next major version.

### Recent Changes (Dec 2025)

**New Semantic Lints (2025-12-18):**

Added 4 new semantic lints using first-principles ability analysis:

**Tier 1 (Stable - Zero False Positives):**
| Lint | Description | State Space | Detection |
|------|-------------|-------------|-----------|
| `public_package_single_module` | `public(package)` in single-module package is redundant | visibility × module_count | Module count + visibility check |

**Tier 2 (Preview - Near-Zero FP, Naming Heuristics):**
| Lint | Description | Heuristic | Detection |
|------|-------------|-----------|-----------|
| `store_capability` | Capability with `store` can leak to dynamic fields | Name ends with Cap or contains Admin/Auth/Capability/Witness | Name pattern + store ability |
| `copyable_capability` | Capability with `copy` can be duplicated | Name ends with Cap or contains Admin/Auth/Capability/Witness | Name pattern + copy ability |
| `shared_with_balance` | Shared object with Balance field needs access control | Balance<T> field type | Field type + share_object call |

**Removed Lint (2025-12-18):**
- `mut_ref_not_mutated` - Removed due to high false positive rate on idiomatic Sui code (e.g., `object::new(&mut TxContext)` requires `&mut` even though only reading)

**Heuristic Improvement (2025-12-18):**
- Changed capability name detection from `contains("Cap")` to `ends_with("Cap")` to avoid false positives on words like "Capacity" and "Captain"

**Previous Type-Based Lints (2025-12-18):**
- `copyable_capability` (Stable) - Detects `key+store+copy` structs that defeat access control
- `droppable_capability` (Stable) - Detects `key+store+drop` structs that can be silently discarded
- `non_transferable_fungible_object` (Stable) - Detects `key` without `store` but with `copy` or `drop`
- `storable_hot_potato` (Experimental) - Detects `store`-only structs that break consumption guarantee

**Heuristic Lint Demotion (2025-12-18):**
Demoted 3 lints from Stable to Experimental due to name-based heuristic detection:
- `stale_oracle_price` - Function name heuristic ("get_price_unsafe")
- `single_step_ownership_transfer` - Function name heuristics ("transfer_admin", "set_owner")
- `admin_cap_position` - Name suffix heuristics ("Cap", "Capability")

**Rationale:** Heuristic-based lints cannot achieve high confidence without ecosystem validation. They belong in Experimental tier where they require explicit opt-in via `--experimental` flag.

**Previous Tier Reorganization:**
- Demoted 5 lints from Preview to Experimental due to medium-high FP risk:
  - `phantom_capability` - Privileged sink detection uses heuristics
  - `transitive_capability_leak` - Cross-module analysis edge cases
  - `flashloan_without_repay` - Naming heuristics
  - `capability_transfer` - Intentional cap grants are common
  - `pure_function_transfer` - Many legitimate patterns exist

**New Lints:**
- `divide_by_zero_literal` (Stable) - Detects literal division by zero
- `destroy_zero_unchecked` (Experimental) - Syntactic version
- `otw_pattern_violation` (Experimental) - OTW naming checks
- `digest_as_randomness` (Experimental) - Randomness source detection
- `fresh_address_reuse` (Experimental) - Address reuse detection

**CFG Upgrades:**
- `destroy_zero_unchecked_v2` (Preview) - CFG version with <1% FP
- `fresh_address_reuse_v2` (Preview) - CFG version with <1% FP
- `unchecked_division_v2` (Preview) - CFG version with <1% FP

### Lint Evolution Examples

**Syntactic → CFG Upgrade Path:**
- `destroy_zero_unchecked` (Experimental, Medium FP) → `destroy_zero_unchecked_v2` (Preview, <1% FP)
- `fresh_address_reuse` (Experimental, Medium FP) → `fresh_address_reuse_v2` (Preview, <1% FP)

**Tier Promotion Path:**
- `suspicious_overflow_check`: Started Experimental → validated on ecosystem → Promoted to Stable

**Deprecated Path:**
- `capability_leak`: Name-based heuristics → Superseded by `capability_transfer` (type-based) → Deprecated

---

## Tier 1: Production-Ready (Zero/Near-Zero False Positives)

### Semantic Lints (TypeBased) - Move Compiler Integration

These use the Move compiler's type system and have **zero heuristics**:

| Lint | Description | Detection Mechanism | Status |
|------|-------------|---------------------|--------|
| `share_owned_authority` | Don't share key+store objects | Type abilities: `key + store` + share_object call | **Stable** |
| `droppable_hot_potato` | Detect broken hot potato structs | Type abilities: ONLY `drop` | **Stable** |
| `copyable_capability` | Capability with `copy` defeats access control | Type abilities: `key + store + copy` | **Stable** |
| `droppable_capability` | Capability with `drop` can be silently discarded | Type abilities: `key + store + drop` (no copy) | **Stable** |
| `non_transferable_fungible_object` | Non-transferable object with fungible abilities | Type abilities: `key` without `store`, with `copy` or `drop` | **Stable** |
| `unused_return_value` | Important return value ignored | Return type analysis | **Stable** |
| `event_emit_type_sanity` | Emitting non-event types | Type abilities check | **Stable** |
| `storable_hot_potato` | Hot potato with `store` can be embedded | Type abilities: ONLY `store` | **Experimental** |
| `public_package_single_module` | `public(package)` in single-module package | Module count + visibility | **Stable** |

### Sui Monorepo Lints (pass-through from sui_mode::linters)

These lints are **pass-through wrappers** for the official Sui Move compiler lints from the Sui monorepo.
They provide unified output formatting through move-clippy when running in `--mode full`.

**Source:** [sui_mode::linters](https://github.com/MystenLabs/sui/tree/main/external-crates/move/crates/move-compiler/src/sui_mode/linters)

| Lint | Description | Status |
|------|-------------|--------|
| `share_owned` | Possible owned object share | **Stable** |
| `self_transfer` | Transferring object to self - consider returning instead | **Stable** |
| `custom_state_change` | Custom transfer/share/freeze should call private variants | **Stable** |
| `coin_field` | Use Balance instead of Coin in struct fields | **Stable** |
| `freeze_wrapped` | Do not freeze objects containing wrapped objects | **Stable** |
| `collection_equality` | Avoid equality checks on collections | **Stable** |
| `public_random` | Random state should remain private | **Stable** |
| `missing_key` | Shared/transferred object missing key ability | **Stable** |
| `freezing_capability` | Avoid freezing capability objects | **Stable** |

> **Note:** These checks also run when you use `sui move build` directly.

### Syntactic Lints - Zero FP (Pattern-Based, No Heuristics)

| Lint | Description | Detection | Why Zero FP |
|------|-------------|-----------|-------------|
| `abilities_order` | Abilities in canonical order | AST pattern | Exact pattern match |
| `doc_comment_style` | Use `///` not `/* */` | AST pattern | Exact syntax match |
| `modern_module_syntax` | Use `module x;` not `module x {}` | AST pattern | Exact syntax match |
| `while_true_to_loop` | Use `loop` not `while(true)` | AST pattern | Exact syntax match |
| `empty_vector_literal` | Use `vector[]` not `vector::empty()` | Call pattern | Exact call match |
| `redundant_self_import` | `use x::{Self}` is redundant | Import pattern | Exact pattern match |
| `prefer_vector_methods` | Use `v.push_back()` not `vector::push_back(&mut v)` | Call pattern | Exact call match |
| `modern_method_syntax` | Use method calls for allowlisted functions | Call pattern | Exact call match (allowlist) |
| `unnecessary_public_entry` | Don't use both `public` and `entry` | Modifier check | Exact modifier match |
| `public_mut_tx_context` | TxContext should be `&mut` | Type check | Exact type pattern |
| `unneeded_return` | Don't use trailing `return` | AST pattern | Exact syntax match |
| `constant_naming` | Constants: `E_*` or `SCREAMING_SNAKE` | Name pattern | Regex validation |
| `equality_in_assert` | Use `assert_eq!` not `assert!(a == b)` | Macro pattern | Exact pattern match |

---

## Tier 2: Low FP Risk (Structure + Type Heuristics)

These use **structural validation** with minimal name heuristics to reduce FPs:

### Security Lints - Structural Validation

| Lint | Status | Detection | Validation | FP Risk |
|------|--------|-----------|------------|---------|
| `missing_witness_drop` | Stable | OTW pattern: uppercase struct name | Missing `drop` ability | Low |
| `public_random_access` | Stable | Param type: `Random`, `&Random` | Function visibility: `public` | Low |
| `suspicious_overflow_check` | Stable | Function name pattern | Body: bit shifts + comparisons | Very Low |
| `ignored_boolean_return` | Stable | Return type: `bool` | Result discarded | Low |

### Style/Convention Lints - Type-Based

| Lint | Status | Detection | FP Risk |
|------|--------|-----------|---------|
| `event_suffix` | Stable | Abilities `copy+drop` without `key` | Low |
| `typed_abort_code` | Stable | Numeric literals in `abort`/`assert!` | Low |

---

## Tier 3: Preview (Low FP Risk, Requires --preview)

These are CFG-aware lints with near-zero FP but require `--preview` flag:

### CFG-Aware Lints (absint_lints.rs)

| Lint | Description | Detection | FP Risk |
|------|-------------|-----------|---------|
| `unchecked_division_v2` | Division without zero check | Type analysis + CFG | <1% |
| `destroy_zero_unchecked_v2` | destroy_zero without zero-value check | Type analysis + CFG | <1% |
| `fresh_address_reuse_v2` | fresh_object_address result reused | Type analysis + CFG | <1% |

### Naming Heuristic Lints (semantic.rs) - NEW

These use naming conventions to identify security-sensitive patterns:

| Lint | Description | Detection | FP Risk |
|------|-------------|-----------|---------|
| `store_capability` | Capability with `store` can leak via dynamic fields | Name (Cap/Admin/Auth/Witness) + `store` ability | Low |
| `copyable_capability` | Capability with `copy` can be duplicated | Name (Cap/Admin/Auth/Witness) + `copy` ability | Low |
| `shared_with_balance` | Shared object with Balance needs access control | `Balance<T>` field + `share_object` call | Very Low |

**Detection Heuristic for Capability Names:**
```
is_capability_name(name) = name.contains("Cap") 
                        || name.contains("Capability")
                        || name.contains("Admin")
                        || name.contains("Auth")
                        || name.ends_with("Witness")
```

> **Note:** These lints may fire on false positives like "Capacity" or "Captain" due to the substring matching. Use `#[ext(move_clippy(allow(store_capability)))]` to suppress.

---

## Tier 4: Experimental (High FP Risk, Requires --experimental)

These use **name-based heuristics** and have medium-high FP risk. They require `--experimental` flag.

### Name-Based Heuristic Lints (Demoted from Stable)

| Lint | Heuristic | FP Risk | Issue |
|------|-----------|---------|-------|
| `stale_oracle_price` | Function name: `get_price_unsafe` | Medium | Name-based detection |
| `single_step_ownership_transfer` | Function names: `transfer_admin`, `set_owner` | Medium | Name patterns may not indicate bugs |
| `admin_cap_position` | Name suffixes: `Cap`, `Capability` | Medium | Many legitimate uses of these suffixes |

### Other Experimental Lints

| Lint | Heuristic | FP Risk | Issue |
|------|-----------|---------|-------|
| `pure_function_transfer` | `transfer::*` in non-entry | Medium | May flag legitimate patterns |
| `unsafe_arithmetic` | `+`, `-`, `*` operators | High | No overflow analysis |
| `destroy_zero_unchecked` | Syntactic pattern | Medium | Needs CFG (use _v2) |
| `otw_pattern_violation` | Module name matching | Medium | Needs better module name handling |
| `digest_as_randomness` | Keyword-based | Medium | Needs taint analysis |
| `fresh_address_reuse` | Usage count heuristic | Medium | Needs CFG (use _v2) |
| `phantom_capability` | Privileged sink heuristics | Medium | Complex detection logic |

### Cross-Module Experimental Lints

| Lint | Description | Detection | FP Risk | Issue |
|------|-------------|-----------|---------|-------|
| `transitive_capability_leak` | Cap leaks across modules | Call graph + type abilities | Medium | Conservative approximation |
| `flashloan_without_repay` | Flashloan not repaid | Call graph analysis | Medium | Naming heuristics for loan detection |

---

## Tier 5: Deprecated

These lints are disabled by default and will be removed in a future version.

| Lint | Reason for Deprecation |
|------|------------------------|
| `unchecked_coin_split` | Sui runtime already enforces balance checks |
| `unchecked_withdrawal` | Business logic bugs require formal verification, not linting |
| `capability_leak` | Superseded by `capability_transfer` (type-based) |

---

## Detection Method Classification

### Pure Structural (No Heuristics) ✓
- All Sui-delegated lints
- Type/ability-based naming lints
- Exact syntax pattern lints

### Name + Structure (Low Risk)
- Hot potato detection: name keywords + ability pattern + field count
- Capability sharing: name pattern + `share_object` call
- OTW detection: module name pattern + struct pattern

### Name-Only (Higher Risk)
- Withdrawal detection: function name contains "withdraw"
- Capability leak: argument name contains "Cap"
- Admin transfer: function name contains "admin"/"owner"

### Call Pattern (API Knowledge)
- `get_price_unsafe` - exact function name
- `share_object` - exact function family
- `coin::split` - exact module::function

---

## Recommendations

### Immediate Actions
1. **Promote to Stable** (after validation):
   - `pure_function_transfer` - useful pattern, low FP after refinement
   
2. **Keep in Preview** (need work):
   - CFG lints need guard pattern detection
   - `unchecked_withdrawal` needs balance analysis
   - `capability_leak` needs recipient validation

3. **Consider Removing**:
   - `unsafe_arithmetic` - too noisy without proper dataflow
   - Deprecated lints should be removed entirely

### Long-Term Improvements
1. **Type-based detection** for security lints where possible
2. **Dataflow analysis** for unchecked operations
3. **Inter-procedural analysis** for leak detection
4. **Allowlist/denylist** configuration for name heuristics

---

## Test Coverage Summary

- 78 unit tests passing
- Ecosystem validation: 4 major repos (requires local clones)
- Snapshot tests: 22 fixture tests
- Fix tests: 16 auto-fix tests
