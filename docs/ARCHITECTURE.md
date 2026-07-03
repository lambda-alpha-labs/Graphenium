# Architecture

This document describes Graphenium's architecture at the three-tier model, graph model, extraction pipeline, trust model, and module level.

## Three-Tier Repository Model

Graphenium models a repository as three layers:

**Tier 1: AST + Resolver — Terrain (Stable)**
The bottom layer is the physical code structure. Tree-sitter parses each file into an AST. Language extractors pull out symbols (functions, classes, methods, structs, traits, imports). The import resolver links `use`/`import`/`require` statements to their target files. This layer produces the base graph with nodes and edges.

**Tier 2: Semantic Pass — Road Network (Stable)**
The middle layer adds LLM-extracted relationships. An optional semantic pass using Claude/etc. identifies behavioural relationships the AST cannot capture: conceptual dependencies, delegation patterns, and architectural intent. These edges carry `extractor: "llm"` and confidence from the model's assessment.

**Tier 3: Telemetry Overlay — Live Traffic (Experimental)**
The top layer imports OpenTelemetry trace JSON to create a `RuntimeOverlay` with per-node call counts and latency percentiles (P50/P95/P99). This enables runtime-weighted traversal and hot-path queries. This layer is experimental and requires explicit trace data.

## Graph Model

Each graph (`schema 0.2.0`) contains:

- **Nodes** — files, modules, functions, classes, methods, structs, traits, tests, documents, build targets, CI jobs, dependencies
- **Edges** — `imports`, `contains`, `calls`, `uses`, `inherits`, `implements`, `tests`, `depends_on`, `runs_in`
- **Hyperedges** — n-ary relationships (e.g., group membership)
- **Communities** — Louvain community detection clusters nodes into architectural groups
- **Metadata** — schema version, build timestamp, project root, extraction mode, languages

## Provenance and Trust Model

Every edge carries:

| Field | Values | Meaning |
|-------|--------|---------|
| `extractor` | `tree-sitter`, `resolver`, `llm`, `csproj-parser` | Which component created this edge |
| `resolution_status` | `resolved`, `unresolved`, `heuristic` | Whether the edge target was found in the graph |
| `confidence` | `EXTRACTED`, `INFERRED`, `AMBIGUOUS` | How much to trust this relationship |

This lets agents distinguish source-backed facts (`EXTRACTED` + `resolved`) from weak leads (`AMBIGUOUS` + `unresolved`).

## Extraction Pipeline

1. **File detection** — `detect/mod.rs` walks the directory, classifies files by extension, respects `.grapheniumignore`
2. **AST parsing** — tree-sitter parses each file; language-specific extractors pull out symbols
3. **Import resolution** — `resolver.rs` builds a symbol index from all extracted nodes, resolves `imports`/`uses` edges
4. **Semantic pass** (optional) — LLM analyses code for behavioural relationships
5. **Graph assembly** — `build.rs` merges all extraction results into a single graph
6. **Clustering** — Louvain community detection partitions nodes into communities
7. **Analysis** — degree distribution, PageRank hubs, chokepoints, architecture summary
8. **Export** — graph exported as JSON, quality report, HTML visualization

## Module Map

| Module | Description |
|--------|-------------|
| `analyze/` | Architecture analysis: diff, impact, rank, god nodes, verifier |
| `build.rs` | Graph assembly pipeline |
| `cache/` | Manifest tracking, semantic caching |
| `cluster/` | Louvain community detection, drift analysis |
| `detect/` | File detection, classification, `.grapheniumignore` |
| `embed.rs` | TF-based text embeddings, Node2Vec structural embeddings |
| `export/` | JSON and HTML export |
| `extract/` | Tree-sitter AST extraction per language |
| `harness.rs` | Trust check harness for CI |
| `model/` | Graph, node, edge, hyperedge, extraction result types |
| `policy.rs` | Trust quality policies |
| `ranking.rs` | Query ranking: lexical, structural, hybrid |
| `resolver.rs` | Cross-file import resolution, build_file_index, resolve_cross_file_symbol |
| `cache/query.rs` | Salsa-backed demand-driven incremental extraction: `salsa_extract_file`, `salsa_extract_all` |
| `analyze/query.rs` | Datalog declarative query engine: `run_datalog_query`, `tokenize`, `solve` with semi-naive fixpoint |
| `semantic/` | LLM-based semantic extraction |
| `serve/` | MCP server with 22 tools |
| `telemetry.rs` | Runtime telemetry overlay |
| `trust.rs` | Evidence span, claims, stale detection |
| `watch.rs` | File watcher for incremental rebuilds |
| `doctor.rs` | Diagnostic checks |
| `error.rs` | Error types |
| `main.rs` | CLI entry point with clap |

## Query Modes

| Mode | Algorithm | Best For |
|------|-----------|----------|
| Lexical | TF-cosine keyword matching | Finding nodes by name/description |
| Structural | Graph-distance from keyword seeds | Finding topologically related code |
| Hybrid | Weighted (0.6 lexical + 0.4 structural) | General-purpose discovery |
| Datalog | First-order logic fixpoint | Declarative reachability and constraint queries |

## Current Limitations

- **Label collisions**: 22% of node labels appear ≥2x; `get_node` shows disambiguation warnings
- **Undirected petgraph**: Relationships are directed but stored in an undirected graph; direction is preserved in edge metadata but requires manual filtering for traversals
- **Telemetry overlay**: Experimental; requires OTEL trace JSON input
- **No built-in diff viewer**: Diff output is textual/JSON; no side-by-side visualization
- **No LSP integration**: Graphenium does not provide IDE completion; it provides MCP tools for agent use
