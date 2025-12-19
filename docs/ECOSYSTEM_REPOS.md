# Ecosystem Test Repositories

**Purpose:** Track all ecosystem repositories used for move-clippy validation.

## Current Ecosystem Repos

### Core Repos (Original)

| Repository | GitHub URL | Category | Baseline | Status |
|------------|-----------|----------|----------|--------|
| DeepBook V3 | https://github.com/MystenLabs/deepbookv3 | DEX/AMM | ✅ `deepbookv3.json` | Active |
| OpenZeppelin Sui | https://github.com/OpenZeppelin/openzeppelin-contracts-sui | Utilities | ✅ `openzeppelin-sui.json` | Active |
| Cetus CLMM | https://github.com/CetusProtocol/cetus-clmm-sui | DEX/AMM | ✅ `cetus-clmm.json` | Active |
| Scallop Lend | https://github.com/scallop-io/sui-lending-protocol | Lending | ✅ `scallop-lend.json` | Active |

### Interest Protocol Repos (Added 2025-12-18)

| Repository | GitHub URL | Category | Stars | Baseline | Status |
|------------|-----------|----------|-------|----------|--------|
| Suitears | https://github.com/interest-protocol/suitears | Utilities/DeFi | 82 ⭐ | ⏳ Pending | Added |
| Memez.gg | https://github.com/interest-protocol/memez-gg | Meme Tokens | 11 ⭐ | ⏳ Pending | Added |
| Interest MVR | https://github.com/interest-protocol/interest-mvr | Sui Libs | 8 ⭐ | ⏳ Pending | Added |
| Move Interfaces | https://github.com/interest-protocol/move-interfaces | Patterns | 0 ⭐ | ⏳ Pending | Added |
| Some | https://github.com/interest-protocol/some | Social | 0 ⭐ | ⏳ Pending | Added |

## Cloning Interest Protocol Repos

```bash
cd ../ecosystem-test-repos

# Clone all Interest Protocol Move repos
git clone https://github.com/interest-protocol/suitears
git clone https://github.com/interest-protocol/memez-gg
git clone https://github.com/interest-protocol/interest-mvr
git clone https://github.com/interest-protocol/move-interfaces
git clone https://github.com/interest-protocol/some
```

## Generating Baselines

After cloning, generate baselines:

```bash
cd /path/to/move-clippy

# Generate all Interest Protocol baselines
UPDATE_BASELINES=1 cargo test --features full ecosystem_suitears -- --ignored
UPDATE_BASELINES=1 cargo test --features full ecosystem_memez_gg -- --ignored
UPDATE_BASELINES=1 cargo test --features full ecosystem_interest_mvr -- --ignored
UPDATE_BASELINES=1 cargo test --features full ecosystem_move_interfaces -- --ignored
UPDATE_BASELINES=1 cargo test --features full ecosystem_some -- --ignored

# Or generate all at once
UPDATE_BASELINES=1 cargo test --features full -- --ignored ecosystem
```

## Candidate Repos (To Add)

### Other Potential Repos

| Repository | GitHub URL | Category | Notes |
|------------|-----------|----------|-------|
| Sui Framework | https://github.com/MystenLabs/sui (crates/sui-framework) | Framework | Reference implementation |
| NAVI Protocol | https://github.com/naviprotocol | Lending | Active Sui lending protocol |
| Bucket Protocol | https://github.com/Bucket-Protocol | Stablecoin | CDP-based stablecoin |
| Aftermath Finance | https://github.com/AftermathFinance | DEX/Derivatives | Multi-product DeFi |
| Turbos Finance | https://github.com/turbos-finance | DEX | Concentrated liquidity |
| FlowX Finance | https://github.com/FlowX-Finance | DEX | AMM DEX |
| Bluefin | https://github.com/bluefin-exchange | Derivatives | Perps exchange |
| Haedal Protocol | https://github.com/haedal-xyz | LST | Liquid staking |
| Kriya DEX | https://github.com/KriyaDex | DEX | Order book DEX |
| Vortex | https://github.com/interest-protocol/vortex | Privacy | Rust-based (not Move) |

## Adding a New Ecosystem Repo

### Step 1: Clone the Repository

```bash
cd ../ecosystem-test-repos
git clone https://github.com/interest-protocol/suitears
```

### Step 2: Identify the Move Package Path

```bash
# Find Move.toml files
find suitears -name "Move.toml" -type f

# Common patterns:
# - Root level: suitears/Move.toml
# - Packages dir: suitears/packages/*/Move.toml
# - Sources dir: suitears/sources/
```

### Step 3: Add Test Definition

Edit `tests/ecosystem.rs`:

```rust
/// Suitears - Interest Protocol's production-ready Move modules
#[test]
#[ignore = "requires ecosystem-test-repos clone"]
fn ecosystem_suitears() {
    let test = EcosystemTest {
        name: "suitears",
        repo_path: "../ecosystem-test-repos/suitears",
        lint_path: None,  // or Some("packages") if needed
    };
    test.run().expect("ecosystem test failed");
}
```

### Step 4: Generate Baseline

```bash
# Run with UPDATE_BASELINES to create initial baseline
UPDATE_BASELINES=1 cargo test --features full ecosystem_suitears -- --ignored

# This creates tests/baselines/suitears.json
```

### Step 5: Review and Commit

1. Review the baseline JSON for sanity
2. Ensure no duplicate repos (check this file)
3. Commit both the test and baseline

## Preventing Duplicates

Before adding a new repo, check:

1. **This document** - Is it already listed above?
2. **Baseline files** - `ls tests/baselines/`
3. **Test file** - `grep "ecosystem_" tests/ecosystem.rs`

## Criteria for Ecosystem Repos

### Must Have
- [ ] Public GitHub repository
- [ ] Contains `.move` files
- [ ] Has a valid `Move.toml`
- [ ] Active development (commits within last year)

### Nice to Have
- [ ] Stars > 10 (indicates community validation)
- [ ] Used in production (mainnet deployment)
- [ ] Diverse patterns (different from existing repos)
- [ ] Good test coverage (indicates code quality)

### Categories to Cover
- [x] DEX/AMM (Cetus, DeepBook)
- [x] Lending (Scallop)
- [x] Utilities (OpenZeppelin)
- [ ] Stablecoins
- [ ] NFT/Gaming
- [ ] Oracles
- [ ] Bridges
- [ ] Liquid Staking
- [ ] Derivatives/Perps

## Maintenance

### Updating Baselines

When lint behavior legitimately changes:

```bash
# Update all baselines
UPDATE_BASELINES=1 cargo test --features full -- --ignored ecosystem

# Or update specific baseline
UPDATE_BASELINES=1 cargo test --features full ecosystem_suitears -- --ignored
```

### Checking for Repo Updates

Periodically check if ecosystem repos have significant updates:

```bash
cd ../ecosystem-test-repos
for repo in */; do
    echo "=== $repo ==="
    cd "$repo" && git log --oneline -3 && cd ..
done
```

### Version Alignment

All repos should be tested against the same Sui framework version. See `LOCAL_ECOSYSTEM_VALIDATION.md` for version sync instructions.

## Statistics

| Metric | Count |
|--------|-------|
| Core repos (with baselines) | 4 |
| Interest Protocol repos (pending baselines) | 5 |
| Total repos | 9 |
| Total baseline diagnostics | ~1,200+ |
| Categories covered | 6/9 |
| Last updated | 2025-12-18 |
