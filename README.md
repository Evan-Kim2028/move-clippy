# Move Clippy

A Move linter for Sui, inspired by Rust Clippy.

> **Research Project** — Built with AI assistance (Claude). Not a substitute for security audits.

## Installation

```bash
git clone https://github.com/Evan-Kim2028/move-clippy.git
cd move-clippy
cargo build --release

# Optional: add to PATH
export PATH="$PATH:$(pwd)/target/release"
```

For `--mode full` (semantic analysis), clone Sui monorepo as sibling:
```bash
git clone https://github.com/MystenLabs/sui.git ../sui
cargo build --release --features full
```

## Quick Start

```bash
# Basic linting
move-clippy path/to/sources

# With compiler type info (requires --features full build)
move-clippy --mode full path/to/package

# Enable more lints (higher FP risk)
move-clippy --preview path/to/sources
move-clippy --experimental path/to/sources

# List available lints
move-clippy list-rules
```

## Lint Tiers

| Tier | Flag | Use Case |
|------|------|----------|
| **Stable** | default | CI, daily dev — low false positives |
| **Preview** | `--preview` | Exploration — still validating |
| **Experimental** | `--experimental` | Audits/research — expect noise |

## What's Included

45 stable lints enabled by default:

| Category | Count |
|----------|-------|
| Security | 11 |
| Suspicious | 9 |
| Style | 11 |
| Modernization | 9 |
| Test Quality | 3 |
| Naming | 2 |

Additional lints available with `--preview` (3) and `--experimental` (16).

### Security & Suspicious (Stable)

| Lint | Description |
|------|-------------|
| `copyable_capability` | Detects `key+store+copy` structs — transferable authority can be duplicated |
| `droppable_capability` | Detects `key+store+drop` structs — authority can be silently discarded |
| `capability_transfer_literal_address` | Capability transferred to literal address like `@0x1` |
| `divide_by_zero_literal` | Division by literal zero — will always abort |
| `suspicious_overflow_check` | Manual bit-shift overflow patterns (Cetus $223M hack pattern) |
| `invalid_otw` | One-time witness violates Sui Adapter rules |
| `public_random_access_v2` | Public function exposes `sui::random::Random` — enables front-running |
| `witness_antipatterns` | Witness struct has `copy/store/key` or public constructor |
| `coin_field` | Use `Balance` instead of `Coin` in struct fields |
| `freeze_wrapped` | Don't freeze objects containing wrapped objects |
| `entry_function_returns_value` | Entry function return value is discarded by runtime |
| `private_entry_function` | Private entry function is unreachable |

### Style & Modernization (Stable)

| Lint | Description |
|------|-------------|
| `modern_method_syntax` | Prefer `v.push_back(x)` over `vector::push_back(&mut v, x)` |
| `modern_module_syntax` | Prefer `module pkg::mod;` over block form |
| `prefer_vector_methods` | Prefer `v.length()` over `vector::length(&v)` |
| `empty_vector_literal` | Prefer `vector[]` over `vector::empty()` |
| `abilities_order` | Struct abilities should be ordered: `key, copy, drop, store` |
| `equality_in_assert` | Prefer `assert_eq!(a, b)` for clearer failure messages |
| `typed_abort_code` | Prefer named error constants over numeric abort codes |

### Preview (--preview)

| Lint | Description |
|------|-------------|
| `stale_oracle_price_v3` | CFG-aware: oracle price used without freshness validation |
| `droppable_flash_loan_receipt` | Function returns Coin/Balance with droppable receipt |
| `mut_key_param_missing_authority` | Public entry takes `&mut` key object without authority param |

Run `move-clippy list-rules` for the complete list with descriptions.

Stable security lints validated against 518 Sui Move repositories with zero false positives.

## Background

Built as a learning project to explore Move static analysis. Most code was AI-generated with manual review and iteration. Started with syntactic pattern matching, evolved toward type-based detection to reduce false positives.

See [docs/](docs/) for technical details.

## Contributing

Issues and PRs welcome. This is a side project — no guarantees on response time.

---

*An experiment in AI-assisted tooling for the Move ecosystem.*
