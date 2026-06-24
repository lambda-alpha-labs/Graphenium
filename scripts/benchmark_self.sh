#!/bin/bash
# Graphenium self-benchmark
# Run from the Graphenium repository root after generating a graph:
#   gm run . --no-semantic --no-viz
#   bash scripts/benchmark_self.sh
set -e

echo "=== Graphenium self-benchmark ==="
echo ""

echo "--- query_graph: 'mcp server handlers' ---"
time gm query "mcp server handlers" --budget 2000 > /dev/null

echo ""
echo "--- query_graph: 'graph build extraction' ---"
time gm query "graph build extraction" --budget 2000 > /dev/null

echo ""
echo "--- query_graph: 'community detection' ---"
time gm query "community detection" --budget 2000 > /dev/null

echo ""
echo "--- query_graph: 'query traversal budget' ---"
time gm query "query traversal budget" --budget 2000 > /dev/null

echo ""
echo "=== Done ==="
echo "Graph: $(wc -c < graphenium-out/graph.json) bytes"
echo "Nodes: $(python3 -c "import json; g=json.load(open('graphenium-out/graph.json')); print(len(g['nodes']))" 2>/dev/null || echo "run with graph.json present")"
echo "Edges: $(python3 -c "import json; g=json.load(open('graphenium-out/graph.json')); print(len(g['edges']))" 2>/dev/null || echo "run with graph.json present")"
