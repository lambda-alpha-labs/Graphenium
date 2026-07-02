#!/usr/bin/env bash
# Graphenium benchmark runner
# Usage: ./scripts/run_benchmarks.sh [--json]
# Requires: gm in PATH, graph at graphenium-out/graph.json
set -euo pipefail

GM="${GM:-gm}"
GRAPH="${GRAPH:-graphenium-out/graph.json}"
JSON_OUT="${JSON_OUT:-}"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --json) JSON_OUT="benchmark_results.json"; shift ;;
        *) echo "Unknown: $1"; exit 1 ;;
    esac
done

if ! command -v "$GM" &>/dev/null; then
    echo "ERROR: $GM not found. Set GM env var or add to PATH."
    exit 1
fi

if [[ ! -f "$GRAPH" ]]; then
    echo "ERROR: graph not found at $GRAPH"
    exit 1
fi

echo "=== Graphenium Benchmark ==="
echo "Binary: $(which "$GM")"
echo "Graph: $GRAPH"
echo "Date: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo ""

declare -A RESULTS

run_query() {
    local name="$1"
    local query="$2"
    local budget="${3:-2000}"
    local mode="${4:-lexical}"

    local start_ms end_ms chars result
    python3 -c "import time; print(int(time.time() * 1000))" > /tmp/bench_start
    result=$("$GM" query "$query" --budget "$budget" --mode "$mode" --graph "$GRAPH" 2>/dev/null || echo "ERROR")
    python3 -c "import time; print(int(time.time() * 1000))" > /tmp/bench_end

    start_ms=$(cat /tmp/bench_start)
    end_ms=$(cat /tmp/bench_end)
    elapsed=$((end_ms - start_ms))
    chars=$(echo "$result" | wc -c | tr -d ' ')

    RESULTS["${name}_chars"]="$chars"
    RESULTS["${name}_ms"]="$elapsed"
    RESULTS["${name}_pass"]="PASS"

    local budget_pass="PASS"
    if [[ "$chars" -gt "$budget" ]]; then
        budget_pass="FAIL"
        RESULTS["${name}_pass"]="FAIL"
    fi

    printf "%-40s %6d chars  %5d ms  %s\n" "$name" "$chars" "$elapsed" "$budget_pass"
}

run_query "impact: replace_file_extraction" "replace_file_extraction" 10000 hybrid
run_query "community: GrapheniumCluster" "GrapheniumCluster" 8000 lexical
run_query "arch: GrapheniumGraph" "GrapheniumGraph" 10000 lexical
run_query "symbol: node_data" "node_data" 10000 lexical
run_query "keyword: authentication flow" "authentication flow" 10000 lexical
run_query "topology: gm serve" "gm serve" 8000 lexical

echo ""
echo "=== Summary ==="
passed=0
failed=0
for key in "${!RESULTS[@]}"; do
    if [[ "$key" == *_pass ]]; then
        name="${key%_pass}"
        status="${RESULTS[$key]}"
        case "$status" in
            PASS) ((passed++)) ;;
            FAIL) ((failed++)) ;;
        esac
    fi
done
echo "Passed: $passed / $((passed + failed))"

if [[ -n "$JSON_OUT" ]]; then
    cat > "$JSON_OUT" <<EOF
{
  "benchmark": {
    "binary": "$(which "$GM")",
    "graph": "$GRAPH",
    "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
    "results": {
EOF
    first=true
    for key in "${!RESULTS[@]}"; do
        $first || echo "," >> "$JSON_OUT"
        first=false
        echo "      \"$key\": \"${RESULTS[$key]}\"" >> "$JSON_OUT"
    done
    echo "    }" >> "$JSON_OUT"
    echo "  }" >> "$JSON_OUT"
    echo "}" >> "$JSON_OUT"
    echo "Wrote: $JSON_OUT"
fi

[[ "$failed" -eq 0 ]] || exit 1
