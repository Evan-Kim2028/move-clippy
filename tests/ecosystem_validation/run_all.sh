#!/bin/bash
# Ecosystem Validation Runner
# Executes move-clippy on all ecosystem repos and captures results

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RESULTS_DIR="$SCRIPT_DIR/results"
CLIPPY_BIN="$SCRIPT_DIR/../../target/release/move-clippy"
REPOS_DIR="$SCRIPT_DIR/../../ecosystem-test-repos"

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Build move-clippy in release mode for speed
echo -e "${YELLOW}Building move-clippy in release mode...${NC}"
cd "$SCRIPT_DIR/../.."
cargo build --release

# Repository list with metadata
declare -A REPOS=(
    ["alphalend"]="AlphaLend lending protocol"
    ["scallop-lend"]="Scallop lending protocol"
    ["suilend"]="Suilend lending protocol"
    ["deepbookv3"]="DeepBook V3 DEX"
    ["cetus-clmm"]="Cetus CLMM DEX"
    ["bluefin-spot"]="Bluefin spot trading"
    ["bluefin-pro"]="Bluefin perpetuals"
    ["bluefin-integer"]="Bluefin integer math"
    ["openzeppelin-sui"]="OpenZeppelin Sui library"
    ["steamm"]="Steamm AMM"
    ["suilend-liquid-staking"]="Suilend liquid staking"
)

# Statistics tracking
TOTAL_REPOS=0
SUCCESSFUL_RUNS=0
FAILED_RUNS=0

echo -e "${GREEN}======================================${NC}"
echo -e "${GREEN}Move Clippy Ecosystem Validation${NC}"
echo -e "${GREEN}======================================${NC}"
echo ""

# Clean previous results
rm -f "$RESULTS_DIR"/*.json
rm -f "$RESULTS_DIR"/*.log

# Run linter on each repo
for repo_name in "${!REPOS[@]}"; do
    TOTAL_REPOS=$((TOTAL_REPOS + 1))
    repo_path="$REPOS_DIR/$repo_name"
    description="${REPOS[$repo_name]}"
    
    echo -e "${YELLOW}Processing: $repo_name${NC} - $description"
    
    if [ ! -d "$repo_path" ]; then
        echo -e "${RED}  ✗ Repo not found: $repo_path${NC}"
        FAILED_RUNS=$((FAILED_RUNS + 1))
        continue
    fi
    
    # Find Move.toml files (may have multiple packages)
    move_tomls=$(find "$repo_path" -name "Move.toml" -type f)
    
    if [ -z "$move_tomls" ]; then
        echo -e "${RED}  ✗ No Move.toml found${NC}"
        FAILED_RUNS=$((FAILED_RUNS + 1))
        continue
    fi
    
    # Run clippy on each package
    package_count=0
    findings_count=0
    
    while IFS= read -r move_toml; do
        package_dir=$(dirname "$move_toml")
        package_name=$(basename "$package_dir")
        
        echo -e "  Checking package: $package_name"
        
        # Run move-clippy with preview lints enabled
        # Capture both stdout and stderr
        output_file="$RESULTS_DIR/${repo_name}_${package_name}.json"
        log_file="$RESULTS_DIR/${repo_name}_${package_name}.log"
        
        if "$CLIPPY_BIN" check "$package_dir" --preview --json > "$output_file" 2> "$log_file"; then
            # Count findings
            count=$(jq '. | length' "$output_file" 2>/dev/null || echo "0")
            findings_count=$((findings_count + count))
            echo -e "    ${GREEN}✓${NC} Found $count lint findings"
        else
            # Linter failed (compilation error, etc.)
            echo -e "    ${RED}✗${NC} Linting failed - see $log_file"
            echo "ERROR" > "$output_file"
        fi
        
        package_count=$((package_count + 1))
    done <<< "$move_tomls"
    
    if [ $package_count -gt 0 ]; then
        echo -e "  ${GREEN}✓${NC} Processed $package_count package(s), found $findings_count total findings"
        SUCCESSFUL_RUNS=$((SUCCESSFUL_RUNS + 1))
    else
        FAILED_RUNS=$((FAILED_RUNS + 1))
    fi
    
    echo ""
done

# Summary
echo -e "${GREEN}======================================${NC}"
echo -e "${GREEN}Summary${NC}"
echo -e "${GREEN}======================================${NC}"
echo -e "Total repos: $TOTAL_REPOS"
echo -e "${GREEN}Successful: $SUCCESSFUL_RUNS${NC}"
echo -e "${RED}Failed: $FAILED_RUNS${NC}"
echo ""
echo -e "Results saved to: $RESULTS_DIR"
echo ""

# Generate combined summary
echo -e "${YELLOW}Generating combined summary...${NC}"
summary_file="$RESULTS_DIR/_SUMMARY.txt"
{
    echo "Move Clippy Ecosystem Validation Summary"
    echo "========================================="
    echo "Generated: $(date)"
    echo ""
    echo "Statistics:"
    echo "  Total repos: $TOTAL_REPOS"
    echo "  Successful: $SUCCESSFUL_RUNS"
    echo "  Failed: $FAILED_RUNS"
    echo ""
    echo "Per-repo findings:"
    echo ""
    
    for repo_name in "${!REPOS[@]}"; do
        total_findings=0
        for result_file in "$RESULTS_DIR/${repo_name}"_*.json; do
            if [ -f "$result_file" ] && [ "$(cat "$result_file")" != "ERROR" ]; then
                count=$(jq '. | length' "$result_file" 2>/dev/null || echo "0")
                total_findings=$((total_findings + count))
            fi
        done
        echo "  $repo_name: $total_findings findings"
    done
} > "$summary_file"

cat "$summary_file"

echo -e "${GREEN}Done!${NC}"
echo -e "Next steps:"
echo -e "  1. Review results in: $RESULTS_DIR"
echo -e "  2. Run triage: python calculate_metrics.py"
echo -e "  3. Update baselines as needed"
