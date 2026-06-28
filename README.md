# Graphenium

**Provenance-aware structural memory for AI coding agents.**

Turns your repository into a persistent, queryable architecture graph so AI
assistants can answer structural questions in milliseconds instead of grepping
through source files.

```sh
# 1. Initialize workspace with safe ignore defaults
gm init

# 2. Build a local graph, no API key needed
gm run . --no-semantic --no-viz

# 3. Ask structural questions
gm query "authentication flow"
```

---

## Why Graphenium?

AI coding assistants navigate codebases like a human using `grep`: search,
open files, follow imports, open more files, infer relationships, repeat in
the next session. In a 5,000-file monorepo, this wastes time, tokens, and
attention.

Graphenium replaces repeated navigation with durable structural memory. It
builds an architecture graph from your source code, exposes it over MCP so
your assistant can query topology, and tracks confidence and provenance on
every relationship.

> **Context-token reduction:** Traditional grep-and-trace navigation forces an
> assistant to load raw source files merely to trace a call chain. Graphenium
> answers the same question in a few hundred tokens instead of tens of thousands.

---

## Quick start

```sh
# Prerequisites: Rust 1.70+
cargo install graphenium

# Or install via script
curl -fsSL https://raw.githubusercontent.com/lambda-alpha-labs/Graphenium/main/install.sh | sh

# Initialize workspace
cd your-project
gm init

# Build a graph (AST-only, local, no API key)
gm run . --no-semantic --no-viz

# Query the graph
gm query "user authentication session"

# Check graph quality
gm doctor

# Start MCP server for your AI assistant
gm serve --graph graphenium-out/graph.json --watch
```

---

## MCP setup

Add Graphenium to your AI assistant's MCP configuration:

### Claude Desktop

```json
{
  "mcpServers": {
    "graphenium": {
      "command": "gm",
      "args": ["serve", "--graph", "/path/to/graphenium-out/graph.json", "--watch"]
    }
  }
}
```

### Cursor

```json
{
  "mcpServers": {
    "graphenium": {
      "command": "gm",
      "args": ["serve", "--graph", "/path/to/graphenium-out/graph.json", "--watch"]
    }
  }
}
```

### CodeWhale

```toml
[mcp_servers.graphenium]
command = "gm"
args = ["serve", "--graph", "/path/to/graphenium-out/graph.json", "--watch"]
```

---

## Key features

- **AST extraction** for 10 languages (Rust, Python, Go, JavaScript, TypeScript, C, C++, Java, C#, Ruby)
- **MCP-native** — 30+ graph tools exposed over Model Context Protocol
- **Provenance** — every edge carries confidence (Extracted/Inferred/Ambiguous) and provenance (tree-sitter, resolver, LLM)
- **Communities** — automatic Louvain clustering groups related modules
- **Diff analysis** — symbol-level diff with downstream impact and review order
- **Trust gating** — `gm check` enforces quality thresholds in CI
- **Hybrid retrieval** — lexical (TF-cosine), structural (graph-distance), or hybrid query mode
- **Watch mode** — incremental rebuilds as files change
- **Multi-tier** — AST-only (local, free) or semantic (LLM-enriched)

---

## Documentation

| Document | What it covers |
|---|---|
| [`docs/COMMAND_REFERENCE.md`](docs/COMMAND_REFERENCE.md) | Complete CLI reference for all `gm` commands and flags |
| [`docs/MCP_TOOLS.md`](docs/MCP_TOOLS.md) | Full MCP tool catalog: read, write, composite, trust, diff |
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | Three-tier model, graph schema, module map, current limitations |
| [`docs/COMPARISON.md`](docs/COMPARISON.md) | How Graphenium compares to grep, AST tools, semantic search |
| [`docs/BENCHMARKING.md`](docs/BENCHMARKING.md) | Token-reduction benchmarks and methodology |

---

## Supported languages

Rust, Python, Go, JavaScript, TypeScript, C, C++, Java, C#, Ruby —
extracted via tree-sitter AST grammars. Add a new language by implementing
an extractor module.

---

## License

MIT &mdash; see [LICENSE](LICENSE).
