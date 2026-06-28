# Graphenium

**Trust-aware codebase memory for AI coding agents.**

Graphenium gives Claude, Cursor, CodeWhale, and other MCP-compatible coding agents a compact, persistent map of your repository so they do not waste tokens rediscovering architecture every session.

Instead of forcing an agent to grep, open files, trace imports, and rebuild the same mental model again and again, Graphenium builds a local repository graph with confidence and provenance on every relationship.

Agents can ask:

- What depends on this module?
- What calls this function?
- What is the shortest path between these components?
- What is the blast radius of this change?
- Which facts are source-backed, inferred, or ambiguous?
- Does this repository still meet our trust-quality bar in CI?

Graphenium is not a replacement for reading source code. It is the map agents use **before** they read code.

---

## Why this matters now

AI coding agents are moving from autocomplete to delegated software work. In small projects, they can read their way into context. In large repositories, that approach becomes expensive and unreliable.

Every cold start costs tokens. Every unnecessary file read costs tokens. Every guessed dependency path creates risk.

Graphenium exists to reduce that waste. It turns repository navigation into compact graph queries, giving agents structural orientation before they spend context on implementation details.

**The goal:** fewer tokens spent on navigation, more context left for reasoning, review, and code synthesis.

---

## The core idea

```text
Without Graphenium:
agent -> grep -> read file -> trace imports -> read more files -> infer architecture -> repeat next session

With Graphenium:
agent -> query graph -> inspect trust profile -> read only the files that matter -> plan change
```

Graphenium builds a durable graph of your repository:

- **Nodes** are files, modules, functions, classes, methods, structs, traits, tests, documents, build targets, CI jobs, and architectural concepts.
- **Edges** are relationships such as `imports`, `contains`, `calls`, `uses`, `inherits`, `implements`, `tests`, `depends_on`, and `runs_in`.
- **Every relationship carries trust metadata** so an agent can distinguish source-backed facts from inferred or ambiguous leads.

---

## What makes Graphenium different

Most code tools help humans find files. Graphenium helps AI agents control context, reason about structure, and avoid over-trusting weak signals.

| Need | Grep / search | Vector search | Generic code graph | Repo map | Graphenium |
|---|---:|---:|---:|---:|---:|
| Find text or symbols | Yes | Sometimes | Sometimes | Sometimes | Yes |
| Understand repository topology | No | Weak | Yes | Yes | Yes |
| Persist structure across sessions | No | Sometimes | Sometimes | Tool-dependent | Yes |
| Serve compact context to AI agents | No | Sometimes | Sometimes | Yes | Yes |
| MCP-first agent interface | No | Sometimes | Sometimes | No / tool-specific | Yes |
| Confidence and provenance per relationship | No | No | Rare | No | Yes |
| Separate extracted, inferred, and ambiguous facts | No | No | Rare | No | Yes |
| Shortest path and dependency tracing | No | No | Sometimes | Limited | Yes |
| Blast-radius analysis | No | No | Sometimes | No | Built in |
| CI trust-quality gates | No | No | No | No | Built in with `gm check` |
| Token-budgeted traversal | No | No | Sometimes | Yes | Yes |

Graphenium is not trying to be the biggest static analyzer or the broadest semantic search database. It is built for a narrower and higher-leverage role:

> **The context-budget, trust, and change-safety layer for AI coding agents working in large repositories.**

---

## Trust-aware graph facts

Every node and edge is labeled with confidence and provenance.

| Confidence | Meaning | How an agent should use it |
|---|---|---|
| `EXTRACTED` | Produced by tree-sitter, resolver, or manually confirmed inspection | Treat as source-backed |
| `INFERRED` | Produced by LLM reasoning or behavioral heuristics | Treat as a high-probability lead |
| `AMBIGUOUS` | Produced from uncertain, conflicting, or unresolved evidence | Investigate before relying on it |

Provenance metadata records how a connection was produced: `extractor` (tree-sitter, resolver, llm, manual-mcp-write) and `resolution_status` (resolved, unresolved, heuristic, inferred).

Example agent-facing output:
```text
[Graphenium] require_session calls validate_token [resolver:resolved] -> High trust
[Graphenium] auth_service uses db_client [llm:inferred] -> Inspect before relying
```

---

## Quick start

### Install

```sh
curl -fsSL https://raw.githubusercontent.com/lambda-alpha-labs/Graphenium/main/install.sh | sh
```

### Or build from source

Requires Rust 1.75+.

```sh
git clone https://github.com/lambda-alpha-labs/Graphenium
cd Graphenium
cargo install --path .
```

The binary is installed as `gm`.

### Build your first graph

```sh
# 1. Initialize workspace with safe ignore defaults
gm init

# 2. Build a local structural graph, no API key needed
gm run . --no-semantic --no-viz

# 3. Query the graph with a token budget
gm query "authentication login session" --budget 1000

# 4. Inspect graph health and trust metrics
gm doctor --resolution

# 5. Enforce CI-ready trust gates
gm check --min-resolution 80 --max-ambiguous 10
```

---

## MCP setup

Graphenium exposes your repository graph through an MCP stdio server.

```sh
gm setup claude
gm setup cursor
gm setup codewhale
```

Or configure manually. The `--watch` flag (default since v0.8.0) auto-reloads when the graph file changes.

### Claude Desktop

```json
{
  "mcpServers": {
    "graphenium": {
      "command": "gm",
      "args": ["serve", "--graph", "/absolute/path/to/graphenium-out/graph.json"]
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
      "args": ["serve", "--graph", "/absolute/path/to/graphenium-out/graph.json"]
    }
  }
}
```

### CodeWhale

```json
{
  "servers": {
    "graphenium": {
      "command": "/absolute/path/to/gm",
      "args": ["serve", "--graph", "/absolute/path/to/graphenium-out/graph.json"],
      "env": {}
    }
  }
}
```

After updating MCP config, fully quit and relaunch the AI tool. On macOS, use `Cmd+Q` rather than closing the window.

---

## Token-cost benchmarking

Graphenium is designed to reduce navigation tokens by replacing repeated source-file exploration with compact graph queries.

Results from Graphenium's own codebase (1,061 nodes, 2,104 edges, 22 communities):

| Task | Graphenium workflow | Output chars | Tokens (~4 chars/token) |
|---|---|---|---|
| Impact analysis of `replace_file_extraction` | `query_transitive` / `blast_radius` | 8,677 | ~2,170 |
| Community overview | `query_graph "GrapheniumCluster" --budget 1500` | 6,690 | ~1,670 |
| Module architecture of `GrapheniumGraph` | `query_graph "GrapheniumGraph" --budget 2000` | 8,395 | ~2,100 |
| Symbol with callers/dependents | `query_graph "node_data" --budget 2000` | 8,570 | ~2,140 |
| Cross-module keyword search | `query_graph "authentication flow" --budget 2000` | 8,409 | ~2,100 |
| Server topology | `query_graph "gm serve" --budget 1500` | 6,635 | ~1,660 |

Compare: answering the same questions via grep + reading source files typically requires **30,000–50,000 characters** (~8,000–12,000 tokens), giving roughly a **4–6x token reduction**.

See [`docs/BENCHMARKING.md`](docs/BENCHMARKING.md) for full methodology and [`scripts/run_benchmarks.sh`](scripts/run_benchmarks.sh) for an automated benchmark runner.

---

## Core capabilities

- **Persistent repository graph:** analysis runs once and reloads across sessions
- **Compact agent context:** token-budgeted traversal, degree-aware output, leaf-symbol omission
- **Confidence-aware retrieval:** `safest_path`, `shortest_path`, `get_neighbors`, `query_graph --safe`
- **Symbol-level diff and blast radius:** `gm diff` produces risk-sorted review plans
- **CI trust gates:** `gm check` enforces resolution, ambiguity, and stale-evidence thresholds
- **MCP-native tools:** 30+ graph tools including architecture summaries, community analysis, blast radius
- **Optional semantic and telemetry layers:** local AST-only, LLM-enriched, or OTEL trace overlay

---

## Language support

| Language | Extensions | Extracted features |
|---|---|---|
| Python | `.py` | Classes, functions, imports, call graph |
| JavaScript | `.js`, `.mjs`, `.cjs` | Classes, functions, arrow functions, imports |
| TypeScript | `.ts`, `.tsx` | JavaScript features plus type declarations |
| Rust | `.rs` | Structs, enums, traits, impl blocks, functions, `use` |
| Go | `.go` | Functions, methods with receivers, import blocks |
| Java | `.java` | Classes, methods, package imports |
| C | `.c`, `.h` | Functions, include directives |
| C++ | `.cpp`, `.cc`, `.cxx`, `.hpp` | Classes, functions, include directives |
| C# | `.cs` | Classes, methods, using directives, namespaces |
| Ruby | `.rb` | Classes, methods, modules, imports |

Repository extraction also detects `Cargo.toml`, `package.json`, and GitHub Actions workflows.

---

## Output files

Graphenium writes outputs to `graphenium-out/` inside the analyzed directory.

| File | Purpose |
|---|---|
| `graph.json` | Machine-readable graph for `gm serve` and `gm query` |
| `GRAPH_REPORT.md` | Markdown architecture report |
| `graph.html` | Self-contained visual graph inspection page |
| `manifest.json` | mtime index for incremental updates |
| `cache/` | Per-file semantic extraction cache, SHA256 keyed |
| `quality.json` | Structured quality report with resolution ratio, ambiguity, and per-file stats |

---

## Documentation

- [`docs/COMMAND_REFERENCE.md`](docs/COMMAND_REFERENCE.md) — CLI commands and common options.
- [`docs/MCP_TOOLS.md`](docs/MCP_TOOLS.md) — MCP tool reference for agent integrations.
- [`docs/BENCHMARKING.md`](docs/BENCHMARKING.md) — token-cost and correctness benchmark methodology.
- [`docs/COMPARISON.md`](docs/COMPARISON.md) — positioning against repo maps, generic code graphs, and enterprise indexers.
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — repository structure and internal modules.

---

## Contributing

Contributions are welcome, especially language extractors, MCP integrations, fixtures, graph analysis tools, and agent workflows. See [`CONTRIBUTING.md`](CONTRIBUTING.md).

- [Good first issues](https://github.com/lambda-alpha-labs/Graphenium/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22)
- [Worked examples](worked/)
- [Demo script](scripts/demo.sh)

## License

MIT — see [LICENSE](LICENSE).
