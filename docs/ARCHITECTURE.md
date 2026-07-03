# Architecture

Graphenium is an active coordination and verification engine, not a passive search index. This document describes the three-tier model, graph schema, extraction pipeline, trust model, and module map that power the agent lifecycle: pre-edit pathfinding, in-edit planning, and post-edit compliance verification.

## Three-Tier Repository Model

Graphenium models a repository as three layers:

**Tier 1: AST + Resolver: Terrain (Stable)**
The bottom layer is the physical code structure. Tree-sitter parses each file into an AST. Language extractors pull out symbols (functions, classes, methods, structs, traits, imports). The import resolver links `use`/`import`/`require` statements to their target files. C# assembly boundaries (`.sln`/`.csproj`) are mapped as top-level structure. Language-family classification (`lang_family` in `resolver.rs`) restricts cross-file resolution scope per-language to prevent false-positive cross-language bindings. This layer produces the base graph with nodes and edges.

**Tier 2: Semantic Pass: Road Network (Stable)**
The middle layer adds LLM-extracted relationships. An optional semantic pass using Claude/etc. identifies behavioural relationships the AST cannot capture: conceptual dependencies, delegation patterns, and architectural intent. This tier also includes academic paper detection (`looks_like_paper`) which upgrades standard Markdown/text files to `FileType::Paper` research nodes when scholarly markers are detected. These edges carry `extractor: "llm"` and confidence from the model's assessment.

**Tier 3: Telemetry Overlay: Live Traffic (Experimental)**
The top layer imports OpenTelemetry trace JSON to create a `RuntimeOverlay` with per-node call counts and latency percentiles (P50/P95/P99). This enables runtime-weighted traversal and hot-path queries. This layer is experimental and requires explicit trace data.

## Graph Model

Each graph (`schema 0.2.0`) contains:

- **Nodes**: files, modules, functions, classes, methods, structs, traits, tests, documents, build targets, CI jobs, dependencies
- **Edges**: `imports`, `contains`, `calls`, `uses`, `inherits`, `implements`, `tests`, `depends_on`, `runs_in`
- **Hyperedges**: n-ary relationships (e.g., group membership)
- **Communities**: Louvain community detection clusters nodes into architectural groups
- **Metadata**: schema version, build timestamp, project root, extraction mode, languages

## Provenance and Trust Model

Every edge carries:

| Field | Values | Meaning |
|-------|--------|---------|
| `extractor` | `tree-sitter`, `resolver`, `llm`, `csproj-parser` | Which component created this edge |
| `resolution_status` | `resolved`, `unresolved`, `heuristic` | Whether the edge target was found in the graph |
| `confidence` | `EXTRACTED`, `INFERRED`, `AMBIGUOUS` | How much to trust this relationship |

This lets agents distinguish source-backed facts (`EXTRACTED` + `resolved`) from weak leads (`AMBIGUOUS` + `unresolved`).

## Extraction Pipeline

1. **File detection**: `detect/mod.rs` walks the directory, classifies files by extension, respects `.grapheniumignore`
2. **AST parsing**: tree-sitter parses each file; language-specific extractors pull out symbols
3. **Import resolution**: `resolver.rs` builds a symbol index from all extracted nodes, resolves `imports`/`uses` edges
4. **Semantic pass** (optional): LLM analyses code for behavioural relationships
5. **Graph assembly**: `build.rs` merges all extraction results into a single graph
6. **Clustering**: Louvain community detection partitions nodes into communities
7. **Analysis**: degree distribution, PageRank hubs, chokepoints, architecture summary
8. **Export**: graph exported as JSON, quality report, HTML visualization

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
| `cache/query.rs` | Salsa-backed demand-driven incremental extraction (same engine behind `rust-analyzer`): `salsa_extract_file`, `salsa_extract_all` |
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

## C# Assembly Boundary Parsing

For .NET projects, Graphenium ingests Visual Studio solution (`.sln`) and project (`.csproj`) files via `CSharpWorkspace` (`src/extract/csharp_project.rs`). Instead of treating directories as flat folders, it maps assembly names, root namespaces, and project references into first-class graph elements using `csproj_...` nodes and `depends_on` edges. These assembly boundaries are injected before file-level extraction, establishing high-level build-configuration boundaries that the file resolver respects. This is critical for enterprise C# applications where directory layout and compiled boundaries diverge.

## Academic Paper Classification

The classification engine (`src/detect/paper.rs`) contains a `looks_like_paper` heuristic that scans the first 3,000 bytes of plain-text and Markdown files for academic signal markers: arXiv IDs, DOIs, LaTeX citation commands, proceedings references, and abstract/begin markers. Files meeting the threshold are classified as `FileType::Paper` nodes and linked into the graph alongside implementation code. This bridges the gap between scientific documentation and source code, allowing agents to map theoretical specifications directly to their implementations.

## Planning Workspace Schema

Nodes and edges carry an optional `plan_id` field that isolates virtual (planned) symbols from the physical graph. When an agent creates a planning workspace, declared symbols are tagged with the plan's identifier. The `verify_plan` engine (`src/harness.rs`) compares the planned virtual subgraph against the extracted physical graph and reports:

- `implemented_nodes`: planned symbols that have been realized in code
- `missing_nodes`: planned symbols still awaiting implementation
- `unplanned_modified_files`: modified source files not declared in the initial plan

This creates a formal design-then-verify loop where agents declare intent, write code, and programmatically verify compliance.

## Betweenness Centrality and Anomaly Detection

Beyond standard metrics like PageRank and degree centrality, Graphenium implements Brandes' O(V·E) algorithm for betweenness centrality (`src/analyze/questions.rs`), safely capped at the first 5,000 nodes. While PageRank identifies popular or heavily utilized files, betweenness centrality locates structural bridge nodes: files that act as the sole conduit between otherwise isolated communities. The engine auto-generates suggested questions prompting agents to review these architectural chokepoints.

The `surprising_connections` algorithm (`src/analyze/surprise.rs`) computes a multi-variable heuristic surprise score for graph edges, aggregating signals from:
- Confidence bonuses: high scores for AMBIGUOUS or INFERRED edges that are statistically unexpected
- Cross-FileType coupling: source files connected to non-code files (e.g., research papers)
- Cross-community boundaries: links bridging distant Louvain-detected communities
- Cross-repository / cross-directory lines: connections across top-level folder silos
- Peripheral-to-hub transitions: low-degree nodes directly coupled to giant god nodes

This provides ML-free architectural anomaly detection, helping agents locate code smells, out-of-boundary imports, and leaky abstractions without custom rules.

## Current Limitations

- **Label collisions**: 22% of node labels appear ≥2x; `get_node` shows disambiguation warnings
- **Undirected petgraph**: Relationships are directed but stored in an undirected graph; direction is preserved in edge metadata but requires manual filtering for traversals
- **Telemetry overlay**: Experimental; requires OTEL trace JSON input
- **No built-in diff viewer**: Diff output is textual/JSON; no side-by-side visualization
- **No LSP integration**: Graphenium does not provide IDE completion; it provides MCP tools for agent use
