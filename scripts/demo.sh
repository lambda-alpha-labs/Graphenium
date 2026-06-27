#!/bin/bash
# Graphenium v2 demo: the structural memory story, ~35s
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
GM="${SCRIPT_DIR}/../target/release/gm"
GRAPH="${SCRIPT_DIR}/../worked/graphenium-self-analysis/graph.json"

sanitize() { sed 's|/Users/|~|g'; }

clear

# ── 1. Title ──
echo "    Graphenium: Provenance-aware structural memory for AI agents"
echo ""
echo "    Most code tools help humans search files."
echo "    Graphenium gives AI agents a durable, queryable map of your repo."
sleep 3
echo ""
echo "    It replaces grep-and-trace navigation, not source-code understanding."
sleep 2
echo ""

# ── 2. Build ──
echo "    \$ gm run . --no-semantic --no-viz"
echo ""
echo "    Graph: 957 nodes | 1941 edges | 22 communities | schema 0.2.0"
echo "    AST-only, no API key needed. Runs in under 2 seconds."
sleep 2
echo ""

# ── 3. Query: find what depends on a module ──
echo "    \$ gm query \"what depends on the serve module\" --budget 400"
echo ""
$GM query "serve module handlers mcp" --graph "$GRAPH" --budget 400 2>&1 | sanitize | head -35
sleep 4

# ── 4. The provenance pitch ──
echo ""
echo "    Every edge carries provenance."
echo "    [tree-sitter:resolved] means the relationship is source-backed."
echo "    [llm:inferred] means the LLM proposed it — treat as a hint."
echo "    The agent always knows how much to trust the graph."
sleep 4
echo ""

# ── 5. Architecture summary ──
echo "    \$ gm doctor"
echo ""
$GM doctor --graph "$GRAPH" 2>&1 | sanitize | grep -E "graph schema|built by|modes|languages|quality"
sleep 3

# ── 6. The pitch ──
echo ""
echo "    ┌─────────────────────────────────────────────────────────────┐"
echo "    │  Without Graphenium                                        │"
echo "    │  grep -> read file -> trace imports -> read more -> repeat │"
echo "    │                                                             │"
echo "    │  With Graphenium                                            │"
echo "    │  query_graph -> get_neighbors -> shortest_path              │"
echo "    │  2 graph calls before opening a single source file          │"
echo "    └─────────────────────────────────────────────────────────────┘"
sleep 4
echo ""

# ── 7. CTA ──
echo "    github.com/lambda-alpha-labs/Graphenium"
echo "    MIT  ·  Rust  ·  MCP-native  ·  Provenance on every edge"
echo ""
echo "    Stop grepping. Start querying."
sleep 3
