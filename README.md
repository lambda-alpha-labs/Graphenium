# Graphenium — Trust-aware codebase memory for AI coding agents

Turn your repository into a persistent, queryable architecture graph so AI assistants stop grepping and start querying.

Most code tools help humans search files. Graphenium gives AI agents a compact, trust-aware map of the repository — with confidence and provenance on every relationship.

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

## Core Features

- **Cross-file call resolution** — resolves calls, uses, and references across file boundaries using tree-sitter and scope-aware symbol indexing. All resolved edges carry `extractor = "tree-sitter-stack-graphs"` provenance. Includes language-family guardrails to prevent cross-language false positives in multi-language monorepos.
- **Declarative Datalog queries** — `gm query --datalog "<program>"` runs first-order logic queries against the graph with support for rules, goals, facts, and negation.
- **Hybrid retrieval** — lexical (TF-cosine), structural (graph-distance), and combined modes via `--mode` flag.
- **Runtime telemetry overlay** — import OpenTelemetry trace JSON to create a RuntimeOverlay with per-node call counts and latency percentiles (P50/P95/P99). Enables hot-path and runtime-weighted traversal.
- **Demand-driven incremental computation** — Salsa-powered memoized extraction caches AST results by content hash, near-instant rebuilds on unchanged files.
- **Provenance on every edge** — every relationship carries `extractor` and `resolution_status` so agents know how much to trust each connection
- **Cross-file resolution** — resolves calls, uses, inherits, and implements across file boundaries
- **Architectural analysis** — Louvain community detection, PageRank hubs, chokepoint analysis (Brandes' betweenness centrality), architecture drift detection
- **Topological anomaly detection** — multi-variable surprise scoring identifies unexpected cross-boundary connections, architectural erosion, and out-of-layer dependencies without custom rules
- **Design-then-verify planning workspaces** — agents declare intended symbols in a virtual workspace before writing code; compliance audit compares the planned design against the extracted physical graph, reporting implemented, missing, and unplanned symbols
- **C# assembly boundary parsing** — reads `.sln` and `.csproj` files to map project references and assembly dependencies as first-class graph elements, not just flat source files
- **Academic paper classification** — heuristic detection of research papers (arXiv, DOI, LaTeX markers) linked into the graph alongside implementation code
- **Symbol diff + impact** — `gm diff` compares graph snapshots and computes blast radius
- **Trust gates for CI** — `gm check` enforces resolution quality and edge confidence policies
- **34 MCP tools** — read, composite, trust, write, diff, and planning tools for AI agents
- **Hybrid retrieval** — lexical (TF-cosine), structural (graph-distance), and combined modes

## Documentation

- [`docs/COMMAND_REFERENCE.md`](docs/COMMAND_REFERENCE.md) — Complete CLI reference for all 13 commands
- [`docs/MCP_TOOLS.md`](docs/MCP_TOOLS.md) — Full MCP tool catalog with parameter tables and selection guide
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — Three-tier model, extraction pipeline, trust model, module map
- [`docs/COMPARISON.md`](docs/COMPARISON.md) — How Graphenium compares to grep, tree-sitter, Sourcegraph, and others
- [`docs/BENCHMARKING.md`](docs/BENCHMARKING.md) — Token-reduction benchmarks and methodology
- [`docs/GETTING_STARTED.md`](docs/GETTING_STARTED.md) — Step-by-step guided workflows for AI agents

## Install

```sh
# From source
cargo install --locked --path .

# Or via curl
curl -fsSL https://raw.githubusercontent.com/lambda-alpha-labs/Graphenium/main/install.sh | sh
```

Requires Rust 1.81+ and tree-sitter language grammars (bundled).

## License

MIT — see [LICENSE](LICENSE).
