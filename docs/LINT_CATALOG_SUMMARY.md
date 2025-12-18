# Move-Clippy Lint Catalog Summary

**Status:** Generated (do not edit by hand)

This file is generated from the unified lint registry.

Regenerate with:

```bash
cargo run --features full --bin gen_lint_catalog_summary > docs/LINT_CATALOG_SUMMARY.md
```

## Totals

- Total lints: 71

## By Tier

| Tier | Count |
|------|-------|
| stable | 48 |
| preview | 8 |
| experimental | 12 |
| deprecated | 3 |

## By Phase

| Phase | Count |
|-------|-------|
| syntactic | 39 |
| semantic | 24 |
| absint | 6 |
| cross-module | 2 |

## By Category

| Category | Count |
|----------|-------|
| style | 9 |
| modernization | 9 |
| naming | 2 |
| security | 35 |
| suspicious | 13 |
| test_quality | 3 |

## By Analysis Kind

| Analysis | Count | Requires |
|----------|-------|----------|
| syntactic | 39 | `fast` |
| type-based | 24 | `--mode full` |
| type-based-cfg | 6 | `--mode full` |
| cross-module | 2 | `--mode full` |
