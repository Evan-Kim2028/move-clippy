#!/bin/bash
# Ecosystem Validation Runner
# Executes move-clippy on all ecosystem repos and captures results

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RESULTS_DIR="$SCRIPT_DIR/results"
CLIPPY_BIN="$SCRIPT_DIR/../../target/release/move-clippy"
# Ecosystem repos are in packages/ecosystem-test-repos
REPOS_DIR="/Users/evandekim/Documents/learning_move/packages/ecosystem-test-repos"

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# NOTE: Semantic lints (--mode full) require --features full at build time.
# Now that compilation issues are fixed, running full mode for semantic lint validation.
LINT_MODE="full"

# Build with full feature for semantic lints
echo -e "${YELLOW}Building move-clippy with full features...${NC}"
cd "$SCRIPT_DIR/../.."
cargo build --release --features full

# Repository list (simple arrays instead of associative)
REPOS=(
    "alphalend:AlphaLend lending protocol"
    "scallop-lend:Scallop lending protocol"
    "suilend:Suilend lending protocol"
    "deepbookv3:DeepBook V3 DEX"
    "cetus-clmm:Cetus CLMM DEX"
    "bluefin-spot:Bluefin spot trading"
    "bluefin-pro:Bluefin perpetuals"
    "bluefin-integer:Bluefin integer math"
    "openzeppelin-sui:OpenZeppelin Sui library"
    "steamm:Steamm AMM"
    "suilend-liquid-staking:Suilend liquid staking"
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
for repo_entry in "${REPOS[@]}"; do
    TOTAL_REPOS=$((TOTAL_REPOS + 1))
    repo_name="${repo_entry%%:*}"
    description="${repo_entry#*:}"
    repo_path="$REPOS_DIR/$repo_name"
    
    echo -e "${YELLOW}Processing: $repo_name${NC} - $description"
    
    if [ ! -d "$repo_path" ]; then
        echo -e "${RED}  ✗ Repo not found: $repo_path${NC}"
        FAILED_RUNS=$((FAILED_RUNS + 1))
        continue
    fi
    
    # Find Move.toml files (may have multiple packages)
    move_tomls=$(find "$repo_path" -name "Move.toml" -type f | grep -v "deps-")
    
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
        # Use --package flag and --format json
        output_file="$RESULTS_DIR/${repo_name}_${package_name}.json"
        log_file="$RESULTS_DIR/${repo_name}_${package_name}.log"
        
        # Run with explicit package flag for semantic lints
        if "$CLIPPY_BIN" --mode "$LINT_MODE" --package "$package_dir" --preview --format json "$package_dir" > "$output_file" 2> "$log_file"; then
            # Count findings
            count=$(jq 'if type=="array" then length else 0 end' "$output_file" 2>/dev/null || echo "0")
            findings_count=$((findings_count + count))
            echo -e "    ${GREEN}✓${NC} Found $count lint findings"
        else
            # Check if it's just a compilation error vs actual crash
            exit_code=$?
            if [ $exit_code -eq 1 ]; then
                # Linter found issues but ran successfully
                count=$(jq 'if type=="array" then length else 0 end' "$output_file" 2>/dev/null || echo "0")
                findings_count=$((findings_count + count))
                echo -e "    ${YELLOW}⚠${NC} Found $count lint findings (with warnings)"
            else
                # Actual error - compilation failed, etc.
                echo -e "    ${RED}✗${NC} Linting failed (exit $exit_code) - see $log_file"
                echo '{"error": "compilation_failed"}' > "$output_file"
            fi
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
    
    for repo_name in "${REPOS[@]}"; do
        repo_name="${repo_name%:*}"
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
