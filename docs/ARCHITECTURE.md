# Graphenium Architecture

> **Provenance-aware structural memory for AI coding agents.**

This document describes the architecture, data model, extraction pipeline, trust system, query modes, module map, and current limitations of Graphenium.

---

## Table of Contents

1. [Three-Tier Model](#three-tier-model)
2. [Graph Model](#graph-model)
3. [Extraction Pipeline](#extraction-pipeline)
4. [Trust Model](#trust-model)
5. [Query Modes](#query-modes)
6. [Module Map](#module-map)
7. [Output Files](#output-files)
8. [Current Limitations](#current-limitations)

---

## Three-Tier Model

Graphenium organises code understanding into three tiers of increasing semantic depth.

### Tier 1: AST + Resolver (Terrain) — **Stable**

The foundation. Tree-sitter grammars parse source files into abstract syntax trees, extracting declarations, imports, function calls, method definitions, and class hierarchies. A cross-file import resolver then binds import statements to their target symbols across the repository.

- **Deterministic**: same code always produces the same graph.
- **Language-specific**: custom extractors for Rust and Go; a generic tree-sitter walker for all other supported languages.
- **Fast**: parallelised with Rayon; incremental via SHA256-based caching.

### Tier 2: Semantic Pass (Road Network) — **Stable**

An optional LLM-powered extraction pass that runs after AST extraction. Uncached files are batched and sent to an AI provider (Anthropic Claude, by default) which produces additional nodes, edges, and hyperedges describing behavioural relationships that tree-sitter cannot capture.

- **Behavioural**: captures `calls`, `uses`, `implements`, and group relationships.
- **Cacheable**: per-file SHA256-keyed cache avoids redundant API calls.
- **Configurable**: batch size, concurrency, and model selection via `SemanticOptions`.

### Tier 3: Telemetry Overlay (Live Traffic) — **Experimental**

An OpenTelemetry trace overlay that imports runtime span data and annotates the static graph with latency, frequency, and hot-path information.

- **Runtime-aware**: weights existing graph edges with observed latency/frequency.
- **Delta-based**: EMA percentile estimation and regression comparison between baseline and current traces.
- **Not a profiler**: the overlay enhances the structural graph; it does not replace a tracing backend or APM system.

---

## Graph Model

### Core Data Structures

| Concept | Type | Description |
|---|---|---|
| **Node** | `Node` | A single entity: function, class, module, file, document, or rationale. Has a stable ID, human-readable label, optional qualified label, file type, source location, community assignment, and provenance fields (`extractor`, `resolution_status`). |
| **Edge** | `Edge` | A directed relationship between two nodes, stored in an undirected graph. Direction is preserved logically via `src_original` / `tgt_original`. Carries a relation type (`calls`, `imports`, `uses`, `contains`, etc.), confidence, confidence score, provenance (`extractor`), and `resolution_status`. |
| **HyperEdge** | `HyperEdge` | An N-ary relationship involving three or more nodes. Captures group relationships like "all implement interface X" or "all participate in authentication flow". Stored as a side-car vector (petgraph does not support hyperedges natively). |
| **Graph** | `GrapheniumGraph` | Wraps a `petgraph::Graph` (undirected) with O(1) node lookup by string ID, a side-car `Vec<HyperEdge>`, and `GraphMetadata`. |

### Underlying Storage

The graph uses **petgraph** with an undirected `Graph<Node, Edge, Undirected>`. Edge direction is recovered at query time through `Edge::src_original` and `Edge::tgt_original`.

### Graph Metadata

Each graph carries metadata in `GraphMetadata`:

- `schema_version` — version of the graph.json format
- `graphenium_version` — version of Graphenium that produced it
- `created_at` — ISO 8601 build timestamp
- `project_root` — absolute path to the analysed project
- `extraction_modes` — which modes were used (`"ast"`, `"semantic"`, etc.)
- `languages` — languages detected in the source tree
- `ast_only` — whether the graph is AST-only (no semantic pass)

### Communities

After the clustering phase, every node is assigned a `community` ID (0-indexed, community 0 is the largest). Communities are detected using the Louvain algorithm with configurable resolution, split/focus clustering for oversized communities, and cohesion scoring.

---

## Extraction Pipeline

The end-to-end pipeline proceeds through these stages:

```text
Source Files
    │
    ▼
┌──────────────────────┐
│ 1. File Detection     │  Walk directory tree, classify files by type
│    (detect/)          │  (Code, Document, Paper, Image), respect
│                       │  .gitignore + .grapheniumignore, skip sensitive
└──────────────────────┘
    │
    ▼
┌──────────────────────┐
│ 2. Tree-sitter AST    │  Parse code files with tree-sitter grammars.
│    (extract/)         │  Extract declarations, imports, calls, methods,
│                       │  classes. Custom extractors for Rust + Go.
└──────────────────────┘
    │
    ▼
┌──────────────────────┐
│ 3. Import Resolution  │  Cross-file import binding: build export index
│    (resolver/)        │  from all extracted symbols, resolve import
│                       │  edges, mark unresolved references.
└──────────────────────┘
    │
    ▼
┌──────────────────────┐
│ 4. Validation         │  Strip malformed nodes/edges (empty IDs, out-of-
│    (validate/)        │  range confidence scores, etc.). Return a
│                       │  ValidationReport.
└──────────────────────┘
    │
    ▼
┌──────────────────────┐
│ 5. Semantic LLM Pass  │  (Optional) Batch uncached files, send to AI
│    (semantic/)        │  provider for behavioural relationship extraction.
│                       │  Results are cached per-file by SHA256.
└──────────────────────┘
    │
    ▼
┌──────────────────────┐
│ 6. Graph Assembly     │  Fold ExtractionResult values into GrapheniumGraph.
│    (build/)           │  Last-write-wins on nodes (semantic overrides AST);
│                       │  dangling edges (unknown endpoints) dropped.
└──────────────────────┘
    │
    ▼
┌──────────────────────┐
│ 7. Clustering         │  Louvain community detection. Assign community IDs,
│    (cluster/)         │  compute cohesion stats, split oversized clusters.
└──────────────────────┘
    │
    ▼
┌──────────────────────┐
│ 8. Analysis           │  PageRank, god nodes, surprising connections,
│    (analyze/)         │  chokepoints, dominators, reverse reachability,
│                       │  change impact, suggested questions.
└──────────────────────┘
    │
    ▼
┌──────────────────────┐
│ 9. Export             │  Write graph.json, graph.html (self-contained
│    (export/)          │  visualisation), GRAPH_REPORT.md, quality.json.
└──────────────────────┘
    │
    ▼
┌──────────────────────┐
│ 10. Serve / Watch     │  Launch MCP server (stdio JSON-RPC) or
│    (serve/, watch/)   │  file watcher with live blast-radius display.
└──────────────────────┘
```

### Telemetry Overlay (optional, experimental)

After the pipeline completes, runtime OpenTelemetry traces can be imported and overlaid on the graph, enriching edges with latency/frequency data and enabling hot-path queries.

---

## Trust Model

Graphenium is designed around **provable trust** — every relationship in the graph carries metadata about how it was discovered and how reliable it is.

### Confidence Levels

| Level | Score | Meaning |
|---|---|---|
| **Extracted** | 1.0 | Explicitly present in source code (import, call, declaration, citation). Deterministically produced by tree-sitter or the resolver. |
| **Inferred** | 0.5 | A reasonable inference with documented reasoning. Typically produced by the semantic LLM pass or heuristic pattern matching. |
| **Ambiguous** | 0.2 | Uncertain — flagged for manual review. May be a naming collision, weak heuristic, or low-confidence LLM output. |

### Provenance

Every node and edge records:

- **`extractor`** — which system produced it: `"tree-sitter"`, `"resolver"`, `"llm-anthropic"`, `"tree-sitter-stack-graphs"`, etc.
- **`resolution_status`** — import/call binding quality: `"resolved"`, `"unresolved"`, `"ambiguous"`, `"heuristic"`, `"inferred"`.

### Evidence Spans (v3)

The trust system adds evidence-anchored graph facts:

- **`EvidenceSpan`**: ties every node/edge to a specific source location (file, byte offsets, line range, SHA256 of span text and full file).
- **Evidence state**: `Valid` / `Stale` / `Unverified` / `Missing`.
- **Stale detection**: re-hashing the file and comparing against the stored `file_hash` detects out-of-date evidence.

### Quality Gates

The `gm check` command enforces trust policies in CI:

- `MinResolution` — minimum import resolution percentage (default 80%)
- `MaxAmbiguous` — maximum number of ambiguous edges (default 10)
- `MaxStale` — maximum number of stale evidence spans
- `MinCoherence` — minimum community coherence
- `MaxUnresolved` — maximum unresolved references
- `MinCallResolution` — minimum call-edge resolution (default 70%)

Policies are definable in TOML files and evaluated against the graph and its `ResolutionReport`.

---

## Query Modes

### Lexical (TF-Cosine)

Keyword-based scoring using term-frequency vectors built from node labels and qualified labels. Terms are lowercased, split on non-alphanumeric characters, TF-normalised, and stop words filtered. Cosine similarity ranks results.

### Structural (Graph-Distance)

Topological proximity scoring: nodes closer to the matched seed(s) in graph distance rank higher. Uses the directed projection for directional awareness.

### Hybrid

Combined lexical + structural scoring. Matches are first filtered by keyword similarity, then re-ranked by graph distance, producing results that are both textually relevant and topologically nearby.

All three modes are implemented in the `ranking` module and exposed through the MCP server's `query_graph` tool.

---

## Module Map

### `src/` Directory Tree

```
src/
├── main.rs                  — CLI entry point (`gm` binary, clap-based commands)
├── lib.rs                   — Library entry point, module re-exports, feature flags
├── analyze/
│   ├── mod.rs               — Aggregate analysis entry point (god nodes + surprise + questions)
│   ├── diff.rs              — Graph diffing: added/removed nodes and edges between snapshots
│   ├── god.rs               — God node (hub) detection: most-connected symbols
│   ├── impact.rs            — Symbol-level diff and downstream impact (blast radius, community moves)
│   ├── questions.rs         — Suggested architectural questions from community structure
│   ├── rank.rs              — Directed projection, PageRank, reverse reachability, dominators, chokepoints
│   ├── surprise.rs          — Surprising/ unexpected edge detection
│   └── verifier.rs          — Graph diff-based verification plan builder
├── build.rs                 — Graph construction from ExtractionResult (nodes → edges → hyperedges)
├── cache/
│   ├── mod.rs               — File-based extraction cache (SHA256-keyed, atomic writes)
│   ├── manifest.rs          — mtime-based manifest for incremental change detection
│   └── semantic_cache.rs    — Semantic extraction result cache
├── cluster/
│   ├── mod.rs               — Louvain community detection entry point
│   ├── cohesion.rs          — Community cohesion scoring
│   ├── drift.rs             — Architecture drift detection (declared vs actual communities)
│   ├── focus.rs             — Focus clustering for sub-community analysis
│   ├── louvain.rs           — Louvain algorithm implementation
│   └── split.rs             — Oversized community splitting
├── detect/
│   ├── mod.rs               — File detection: walk, classify, skip hidden/sensitive
│   ├── classify.rs          — File type classification by extension and content
│   ├── corpus.rs            — Corpus health checks and warnings
│   ├── paper.rs             — Academic paper detection heuristic
│   └── sensitive.rs         — Sensitive file/content skipping (credentials, binaries)
├── doctor.rs                — `gm doctor` diagnostic command (binary health, graph health, API keys)
├── embed.rs                 — Embedding-based retrieval: TF vectors (text) + Node2Vec (structural)
├── error.rs                 — Central error type (`GrapheniumError`)
├── export/
│   ├── mod.rs               — Export orchestration: write graph.json + graph.html
│   ├── html.rs              — Self-contained HTML visualisation renderer
│   ├── html_template.rs     — HTML template strings
│   └── json.rs              — JSON serialisation/deserialisation + quality report
├── extract/
│   ├── mod.rs               — Extraction orchestrator: dispatch, parallel extraction, Python post-processing
│   ├── ci.rs                — CI pipeline extraction (Cargo.toml, package.json, GitHub Actions)
│   ├── config.rs            — Repository config extraction (graphenium.toml)
│   ├── cross_file.rs        — Python cross-file import resolution (post-processing)
│   ├── go.rs                — Go language tree-sitter extractor
│   ├── import_handlers.rs   — Generic import statement handlers
│   ├── rust_lang.rs         — Rust language tree-sitter extractor
│   └── walker.rs            — Generic tree-sitter AST walker for all other languages
├── graph_integrity.rs       — Graph invariant checker for debug builds and `gm doctor`
├── harness.rs               — CI trust harness: `gm check` quality gate logic
├── model/
│   ├── mod.rs               — Model re-exports
│   ├── node.rs              — Node struct with FileType, community, provenance fields
│   ├── edge.rs              — Edge struct with Confidence enum, direction metadata
│   ├── graph.rs             — GrapheniumGraph: petgraph wrapper with ID index, hyperedges, metadata
│   ├── hyperedge.rs         — HyperEdge struct for N-ary relationships
│   ├── id.rs                — Node ID generation, normalisation, label normalisation
│   └── extraction.rs        — ExtractionResult: the extraction output data structure
├── policy.rs                — Policy-based quality gates (MinResolution, MaxAmbiguous, etc.)
├── ranking.rs               — Query modes: Lexical, Structural, Hybrid; ranked node scoring
├── report.rs                — GRAPH_REPORT.md Markdown report generation
├── resolver.rs              — Cross-file import resolution post-processor
├── semantic/
│   ├── mod.rs               — Semantic extraction orchestrator (batching, caching, async LLM dispatch)
│   ├── client.rs            — LLM API client (Claude/Anthropic)
│   ├── parse.rs             — LLM response parsing and validation
│   ├── prompt.rs            — System prompt templates for semantic extraction
│   └── provider.rs          — AI provider configuration (Anthropic, etc.)
├── serve/
│   ├── mod.rs               — MCP server (stdio JSON-RPC transport, graph hot-reload on file change)
│   ├── handlers.rs          — MCP tool handlers (query_graph, get_node, get_neighbors, etc.)
│   └── traversal.rs         — Graph traversal logic for MCP queries (BFS, DFS, keyword matching)
├── telemetry.rs             — Runtime telemetry overlay: OTEL trace import, EMA percentiles, regression compare
├── trust.rs                 — Evidence and claim models: EvidenceSpan, stale detection, ResolutionReport
├── validate.rs              — ExtractionResult validation: strip malformed nodes/edges, ValidationReport
└── watch.rs                 — File-watch mode: debounced FS watcher, incremental rebuild, live blast radius
```

### Module Responsibilities

| Module | Responsibility |
|---|---|
| `main.rs` | CLI binary with `gm` subcommands (`run`, `serve`, `watch`, `check`, `doctor`, `query`, `snapshot`, `gate`, etc.) |
| `lib.rs` | Library entry point, feature flags (`harness`, `lang-python`, `lang-rust`, etc.) |
| `analyze/` | Post-clustering analysis: god nodes, surprising edges, change impact, PageRank, diff, verification plans |
| `build.rs` | Graph assembly from extraction results, last-write-wins merge |
| `cache/` | SHA256-keyed file cache for extraction results, mtime manifest |
| `cluster/` | Louvain community detection, splitting, cohesion, drift detection |
| `detect/` | File system walking, classification, corpus health, sensitive file filtering |
| `doctor.rs` | Health diagnostics for installation and graph integrity |
| `embed.rs` | TF-based text embeddings and Node2Vec structural embeddings |
| `error.rs` | Unified error type |
| `export/` | JSON export, self-contained HTML visualisation, quality report |
| `extract/` | Tree-sitter AST extraction for Rust, Go, and generic languages; CI config extraction |
| `graph_integrity.rs` | Invariant checking for debug builds |
| `harness.rs` | CI trust gate logic for `gm check` |
| `model/` | Core data types: Node, Edge, HyperEdge, GrapheniumGraph, Confidence, FileType |
| `policy.rs` | Policy definition and evaluation for quality gates |
| `ranking.rs` | Query mode scoring (lexical, structural, hybrid) |
| `report.rs` | Markdown architecture report generation |
| `resolver.rs` | Cross-file import resolution and status marking |
| `semantic/` | LLM-powered behavioral extraction with caching and batching |
| `serve/` | MCP server exposing graph tools over stdio JSON-RPC |
| `telemetry.rs` | Experimental OpenTelemetry trace overlay for runtime awareness |
| `trust.rs` | Evidence spans, stale detection, resolution reporting |
| `validate.rs` | Extraction result cleaning and validation |
| `watch.rs` | Debounced file-system watcher with incremental rebuild |

---

## Output Files

All outputs are written to `graphenium-out/` inside the analysed directory.

| File | Purpose |
|---|---|
| `graph.json` | Machine-readable graph (JSON) for `gm serve` and `gm query` |
| `GRAPH_REPORT.md` | Markdown architecture report with communities, god nodes, surprises |
| `graph.html` | Self-contained visual graph inspection page |
| `manifest.json` | mtime index for incremental updates |
| `cache/` | Per-file semantic extraction cache, SHA256 keyed |
| `quality.json` | Structured quality report: resolution ratio, ambiguous edges, per-file stats |

---

## Current Limitations

1. **Label collisions can still happen.** Common names like `new`, `mod`, and `run` appear across modules. Qualified labels, resolver metadata, and `resolution_status` help disambiguate. `graph_stats` reports collision counts so you know when results may be fuzzy.

2. **The underlying graph is undirected.** Directionality is preserved logically in each `Edge` via `src_original` / `tgt_original`, but petgraph's undirected `Graph` type means some directed-graph algorithms (e.g., dominator trees) require a separate directed projection step.

3. **Telemetry is an overlay, not a profiler.** Runtime trace ingestion can weight existing graph edges with latency and frequency data, but Graphenium does not replace a tracing backend, profiler, or APM system. The telemetry overlay is experimental.

4. **No LSP or decompilation support.** Graphenium works on source files at rest. It does not integrate with language servers for live symbol resolution or handle decompiled/obfuscated code.

5. **No built-in diff viewer.** The `gm gate` and `gm snapshot` commands produce structured diff data, but there is no built-in side-by-side or visual diff viewer for graph snapshots.

6. **Large corpora need pruning.** Projects with many vendored dependencies should use `.grapheniumignore` to exclude `target/`, `node_modules/`, `.rust-toolchain/`, and similar directories.

7. **AST-only extraction is structural, not fully behavioral.** Tree-sitter and resolver-backed extraction capture imports, containment, declarations, method relationships, and some resolved calls where language support is available. Richer cross-file `calls`, `uses`, and `implements` relationships require the semantic pass, manual graph writes, or telemetry overlays.

8. **Quality gates are only as good as the graph and policy.** `gm check` helps enforce trust thresholds, but teams should tune policies to their repository, language mix, and risk tolerance.
