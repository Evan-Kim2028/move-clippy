# Ecosystem Validation

This directory contains tools and results for validating move-clippy against real-world Move codebases.

## Overview

Ecosystem validation helps ensure move-clippy produces high-quality results on production Move code by:
- Running lints against major Sui protocols (Cetus, DeepBook, Scallop, etc.)
- Tracking false positive rates
- Validating lint accuracy and usefulness

## Usage

```bash
# Run validation on ecosystem repos (requires cloned repos in packages/ecosystem-test-repos)
./run_all.sh

# Analyze results
python3 calculate_metrics.py

# Import findings to triage database
python3 scripts/import_results_to_triage.py
```

## Directory Structure

- `results/` - Lint output (JSON + logs) from ecosystem repos
- `scripts/` - Automation scripts for validation workflow
- `analysis/` - Manual analysis and patterns documentation
- `triage*.json` - Triage databases (gitignored, regeneratable)

## Internal vs Public

**Note:** Most files in this directory are gitignored to keep the repository clean:
- Validation results (`.json`, `.log` files)
- Triage databases
- Analysis reports
- Scripts

Only this README is version controlled. The validation artifacts remain local for your use but aren't committed to the repository.

## Ecosystem Repos Validated

Currently validated against:
- **Cetus** - CLMM DEX with concentrated liquidity
- **DeepBook** - Central limit order book
- **Scallop** - Lending protocol
- **Suilend** - Lending and liquid staking
- **OpenZeppelin Sui** - Security libraries
- **Alphalend** - Lending protocol
- **Bluefin** - Perpetuals and spot DEX
- **Steamm** - AMM protocol

See `packages/ecosystem-test-repos/` for the actual repositories.
