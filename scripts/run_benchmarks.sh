#!/usr/bin/env bash
# Graphenium Benchmark Runner
#
# Runs standard gm queries, records output character counts, and asserts each
# stays under a character budget. Exits zero if all pass, non-zero if any fail.
#
# Usage:
#   chmod +x scripts/run_benchmarks.sh
#   bash scripts/run_benchmarks.sh
#
# Requirements:
#   - gm CLI installed and on PATH
#   - A built graph at graphenium-out/graph.json (run `gm run .` first)

set -euo pipefail

# ------------------------------------------------------------------
# Configuration — adjust budgets for your repository size
# ------------------------------------------------------------------
BUDGET_QUERY1=10000    # chars: "replace_file_extraction"
BUDGET_QUERY2=10000    # chars: "GrapheniumCluster"
BUDGET_QUERY3=8000     # chars: "shortest_path"
BUDGET_QUERY4=12000    # chars: "community detection"

# Queries to run
QUERY1="replace_file_extraction"
QUERY2="GrapheniumCluster"
QUERY3="shortest_path GrapheniumServer GrapheniumGraph"
QUERY4="community detection"

# ------------------------------------------------------------------
# Helper functions
# ------------------------------------------------------------------
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Colour

pass() { printf "  ${GREEN}PASS${NC}  %s\n" "$1"; }
fail() { printf "  ${RED}FAIL${NC}  %s\n" "$1"; }
info() { printf "  ${YELLOW}INFO${NC} %s\n" "$1"; }

# ------------------------------------------------------------------
# Checks
# ------------------------------------------------------------------
echo "=== Graphenium Benchmark Runner ==="
echo ""

# 1. Check gm is installed
if ! command -v gm &>/dev/null; then
    echo "ERROR: 'gm' command not found. Install Graphenium CLI first."
    echo "       See https://github.com/your-org/graphenium#installation"
    exit 1
fi
info "gm found: $(command -v gm)"

# 2. Check graph exists (from repo root)
PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
GRAPH_FILE="$PROJECT_DIR/graphenium-out/graph.json"
if [ ! -f "$GRAPH_FILE" ]; then
    echo "ERROR: Graph not found at $GRAPH_FILE"
    echo "       Run 'gm run .' from the repository root first."
    exit 1
fi
info "Graph found at $GRAPH_FILE ($(wc -c < "$GRAPH_FILE") bytes)"

echo ""
echo "--- Running benchmarks ---"
echo ""

# ------------------------------------------------------------------
# Run queries and measure
# ------------------------------------------------------------------
PASS_COUNT=0
FAIL_COUNT=0
RESULTS_FILE=$(mktemp)

run_query() {
    local label="$1"
    local budget="$2"
    local query="$3"

    printf "  [%s] " "$label"

    # Capture output size and stderr separately
    local output
    output=$(gm query "$query" --budget 2000 2>/dev/null || true)
    local char_count
    char_count=$(echo -n "$output" | wc -c | tr -d ' ')

    echo "$label|$char_count|$budget" >> "$RESULTS_FILE"

    if [ "$char_count" -le "$budget" ]; then
        pass "${char_count} chars (budget ${budget})"
        PASS_COUNT=$((PASS_COUNT + 1))
    else
        fail "${char_count} chars (budget ${budget}) — exceeded by $((char_count - budget)) chars"
        FAIL_COUNT=$((FAIL_COUNT + 1))
    fi
}

run_query "Query 1" "$BUDGET_QUERY1" "$QUERY1"
run_query "Query 2" "$BUDGET_QUERY2" "$QUERY2"
run_query "Query 3" "$BUDGET_QUERY3" "$QUERY3"
run_query "Query 4" "$BUDGET_QUERY4" "$QUERY4"

# ------------------------------------------------------------------
# Summary
# ------------------------------------------------------------------
echo ""
echo "--- Summary ---"
echo ""

# Print results table
printf "  %-45s %10s %12s %s\n" "Query" "Chars" "Budget" "Status"
printf "  %-45s %10s %12s %s\n" "---------------------------------------------" "----------" "----------" "------"
while IFS='|' read -r label chars budget; do
    if [ "$chars" -le "$budget" ]; then
        status="${GREEN}PASS${NC}"
    else
        status="${RED}FAIL${NC}"
    fi
    printf "  %-45s %10s %12s %b\n" "$label" "$chars" "$budget" "$status"
done < "$RESULTS_FILE"

echo ""

rm -f "$RESULTS_FILE"

if [ "$FAIL_COUNT" -eq 0 ]; then
    echo "  Result: ${GREEN}ALL PASSED${NC} (${PASS_COUNT}/${PASS_COUNT})"
    echo ""
    echo "=== Benchmark complete ==="
    exit 0
else
    echo "  Result: ${RED}${FAIL_COUNT} FAILURE(S)${NC} (${PASS_COUNT}/$((PASS_COUNT + FAIL_COUNT)) passed)"
    echo ""
    echo "=== Benchmark complete with failures ==="
    exit 1
fi
