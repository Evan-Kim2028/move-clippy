# Move-Clippy Lint Catalog Summary

**Status:** Generated (do not edit by hand)

This file is generated from the unified lint registry.

Regenerate with:

```bash
cargo run --features full --bin gen_lint_catalog_summary > docs/LINT_CATALOG_SUMMARY.md
```

## Totals

- Total lints: 82

## By Tier

| Tier | Count |
|------|-------|
| stable | 51 |
| preview | 8 |
| experimental | 20 |
| deprecated | 3 |

## By Phase

| Phase | Count |
|-------|-------|
| syntactic | 40 |
| semantic | 35 |
| absint | 5 |
| cross-module | 2 |

## By Category

| Category | Count |
|----------|-------|
| style | 11 |
| modernization | 9 |
| naming | 2 |
| security | 44 |
| suspicious | 13 |
| test_quality | 3 |

## By Analysis Kind

| Analysis | Count | Requires |
|----------|-------|----------|
| syntactic | 40 | `fast` |
| type-based | 35 | `--mode full` |
| type-based-cfg | 5 | `--mode full` |
| cross-module | 2 | `--mode full` |
