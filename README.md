# Graphenium (gm): The active coordination & verification loop for autonomous AI coding agents

Most code tools help AI agents search files. Graphenium is the only tool that helps agents *plan, write, and verify* structural changes. It provides a stateful coordination whiteboard, real-time reactive indexing, and a mathematical verification loop: so coding agents stop guessing and start engineering.

Binary: `gm` | Schema: `0.2.0` | Status: `AST + Resolver [Stable]`, `Semantic Pass [Stable]`, `Telemetry Overlay [Experimental]`

## Quick Start

```sh
# 1. Initialize workspace (creates .grapheniumignore)
gm init

# 2. Build the graph
gm run . --no-semantic --no-viz

# 3. Start the MCP server
gm serve

# 4. Query the graph (via MCP)
gm query "authentication flow" --budget 2000

# 6. Run a Datalog query
gm query --datalog "?- node(X, _, _, _, _)."

# 5. Run diagnostics
gm doctor --graph graphenium-out/graph.json
```

## MCP Setup

### Claude Desktop
Add to `claude_desktop_config.json`:
```json
{
  "mcpServers": {
    "graphenium": {
      "command": "gm",
      "args": ["serve", "--graph", "/path/to/graphenium-out/graph.json"]
    }
  }
}
```

### Cursor
Add to `~/.cursor/mcp.json`:
```json
{
  "mcpServers": {
    "graphenium": {
      "command": "gm",
      "args": ["serve", "--graph", "/path/to/graphenium-out/graph.json"]
    }
  }
}
```

### CodeWhale
Add to `~/.codewhale/mcp.json`:
```json
{
  "graphenium": {
    "command": "gm",
    "args": ["serve", "--graph", "/path/to/graphenium-out/graph.json"]
  }
}
```

## Core Features (Agent Lifecycle)

### Pre-Edit: Trust-Aware Pathfinding

- **Provenance on every edge**: every relationship carries `extractor`, `resolution_status`, and three confidence tiers (`EXTRACTED`, `INFERRED`, `AMBIGUOUS`). Agents route traversals through high-trust, source-backed edges to prevent hallucination cascades.
- **Cross-file call resolution**: resolves calls, uses, inherits, and implements across file boundaries using tree-sitter and scope-aware symbol indexing. Includes language-family guardrails to prevent cross-language false positives in multi-language monorepos.
- **Architectural analysis**: Louvain community detection, PageRank hubs, chokepoint analysis (Brandes' betweenness centrality), and architecture drift detection give agents the high-level shape of the codebase in one query.
- **Topological anomaly detection**: multi-variable surprise scoring identifies unexpected cross-boundary connections, architectural erosion, and out-of-layer dependencies without custom rules.
- **Hybrid retrieval**: lexical (TF-cosine), structural (graph-distance), and combined modes via `--mode` flag. Datalog declarative queries for first-order logic reachability and constraint queries.
- **Runtime telemetry overlay**: import OpenTelemetry trace JSON to create a RuntimeOverlay with per-node call counts and latency percentiles (P50/P95/P99). Hot-path and runtime-weighted traversal.
- **C# assembly boundary parsing**: reads `.sln` and `.csproj` files to map project references and assembly dependencies as first-class graph elements, not just flat source files.
- **Academic paper classification**: heuristic detection of research papers (arXiv, DOI, LaTeX markers) linked into the graph alongside implementation code.

### In-Edit: Reactive Multi-Turn Planning

- **Salsa-backed incremental indexing**: demand-driven memoized extraction via Salsa. When an agent edits a file, only the changed file and its downstream importers are re-parsed, updating the graph in milliseconds. Designed for active writers, not passive readers.
- **Design-then-verify planning workspaces**: agents register intended symbols in a virtual workspace before writing code. The compliance audit compares the planned design against the extracted physical graph, reporting implemented, missing, and unplanned symbols. Transitions agents from chaotic file modification to a structured engineering loop.

### Post-Edit: Automated Compliance & Gating

- **Symbol diff + impact**: `gm diff` compares graph snapshots and computes blast radius (downstream impact) for changed symbols.
- **Trust gates for CI**: `gm check` enforces resolution quality and edge confidence policies. Block PRs when the graph is too unreliable to plan changes, or when agent-generated code violates architectural boundaries.
- **34 MCP tools**: read, composite, trust, write, diff, and planning tools across the full agent lifecycle.

## Documentation

- [`docs/COMMAND_REFERENCE.md`](docs/COMMAND_REFERENCE.md): Complete CLI reference for all 13 commands
- [`docs/MCP_TOOLS.md`](docs/MCP_TOOLS.md): Full MCP tool catalog with parameter tables and selection guide
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md): Three-tier model, extraction pipeline, trust model, module map
- [`docs/COMPARISON.md`](docs/COMPARISON.md): How Graphenium compares to grep, tree-sitter, Sourcegraph, and others
- [`docs/BENCHMARKING.md`](docs/BENCHMARKING.md): Token-reduction benchmarks and methodology
- [`docs/GETTING_STARTED.md`](docs/GETTING_STARTED.md): Step-by-step guided workflows for AI agents

## Install

```sh
# From source
cargo install --locked --path .

# Or via curl
curl -fsSL https://raw.githubusercontent.com/lambda-alpha-labs/Graphenium/main/install.sh | sh
```

Requires Rust 1.81+ and tree-sitter language grammars (bundled).

## License

MIT: see [LICENSE](LICENSE).
