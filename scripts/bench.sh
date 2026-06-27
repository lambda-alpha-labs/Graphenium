#!/usr/bin/env bash
# Benchmark CLI for Graphenium.
# Usage: bash scripts/bench.sh [--json]
# Measures: cold index, incremental update, graph load, query latency

set -euo pipefail

OUTPUT_JSON=false
if [[ "${1:-}" == "--json" ]]; then
    OUTPUT_JSON=true
fi

PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$PROJECT_DIR"

GM="${CARGO_HOME:-$HOME/.cargo}/bin/gm"
if [ ! -x "$GM" ]; then
    GM="./target/release/gm"
fi

# Use a fixed test fixture for reproducibility
FIXTURE_DIR="tests/fixtures/large_synthetic_1k"
if [ ! -d "$FIXTURE_DIR" ]; then
    if [ "$OUTPUT_JSON" = false ]; then
        echo "SKIP: fixtures not found. Run 'cargo test' first."
    fi
    exit 0
fi

RESULTS="{}"

cold_index() {
    local start end elapsed
    start=$(date +%s%N)
    "$GM" run "$FIXTURE_DIR" --no-semantic --no-viz --no-report 2>/dev/null
    end=$(date +%s%N)
    elapsed=$(( (end - start) / 1000000 ))
    RESULTS=$(echo "$RESULTS" | jq --argjson ms "$elapsed" '. + {"cold_index_ms": $ms}')
    [ "$OUTPUT_JSON" = false ] && echo "cold_index: ${elapsed}ms"
}

graph_load() {
    local start end elapsed
    start=$(date +%s%N)
    "$GM" doctor --graph "$FIXTURE_DIR/graphenium-out/graph.json" 2>/dev/null
    end=$(date +%s%N)
    elapsed=$(( (end - start) / 1000000 ))
    RESULTS=$(echo "$RESULTS" | jq --argjson ms "$elapsed" '. + {"graph_load_ms": $ms}')
    [ "$OUTPUT_JSON" = false ] && echo "graph_load: ${elapsed}ms"
}

query_latency() {
    local start end elapsed
    start=$(date +%s%N)
    "$GM" query "function" --graph "$FIXTURE_DIR/graphenium-out/graph.json" --budget 500 2>/dev/null
    end=$(date +%s%N)
    elapsed=$(( (end - start) / 1000000 ))
    RESULTS=$(echo "$RESULTS" | jq --argjson ms "$elapsed" '. + {"query_ms": $ms}')
    [ "$OUTPUT_JSON" = false ] && echo "query: ${elapsed}ms"
}

cold_index
graph_load
query_latency

if [ "$OUTPUT_JSON" = true ]; then
    echo "$RESULTS"
fi
