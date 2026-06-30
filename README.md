# Graphenium

**Provenance-aware repository memory for AI coding agents.**

Before your agent edits code, give it a map it can trust.

Graphenium builds a local, persistent graph of your repository so Claude, Cursor, CodeWhale, and other MCP-compatible coding agents can plan changes, trace impact, and choose the right files to read before spending context on source code.

Most code tools help humans find files. Graphenium helps AI agents answer safer pre-edit questions:

- What depends on this module?
- What calls this function?
- What is the shortest path between these components?
- What is the safest source-backed path?
- What is the blast radius of this change?
- Which facts are extracted, inferred, or ambiguous?
- Which files should the agent read first?
- Does this repository still meet our trust-quality bar in CI?

Graphenium is not a replacement for reading source code. It is the map agents use before they read code.

---

## Why this matters now

AI coding agents are moving from autocomplete to delegated software work. In small projects, an agent can read its way into context. In large repositories, that approach becomes expensive, repetitive, and risky.

Every cold start costs tokens. Every unnecessary file read costs tokens. Every guessed dependency path creates review risk.

Graphenium turns repository navigation into compact graph queries. The goal is simple: fewer tokens spent rediscovering architecture, more context left for reasoning, implementation, and review.

Use Graphenium when:

- your repository is too large for agents to understand by reading files casually;
- repeated agent sessions rediscover the same architecture;
- reviewers need dependency paths and blast-radius analysis before accepting agent changes;
- you want agents to distinguish source-backed facts from weak leads;
- CI should fail when the graph becomes too ambiguous or unresolved.

---

## The core workflow

```text
Without Graphenium:
agent -> grep -> read file -> trace imports -> read more files -> infer architecture -> repeat next session

With Graphenium:
agent -> query graph -> inspect trust profile -> read only the files that matter -> plan change -> run gate
```

A typical pre-edit flow:

1. Build or update the repository graph.
2. Ask Graphenium what depends on the target symbol, module, or feature.
3. Inspect the trust profile: `EXTRACTED`, `INFERRED`, and `AMBIGUOUS` facts.
4. Ask for the next files to read.
5. Read those source files directly.
6. Make the change.
7. Run diff, blast-radius, and trust gates before review.

Example agent prompt:

```text
Before editing auth.validate_token, use Graphenium to identify downstream impact,
source-backed dependency paths, ambiguous relationships, and the first files I
should read. Do not edit until you have a change plan.
```

Example Graphenium-style answer:

```text
Impact summary: 17 downstream symbols, 4 high-risk paths, 3 ambiguous edges.
Safest source-backed path: auth.validate_token -> auth.require_session -> routes.account.
Read first: src/auth/session.py, src/routes/account.py, tests/auth/test_session.py.
Trust profile: 28 EXTRACTED, 5 INFERRED, 3 AMBIGUOUS.
```

---

## What Graphenium builds

Graphenium models a repository as a durable graph:

- **Nodes** are files, modules, functions, classes, methods, structs, traits, tests, documents, build targets, CI jobs, dependencies, and architectural concepts.
- **Edges** are relationships such as `imports`, `contains`, `calls`, `uses`, `inherits`, `implements`, `tests`, `depends_on`, `runs_in`, and `rationale_for`.
- **Evidence** records where a fact came from and how confidently it should be used.
- **Topology** surfaces communities, hubs, dependency paths, surprising cross-community connections, and blast radius.

Graphenium can run locally with structural extraction only, or it can add optional semantic extraction and telemetry overlays when you need more context.

---

## What makes Graphenium different

Graphenium is deliberately narrower than a general static analyzer, semantic search engine, or enterprise code intelligence platform. Its job is to provide a trust-aware memory layer for coding agents.

| Need | Grep / search | Vector search | Repo map | Generic MCP code graph | Graphenium |
|---|---:|---:|---:|---:|---:|
| Exact text lookup | Yes | Sometimes | Sometimes | Sometimes | Yes |
| Repository topology | No | Weak | Yes | Yes | Yes |
| Persistent agent memory | No | Sometimes | Tool-dependent | Usually | Yes |
| Token-budgeted traversal | No | Sometimes | Yes | Sometimes | Yes |
| Confidence per relationship | No | No | No | Rare | Core model |
| Extracted vs inferred separation | No | No | No | Rare | Core model |
| Source-backed safest path | No | No | No | Sometimes | Built in |
| Blast-radius analysis | No | No | No | Sometimes | Built in |
| CI trust-quality gates | No | No | No | No | Built in |

Positioning:

> Graphenium is the context-budget, trust, and change-safety layer for AI coding agents working in large repositories.

---

## Trust-aware graph facts

Every important node and edge carries confidence and provenance metadata.

| Confidence | Meaning | How an agent should use it |
|---|---|---|
| `EXTRACTED` | Produced by tree-sitter, resolver, telemetry import, or manually confirmed inspection | Treat as source-backed, then verify implementation details in source |
| `INFERRED` | Produced by LLM reasoning or behavioral heuristics | Treat as a high-probability lead |
| `AMBIGUOUS` | Produced from uncertain, conflicting, or unresolved evidence | Investigate before relying on it |

Provenance metadata records how a relationship was produced, such as `tree-sitter`, `resolver`, `llm`, `telemetry`, or `manual-mcp-write`. Resolution metadata records whether a reference was resolved, unresolved, heuristic, inferred, or manually confirmed.

Agent-facing output should make this visible:

```text
[Graphenium] require_session calls validate_token [resolver:resolved] -> high trust
[Graphenium] auth_service uses db_client [llm:inferred] -> inspect before relying
[Graphenium] AccountController calls login [resolver:ambiguous] -> verify target symbol
```

---

## Quick start

### Install

**macOS / Linux:**
```sh
curl -fsSL https://raw.githubusercontent.com/lambda-alpha-labs/Graphenium/main/install.sh | sh
```

**Windows (PowerShell):**
```powershell
powershell -ExecutionPolicy Bypass -File install.ps1
```

### Or build from source

Requires Rust 1.75+.

```sh
git clone https://github.com/lambda-alpha-labs/Graphenium
cd Graphenium
cargo install --path .
```

The binary is installed as `gm`.

### Build and query your first graph

```sh
# 1. Initialize workspace with safe ignore defaults
gm init

# 2. Build a local structural graph, no API key needed
gm run . --no-semantic --no-viz

# 3. Query with a token budget and confidence-aware traversal
gm query "authentication login session" --safe --budget 1000

# 4. Inspect graph health and resolution quality
gm doctor --resolution

# 5. Enforce a CI-ready trust bar
gm check --min-resolution 80 --max-ambiguous 10
```

The fastest adoption path is:

```sh
gm run . --no-semantic
gm setup claude
gm query "symbol or feature" --safe
```

See [`docs/GETTING_STARTED.md`](docs/GETTING_STARTED.md) for a guided first workflow.

---

## MCP setup

Graphenium exposes your repository graph through an MCP stdio server.

```sh
gm setup claude              # Shows available Claude targets
gm setup claude-desktop      # Claude Desktop app config
gm setup claude-code         # Claude Code CLI (recommended for CLI users)
gm setup cursor
gm setup codewhale
```

**Claude Code** uses the `claude mcp add` command:
```bash
claude mcp add graphenium --scope user -- gm serve
```

**Claude Desktop** uses `claude_desktop_config.json` (see `gm setup claude-desktop`).
**Cursor / CodeWhale** use JSON / TOML config (see `gm setup cursor` / `gm setup codewhale`).

The server starts gracefully even before a graph exists — run `gm run .` after
setup and the server will automatically load the graph when it appears.

---

## Common agent workflows

### Pre-edit planning

```text
Use Graphenium before making changes. Find the blast radius for SYMBOL_NAME,
show the safest source-backed paths, list ambiguous relationships, and tell me
which files to read first.
```

### Architecture orientation

```text
Use Graphenium to summarize the repository architecture, major communities,
hub nodes, and the highest-risk chokepoints.
```

### Review planning

```text
Use Graphenium to diff the current graph against the previous snapshot. Produce
a risk-sorted review plan with removed symbols, changed dependencies, community
moves, and high-degree consumers.
```

More examples are in `docs/AGENT_WORKFLOWS.md`.

---

## Core capabilities

- Persistent repository graph that can be reused across sessions.
- Compact agent context with token-budgeted traversal and leaf-symbol omission.
- Confidence-aware retrieval through safe traversal, safest paths, shortest paths, and neighbor queries.
- Symbol-level diffing and blast-radius analysis for review planning.
- CI trust gates for resolution quality, ambiguity, and stale evidence.
- MCP-native tools for architecture summaries, dependency paths, verification plans, and graph updates.
- Optional semantic extraction for concepts the AST cannot capture.
- Optional telemetry overlays for runtime hot paths and regression comparison.

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

## Benchmarking status

Graphenium should be evaluated on **tokens to correct change plan**, not token reduction alone.

Self-benchmarks on Graphenium's own codebase (1,061 nodes, 2,104 edges, 22 communities):

| Task | Graphenium workflow | Output chars | Tokens (~4 c/t) | Response time |
|---|---|---|---:|---:|
| Impact of `replace_file_extraction` | `query_transitive` + `blast_radius` | 8,674 | ~2,170 | 27ms |
| Community overview | `query_graph` on `GrapheniumCluster` | 6,690 | ~1,670 | 31ms |
| Module architecture | `query_graph` on `GrapheniumGraph` | 8,395 | ~2,100 | 24ms |
| Cross-module search | `query_graph "authentication flow"` | 8,409 | ~2,100 | 31ms |

Compare to grep + source reading: typically 30,000-50,000 characters (~8,000-12,000 tokens) for the same tasks — a **4-6x token reduction**.

See [`docs/BENCHMARKING.md`](docs/BENCHMARKING.md) for the full benchmark protocol, reproducibility steps, and limits described there.

---

## Documentation

- [`docs/GETTING_STARTED.md`](docs/GETTING_STARTED.md) - guided first workflow.
- [`docs/AGENT_WORKFLOWS.md`](docs/AGENT_WORKFLOWS.md) - prompts and agent usage patterns.
- [`docs/COMMAND_REFERENCE.md`](docs/COMMAND_REFERENCE.md) - CLI commands and common options.
- [`docs/MCP_TOOLS.md`](docs/MCP_TOOLS.md) - MCP tool reference for agent integrations.
- [`docs/BENCHMARKING.md`](docs/BENCHMARKING.md) - token-cost and correctness benchmark methodology.
- [`docs/COMPARISON.md`](docs/COMPARISON.md) - positioning against search, repo maps, generic graphs, and enterprise indexers.
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) - graph model, extraction pipeline, internals, and limitations.
- [`docs/DEMO_SCRIPT.md`](docs/DEMO_SCRIPT.md) - suggested demo script and terminal storyboard.

---

## Contributing

Contributions are welcome, especially language extractors, MCP integrations, fixtures, graph analysis tools, and agent workflows.

Good contribution areas:

- new language extractors;
- higher-precision resolvers;
- benchmark fixtures and scoring rubrics;
- MCP client recipes;
- graph analysis algorithms;
- CI and policy gate examples;
- worked examples for common agent workflows.

## License

MIT - see `LICENSE`.
