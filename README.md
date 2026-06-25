# Graphenium

**Persistent structural memory for AI coding agents.**

Graphenium turns your repository into a queryable graph so Claude, Cursor,
and other MCP-compatible assistants can answer structural questions in ~20 ms,
often before reading a single source file. **Especially valuable in large,
multi-module, or unfamiliar codebases** where grep-and-trace navigation breaks
down:

- What calls this function?
- What depends on this module?
- What are the architectural hubs?
- What is the shortest path between these components?
- Which files belong to the same community?

It replaces grep-and-trace navigation, not source-code understanding.

![Demo](docs/demo.gif)

---

## Why Graphenium exists

AI coding assistants are good at reading code, but they navigate repositories
like a human using `grep`: search for a symbol, open the file, follow imports,
open more files, infer relationships. Then do it all again in the next session.

In a 50-file project, grep works. In a 5,000-file monorepo with deep import
chains, it does not. That workflow has five persistent problems:

- **Repeated cold starts.** Every new session begins without a durable model
  of the repository.
- **Context window pressure.** Raw source files are large; navigation consumes
  tokens that could be used for reasoning.
- **No structural memory.** After reading files, the assistant has no persisted
  graph of how modules, functions, and concepts relate.
- **Missed cross-file relationships.** Grep surfaces text matches, not
  architectural topology, hubs, communities, or paths.
- **Scale multiplies the pain.** Every new file and dependency makes the
  grep-and-trace loop slower and more expensive. The graph stays fast regardless
  of repo size.

Graphenium runs analysis once, persists the result as a graph, and exposes it
to assistants via an [MCP](https://modelcontextprotocol.io) server. The graph
becomes the assistant's long-term memory for your repository.

**What changes:**

- **Orientation in seconds, not minutes.** `architecture_summary` gives a
  30-second map of the codebase before the assistant reads a single file.
- **Context stays focused.** Instead of filling the context window with raw
  source during navigation, the assistant reasons over compact graph output and
  reads only the files that matter.
- **Memory survives sessions.** The graph persists. A new AI session starts
  with the same structural knowledge the last one had.

---

## What it's good at, and what it's not

**Good at**

- **Navigating large codebases.** In 50+ file repos, monorepos, or unfamiliar
  projects, grep-and-trace wastes context. The graph replaces that navigation
  loop.
- AI-assisted code navigation: answer structural questions without repeatedly
  reading files.
- Impact analysis: identify connected nodes before changing a function, class,
  or module.
- Onboarding: get a high-level architectural map of an unfamiliar repo fast.
- Refactoring planning: find god nodes, low-cohesion communities, and
  surprising cross-boundary edges.
- Code review: inspect symbols, degrees, and hotspots before reviewing a
  changed file.
- Keeping the graph current with watch mode during active development.

**Not a replacement for**

- **Reading source code.** The graph captures structure and relationships, not
  implementation logic. An assistant still needs to read actual code before
  making implementation changes.
- **A full language server.** It does not perform complete type checking or
  language-specific semantic analysis at LSP depth.
- **Runtime execution.** Local graph extraction is static analysis plus optional
  LLM extraction. Telemetry overlays can import runtime data, but Graphenium
  does not execute the program.
- **A general-purpose semantic search engine.** Graphenium is primarily a
  structural repository graph. Hybrid retrieval combines keyword and graph
  topology signals, but it is not a replacement for a full code embedding
  database or semantic search engine.
- **Security scanning.** Relationship graphs are not a substitute for dedicated
  SAST tools.

---

## 20-second example

```text
Without Graphenium:
grep -> read file -> trace imports -> read more files -> infer architecture

With Graphenium:
query_graph -> get_neighbors -> shortest_path -> read only the right files
```

```sh
# Build a graph for your project, no API key needed
gm run . --no-semantic --no-viz

# Ask structural questions
gm query "what calls build_from_extraction?"

# Or connect an AI assistant via MCP and ask directly
```

---

## Quick start

### One-line install

```sh
curl -fsSL https://raw.githubusercontent.com/lambda-alpha-labs/Graphenium/main/install.sh | sh
```

### From source

Requires Rust 1.75+ ([rustup](https://rustup.rs)).

```sh
git clone https://github.com/lambda-alpha-labs/Graphenium
cd Graphenium
cargo install --path .
```

The binary is installed as `gm`.

### First run

```sh
# Build a graph, no API key needed
gm run . --no-semantic --no-viz

# Query it
gm query "authentication login session" --budget 1000

# Check your installation
gm doctor
```

### After changes

After saving or generating another graph snapshot, compare the structural
impact of your changes:

```sh
gm diff --before old-graph.json --after graphenium-out/graph.json --impact
```

---

## Symbol-level diff and blast-radius analysis

Graphenium is not just a passive repository map. It actively calculates the
structural consequences of code changes. The `gm diff` command compares two
graph snapshots, such as your `main` branch graph and your working-directory
graph, to perform change impact analysis.

```sh
# Diff your current graph against an older snapshot to show impact
gm diff --before old-graph.json --after graphenium-out/graph.json --impact
```

**What it gives the AI agent:**

- **Symbol inventory diff.** Detects which symbols were added, removed,
  renamed, or moved across communities.
- **Downstream impact.** Uses directed reverse reachability to identify callers
  or consumers affected by modified symbols.
- **Automated review order.** Generates a recommended, risk-sorted review order:
  removed symbols first, then community moves, then additions, weighted by
  downstream dependency counts.

---

## Three-tier repository model

Graphenium offers three progressive layers of analysis. Run in the mode that
matches your performance and budget needs.

| Layer | What you get | Best for | Cost / API key |
|---|---|---|---|
| **1. AST + Resolver** (Terrain) **[Stable]** | Deterministic syntax extraction, import binding, resolved calls where supported, methods, inheritance, and communities. | Syntax-accurate architectural mapping and basic navigation. | Free, local |
| **2. Semantic Pass** (Road Network) **[Stable]** | Inferred conceptual dependencies, docstring rationale, and cross-file relationships. | Behavioural tracing and richer agent reasoning. | Paid, LLM key |
| **3. Telemetry Overlay** (Live Traffic) **[Experimental]** | OTEL trace integration, P50/P95/P99 latency percentiles, and hot-path mapping. | Runtime-aware optimization and production-sensitive refactoring. | Free, local JSON |

```sh
# Tier 1: AST-only with deterministic import resolution, default local mode
gm run . --no-semantic --no-viz

# Tier 2: Add LLM-inferred relationships
gm run . --provider anthropic

# Tier 3: Telemetry overlay support is experimental.
# It ingests OpenTelemetry traces to weight the graph with runtime behaviour.
# Import commands and trace schemas may change before stabilization.
```

The `graph_stats` tool reports edge confidence and provenance breakdowns, so
the assistant knows what kind of graph it is using.

---

## MCP setup

Add Graphenium to your AI assistant's MCP config. The server uses the standard
MCP stdio transport. You can also run `gm setup <target>` to print the config
for your assistant.

**Claude Desktop** (`claude_desktop_config.json`):

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

**Cursor** (`~/.cursor/mcp.json`):

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

**CodeWhale** (`~/.codewhale/mcp.json`):

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

**After updating config, quit and relaunch the AI tool completely**. On macOS,
use Cmd+Q rather than closing the window. MCP servers are only loaded at
startup.

---

## AI Skill

The repo ships an AI Skill at `skills/graphenium/SKILL.md` that teaches
assistants which tool to reach for, how to interpret confidence levels, and how
to fall back to `gm query` when MCP is unavailable.

---

## What the assistant can ask

Once connected, the assistant has access to Graphenium's core graph tools.

**Read tools:**

| Tool | Purpose |
|---|---|
| `graph_stats` | Node/edge counts, file types, confidence breakdown |
| `architecture_summary` | Communities, focus paths, god nodes, confidence summary |
| `query_graph` | Keyword-scored BFS/DFS traversal within a token budget |
| `get_node` | Full node details by ID or label |
| `get_neighbors` | Direct neighbours with edge types and confidence |
| `get_community` | All nodes in a community cluster |
| `god_nodes` | Top N most-connected hub nodes |
| `shortest_path` | Path between any two components |
| `summarize_file` | Every symbol extracted from a source file |
| `reload_graph` | Hot-swap the graph without restarting |

**Write tools:**

| Tool | Purpose |
|---|---|
| `add_node` | Register concepts the AST cannot capture |
| `add_edge` | Record relationships confirmed through inspection |
| `remove_edge` | Correct false positives or stale relationships |

All writes persist to disk immediately.

---

## Repository memory model

Graphenium models a codebase as three things.

### Nodes

Nodes represent meaningful entities: functions, methods, classes, modules,
structs, traits, documents, images, and architectural concepts. Each node
carries metadata such as label, qualified label, file type, source file, source
location, and community ID.

### Edges

Edges are typed, directed relationships.

| Relation | Meaning | Source |
|---|---|---|
| `imports` | Module-level import/include | AST / resolver |
| `contains` | Module/class contains a symbol | AST |
| `method` | Method belongs to a class/type | AST |
| `calls` | Function calls another function | AST / resolver / semantic |
| `uses` | Cross-file usage dependency | AST / resolver / semantic |
| `inherits` | OOP inheritance | AST / semantic |
| `implements` | Interface/trait implementation | AST / semantic |
| `depends_on` | Conceptual dependency | Semantic |
| `rationale_for` | Document/comment explains code | Semantic |

### Topology

Graphenium analyzes the graph to surface communities, hub nodes, shortest
paths, surprising cross-community connections, architectural focus paths, and
change impact. The assistant can orient itself structurally before reading
implementation details.

---

## Trust and provenance model

To prevent the AI from treating guesswork as ground truth, Graphenium enforces
a multi-dimensional trust model. Every node and edge is labeled with confidence
and provenance.

### 1. Confidence tiers

- **EXTRACTED**: Tree-sitter AST, resolver output, Stack Graphs, or manually
  confirmed inspection. Treat as source-backed.
- **INFERRED**: LLM or behavioural heuristic reasoning. Treat as a
  high-probability hint.
- **AMBIGUOUS**: Heuristic uncertainty. Treat as a lead to investigate, not as
  a fact.

### 2. Provenance metadata

Every connection in the graph carries metadata tracking how it was resolved:

- `extractor`: Identifies the system that produced the edge, such as
  `tree-sitter`, `resolver`, `llm`, `manual-mcp-write`, or `runtime-otel`.
- `resolution_status`: Discloses how the target was bound, such as `resolved`,
  `unresolved`, `heuristic`, or `inferred`.

AI assistants use this metadata to weigh their conclusions:

```text
[Graphenium] Connection: require_session  calls  validate_token [resolver:resolved] -> High trust
[Graphenium] Connection: auth_service  uses  db_client [llm:inferred] -> Inspect before relying on it
```

A good assistant workflow:

1. Trust `EXTRACTED` edges as source-backed.
2. Use `INFERRED` edges as strong hints.
3. Treat `AMBIGUOUS` edges as leads to inspect.
4. Read source code before making implementation changes.

`graph_stats` reports both confidence and provenance breakdowns so you know
exactly what kind of graph you are working with.

---

## Language support

Graphenium uses [tree-sitter](https://tree-sitter.github.io/) for AST
extraction across 9 languages.

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

Semantic extraction also processes documents (`.md`, `.rst`, `.txt`), PDFs,
and images.

Build with only the languages you need:

```sh
cargo build --release --no-default-features --features lang-python,lang-rust
```

Features: `lang-python`, `lang-js`, `lang-ts`, `lang-rust`, `lang-go`,
`lang-java`, `lang-c`, `lang-cpp`, `lang-csharp`.

---

## Commands

### `gm run`

Run the full analysis pipeline on a directory.

```text
gm run [PATH] [OPTIONS]
```

| Option | Description |
|---|---|
| `PATH` | Directory to analyse, default `.` |
| `--no-semantic` | Skip LLM extraction and use local structural results |
| `--no-viz` | Skip HTML generation |
| `--provider NAME` | AI provider: `anthropic`, `openai`, `openrouter`, `deepseek`, or `openai-compatible` |
| `--model NAME` | Model to use, defaults to provider-specific default |
| `--api-key KEY` | API key, overrides provider-specific env var |
| `--api-base URL` | API base URL for `openai-compatible` provider |
| `--mode deep` | Aggressive LLM inference |
| `--update` | Incremental mode: only re-extract changed files |

```sh
gm run . --no-semantic --no-viz      # Fast local structural scan
gm run . --provider openai           # With LLM semantic extraction
gm run . --update                    # Incremental after editing files
```

### `gm query`

Query an existing graph using multiple retrieval models.

```text
gm query "<keywords>" [OPTIONS]
```

| Option | Default | Description |
|---|---|---|
| `--graph PATH` | `graphenium-out/graph.json` | Path to graph file |
| `--budget N` | `2000` | Output token budget |
| `--mode MODE` | `lexical` | Retrieval model: `lexical` for TF-cosine keyword scoring, `structural` for graph-distance proximity, or `hybrid` |
| `--dfs` | off | Use depth-first search |

```sh
gm query "authentication login" --mode lexical     # TF-cosine keyword scoring
gm query "database connection" --mode structural  # Topological neighbor clusters
gm query "parser ast walker" --mode hybrid        # Keyword plus structural proximity
```

### `gm serve`

Start an MCP server exposing the graph over stdio.

```text
gm serve [OPTIONS]
```

| Option | Default | Description |
|---|---|---|
| `--graph PATH` | `graphenium-out/graph.json` | Path to graph file |

### `gm watch`

Watch a directory and auto-rebuild the graph on changes.

```text
gm watch [PATH] [OPTIONS]
```

| Option | Default | Description |
|---|---|---|
| `PATH` | `.` | Directory to watch |
| `--debounce SECS` | `3.0` | Wait after last event before rebuild |
| `--incremental` | `true` | Patch changed files. Use `false` for full rebuilds |

```sh
gm watch . --debounce 2.0
```

### `gm doctor`

Run diagnostic checks on your Graphenium installation: binary location, graph
file health, tree-sitter languages, API keys, and graph quality.

```text
gm doctor [--graph PATH]
```

### `gm setup`

Print ready-to-paste MCP config for an AI assistant.

```text
gm setup <claude|cursor|codewhale> [--graph PATH]
```

```sh
gm setup claude
gm setup cursor
gm setup codewhale
```

### `gm diff`

Diff two graph snapshots and show symbol-level changes.

```text
gm diff [OPTIONS]
```

| Option | Default | Description |
|---|---|---|
| `--before PATH` | empty graph | Path to the old `graph.json` |
| `--after PATH` | `graphenium-out/graph.json` | Path to the new `graph.json` |
| `--impact` | off | Show downstream impact analysis and review order |

```sh
gm diff --before old-graph.json --after new-graph.json
gm diff --after new-graph.json --impact
```

---

## Output files

Graphenium writes outputs to `graphenium-out/` inside the analysed directory.

| File | Purpose |
|---|---|
| `graph.json` | Machine-readable graph for `gm serve` and `gm query` |
| `GRAPH_REPORT.md` | Markdown architecture report |
| `graph.html` | Self-contained visual graph inspection page |
| `manifest.json` | mtime index for incremental updates |
| `cache/` | Per-file semantic extraction cache, SHA256 keyed |

---

## Architecture

```text
src/
  extract/     tree-sitter syntax extraction for 9 languages
  model/       graph, node, edge, and hyperedge schemas plus graph metadata
  resolver/    cross-file import binding and target resolution
  embed/       TF-based cosine similarities and Node2Vec structural embeddings
  cluster/     Louvain community detection, split/focus clustering, and cohesion scoring
  detect/      file classification, sensitive skipping, and corpus health checks
  analyze/     PageRank, chokepoint reporting, dominators, reverse reachability, and surprise edges
  serve/       MCP server, tool handlers, and mode-aware query traversal
  semantic/    async LLM batch extraction client and response parser
  telemetry/   OTEL trace import, EMA percentile estimation, and hot-path queries (experimental)
  export/      JSON export, HTML visualisation
  cache/       mtime manifest and semantic extraction cache
  watch/       file-system watcher with incremental patching
```

---

## Limitations

- **Local graphs are structural, not fully behavioural.** AST and
  resolver-backed extraction capture imports, containment, declarations, method
  relationships, and some resolved calls where language support is available.
  They do not model full runtime behaviour, dynamic dispatch, reflection,
  generated code, or framework-specific execution paths. Richer cross-file
  `calls`, `uses`, and `implements` relationships may require the semantic
  pass, manual graph writes, or telemetry overlays.
- **Label collisions can still happen.** Common names like `new`, `mod`, and
  `run` appear across modules. Qualified labels, resolver metadata, and
  `resolution_status` help disambiguate results. `graph_stats` reports
  collision counts so you know when results may be fuzzy.
- **Large corpora need pruning.** Projects with many vendored dependencies
  should use `.grapheniumignore` to exclude `target/`, `node_modules/`,
  `.rust-toolchain/`, and similar directories.
- **Telemetry is an overlay, not a profiler.** Runtime trace ingestion can
  weight existing graph edges with latency and frequency data, but Graphenium
  does not replace a tracing backend, profiler, or APM system.

---

## Contributing

Contributions are welcome, especially language extractors, MCP integrations,
fixtures, graph analysis tools, and agent workflows. See
[CONTRIBUTING.md](CONTRIBUTING.md).

[Good first issues](https://github.com/lambda-alpha-labs/Graphenium/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22)
| [Worked examples](worked/)
| [Demo script](scripts/demo.sh)
