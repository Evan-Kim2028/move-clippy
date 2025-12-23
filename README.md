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

39 stable lints enabled by default:

| Category | Count |
|----------|-------|
| Security | 8 |
| Suspicious | 9 |
| Style | 11 |
| Modernization | 6 |
| Test Quality | 3 |
| Naming | 2 |

Additional lints available with `--preview` (3) and `--experimental` (12).

Many lints are backed by the official [Move Book Code Quality Checklist](https://move-book.com/guides/code-quality-checklist/).

### Analysis Types

| Type | Description | Mode |
|------|-------------|------|
| **syntactic** | Pattern matching on AST — fast, no type info needed | `fast` (default) |
| **type-based** | Uses Move compiler type information | `--mode full` |
| **CFG** | Control-flow graph analysis for data flow tracking | `--mode full` |

### Security & Suspicious (Stable)

| Lint | Analysis | Description |
|------|----------|-------------|
| `copyable_capability` | type-based | Detects `key+store+copy` structs — transferable authority can be duplicated |
| `droppable_capability` | type-based | Detects `key+store+drop` structs — authority can be silently discarded |
| `capability_transfer_literal_address` | type-based | Capability transferred to literal address like `@0x1` |
| `suspicious_overflow_check` | syntactic | Manual bit-shift overflow patterns (Cetus $223M hack pattern) |
| `public_random_access_v2` | type-based | Public function exposes `sui::random::Random` — enables front-running |
| `witness_antipatterns` | type-based | Witness struct has `copy/store/key` or public constructor |
| `coin_field` | type-based | Use `Balance` instead of `Coin` in struct fields |
| `freeze_wrapped` | type-based | Don't freeze objects containing wrapped objects |
| `entry_function_returns_value` | type-based | Entry function return value is discarded by runtime |
| `private_entry_function` | type-based | Private entry function is unreachable |

### Style & Modernization (Stable)

| Lint | Analysis | Description |
|------|----------|-------------|
| `modern_method_syntax` | syntactic | Prefer `v.push_back(x)` over `vector::push_back(&mut v, x)` |
| `modern_module_syntax` | syntactic | Prefer `module pkg::mod;` over block form |
| `prefer_vector_methods` | syntactic | Prefer `v.length()` over `vector::length(&v)` |
| `empty_vector_literal` | syntactic | Prefer `vector[]` over `vector::empty()` |
| `abilities_order` | syntactic | Struct abilities should be ordered: `key, copy, drop, store` |
| `equality_in_assert` | syntactic | Prefer `assert_eq!(a, b)` for clearer failure messages |
| `typed_abort_code` | syntactic | Prefer named error constants over numeric abort codes |

### Preview (--preview)

| Lint | Analysis | Description |
|------|----------|-------------|
| `stale_oracle_price_v3` | CFG | Oracle price used without freshness validation |
| `droppable_flash_loan_receipt` | type-based | Function returns Coin/Balance with droppable receipt |
| `mut_key_param_missing_authority` | type-based | Public entry takes `&mut` key object without authority param |

Run `move-clippy list-rules` for the complete list with descriptions.

### Sui Linter Pass-throughs

Several lints are pass-throughs from the Sui compiler's built-in linter (`sui_mode::linters`), exposed through Move Clippy for unified output formatting:
- `coin_field`, `collection_equality`, `custom_state_change`, `freeze_wrapped`
- `freezing_capability`, `missing_key`, `public_random`, `self_transfer`, `share_owned`

### Validation

Stable security lints have been validated against ecosystem repositories. Note: Lint counts and validation statistics may change between versions as lints are added, refined, or deprecated.

## Background

Built as a learning project to explore Move static analysis. Most code was AI-generated with manual review and iteration. Started with syntactic pattern matching, evolved toward type-based detection to reduce false positives.

See [docs/](docs/) for technical details.

## Contributing

Issues and PRs welcome. This is a side project — no guarantees on response time.

---

*An experiment in AI-assisted tooling for the Move ecosystem.*
