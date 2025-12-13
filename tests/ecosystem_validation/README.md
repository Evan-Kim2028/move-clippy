# Ecosystem Validation

This directory contains tools and data for validating Move Clippy against real-world production codebases.

## Purpose

Measure actual false positive (FP) rates for lints by running them against 14 production Move repositories (~40K lines of code) and manually triaging the results.

## Workflow

### 1. Run Validation

```bash
./run_all.sh
```

This will:
- Build move-clippy in release mode
- Run linter on all ecosystem repos with `--preview` flag
- Save results to `results/` directory
- Generate a summary

### 2. Manual Triage

Review the findings in `results/*.json` and classify each as:
- **TP** (True Positive): Real bug or code smell
- **FP** (False Positive): Incorrect warning
- **INFO**: Informational (useful but not a bug)
- **SKIP**: Unable to determine

Copy `triage_template.json` to `triage.json` and fill in classifications:

```bash
cp triage_template.json triage.json
# Edit triage.json manually
```

### 3. Calculate Metrics

```bash
python calculate_metrics.py
```

This generates:
- Console summary with FP rates per lint
- Detailed Markdown report: `VALIDATION_REPORT.md`
- Recommendations for lint promotion/demotion

## Triage Schema

See `triage_schema.json` for the JSON schema.

Each finding requires:
- `id`: Unique identifier (repo_file_line_lint)
- `classification`: TP, FP, INFO, or SKIP
- `rationale`: Explanation for the classification

Optional fields:
- `severity`: critical, high, medium, low (for TPs)
- `fixed_in_commit`: Git hash where bug was fixed
- `audit_reference`: Link to audit report

## Ecosystem Repositories

| Repository | Type | LOC | Audit Status |
|------------|------|-----|--------------|
| alphalend | Lending | ~3K | Pre/post audit commits available |
| scallop-lend | Lending | ~5K | OtterSec + MoveBit audits |
| suilend | Lending | ~4K | OtterSec audit |
| deepbookv3 | DEX | ~8K | Trail of Bits audit |
| cetus-clmm | DEX | ~6K | Post-hack code |
| bluefin-* (3 repos) | Perps | ~10K | MoveBit Contest 2024 |
| openzeppelin-sui | Library | ~2K | Production-grade reference |
| steamm | AMM | ~2K | Unknown |
| suilend-liquid-staking | Staking | ~2K | Unknown |

**Total:** 14 repos, ~40K lines of production Move code

## Success Criteria

**Minimum Viable (v0.4.0):**
- ✅ At least 5 repos validated
- ✅ FP rates measured for all Preview lints
- ✅ Report published

**Target (v0.4.0):**
- ✅ All 14 repos validated
- ✅ 1-2 lints promoted to Stable (FP < 10%)
- ✅ FP rates < 15% for Preview lints

## Lint Promotion Criteria

**Stable (default enabled):**
- FP rate < 10%
- ≥ 5 findings across repos
- ≥ 2 weeks in Preview
- No blocking community concerns

**Preview (opt-in):**
- FP rate 10-25%
- Useful but needs refinement

**Research (experimental):**
- FP rate > 25%
- Needs major redesign or more evidence

## Files

- `run_all.sh` - Main validation runner
- `calculate_metrics.py` - Metrics calculator
- `triage_schema.json` - JSON schema for triage data
- `triage_template.json` - Template for manual triage
- `triage.json` - Actual triage data (gitignored)
- `results/` - Raw lint findings (gitignored)
- `baselines/` - Known-good violation baselines
- `VALIDATION_REPORT.md` - Generated report (gitignored)

## Tips

1. **Focus on security lints first** - highest impact
2. **Look for patterns in FPs** - helps with refinement
3. **Document FP rationales clearly** - enables systematic fixes
4. **Check audit reports** - confirm TPs were real bugs
5. **Use git blame** - see if findings were fixed in later commits
