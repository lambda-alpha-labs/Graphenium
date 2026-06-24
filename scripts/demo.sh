#!/bin/bash
# Graphenium demo: execution-focused, ~30s pacing
set -e

GM="/Users/liamandrew/Documents/Code/Graphenium/target/release/gm"
GRAPH="/Users/liamandrew/Documents/Code/Graphenium/worked/graphenium-self-analysis/graph.json"

sanitize() { sed 's|/Users/liamandrew|~|g'; }

clear

# ── Title ──
echo "    Graphenium: Persistent structural memory for AI coding agents"
echo ""
printf "    AI assistants waste context grepping the same files every session."
sleep 3  && echo "" && echo ""

# ── Build snapshot ──
echo "    \$ gm run . --no-semantic --no-viz"
# Show cached result: this is what gm run produces on this repo
echo "    Graph: 868 nodes, 1768 edges, 19 communities (AST-only, 0 API calls)"
echo ""
sleep 2

# ── Query 1 ──
echo "    \$ gm query --graph graph.json \"what calls build_from_extraction\""
echo ""
$GM query "what calls build_from_extraction" --graph "$GRAPH" --budget 300 2>&1 | sanitize
sleep 4

# ── Query 2 ──
echo ""
echo "    \$ gm query --graph graph.json \"mcp server shortest path\""
echo ""
$GM query "mcp server shortest path" --graph "$GRAPH" --budget 300 2>&1 | sanitize
sleep 4

# ── Query 3 ──
echo ""
echo "    \$ gm query --graph graph.json \"community louvain detection\""
echo ""
$GM query "community louvain detection" --graph "$GRAPH" --budget 250 2>&1 | sanitize
sleep 4

# ── Before / After ──
echo ""
echo "    ┌─────────────────────────────────────────────────────────┐"
echo "    │  WITHOUT: grep → read → trace → read more → repeat      │"
echo "    │  WITH:    query_graph → neighbors → path → right files  │"
echo "    └─────────────────────────────────────────────────────────┘"
echo ""
sleep 4

# ── CTA ──
echo "    github.com/lambda-alpha-labs/Graphenium     (MIT, Rust, MCP)"
echo ""
sleep 3
