# Changelog

All notable changes to Graphenium are documented in this file.

## v0.4.1 (2026-06-27) — Patch: incremental rebuild fixes

### Fixed
- **Critical data corruption in incremental rebuilds:** `replace_file_extraction` now calls `rebuild_id_index()` after batch-removing stale nodes. Without this, petgraph's swap-remove shifts node indices, silently corrupting lookups in the `id_index`.
- **Disconnected manifest invalidation:** `try_incremental` now loads the manifest and uses `invalidation_set()` to include downstream importers in the re-extraction set during watch-mode rebuilds.

## v0.4.0 (2026-06-27) — Scale Foundation + Trust UX

### Added
- **Graph Invariant Checker**: `graph_integrity.rs` — validates `id_index` consistency, catches dangling edges and stale indices after incremental updates
- **`rebuild_id_index()`**: Fixes petgraph `NodeIndex` invalidation after node deletion — prevents stale index corruption in incremental builds
- **`quality.json`**: Structured quality report generated alongside `graph.json` — includes resolution ratio, ambiguous edges, per-file unresolved refs, recommended commands
- **Benchmark Harness**: `scripts/bench.sh` — measures cold index, graph load, and query latency with JSON output
- **Manifest Dependency Tracking**: `FileMeta` stores per-file `imports` and `imported_by` for cache invalidation
- **`invalidation_set()`**: Computes downstream files needing re-extraction when dependencies change
- **MCP Pre-serialization Truncation**: Community member listing capped at 200 with overflow notes
- **TS/JS Extractor Upgrade**: ES module import/export handler (named, default, namespace, side-effect) + CommonJS `require()` support
- **CI Lifecycle**: Build and test targets link to CI jobs via `runs_in` edges

### Fixed
- Petgraph `id_index` invalidation: `rebuild_id_index()` called after batched node deletion
- MCP `summarize_community` now caps output at 200 members to prevent 293KB artifacts
- Manifest now persists `file_meta` alongside legacy `entries` for backward compatibility

### Performance
- 342 tests across all modules (was 340)

## v0.3.1 (2026-06-27) — Usability, Bug Fixes, and Quality-of-Life

### Added
- `gm run --no-report` flag to skip GRAPH_REPORT.md generation
- `gm run --exclude-dirs` flag to filter directories without `.grapheniumignore`
- `gm serve --watch` flag for auto-reloading MCP graph on file changes
- `graph_info` MCP tool: returns full graph metadata (schema, version, project root, counts)
- `recluster` MCP tool: re-run community detection after manual edits
- `min_degree` filter on `query_graph` and `summarize_file` MCP tools
- `exclude_test_nodes` parameter on `query_graph` MCP tool
- `max_neighbors` parameter on `get_neighbors` MCP tool
- Relative paths in `get_node` output (strips project root prefix)
- Dynamic dominator iteration limit scaled by subgraph size
- Graph provenance metadata displayed in `graph_stats` output

### Fixed
- Label disambiguation: `get_node` warns when label matches multiple nodes
- Import resolver normalizes edge targets for mixed-case matching
- `add_node`/`add_edge`/`remove_edge` now return total node/edge counts
- `get_community` `include_members` defaults to `false` (prevents large artifacts)
- `query_graph` truncation shows "showing X of Y matches" with guidance
- AST-only tuning banner suppressed (filter message already conveys info)
- `gm check` handles AST-only graphs without resolver annotations
- `gm setup codewhale` outputs correct TOML config format
- Snapshot create/list commands implemented (not stubs)
- `add_edge` persist round-trip test added to prevent regression

### Performance
- 340 tests across all modules (was 339)

## v0.3.0 (2026-06-25) — Trust Core, Repository Verification, Change Safety

### Added
- **Trust Foundation**: EvidenceSpan with SHA256 hashing, stale detection, Claim model
- **Trust Harness**: `check_resolution_quality()` for CI gate, `gm check` CLI command
- **CI Config Extraction**: Parse Cargo.toml, package.json, GitHub Actions, Makefile
- **Verification Planner**: 7-tier prioritized verification plans from graph diffs
- **Policy Engine**: 6 configurable trust gates (MinResolution, MaxAmbiguous, MinCallResolution, etc.)
- **Architecture Drift Detection**: Community changes, boundary crossings, hub migration over time
- **Watch-mode Blast Radius**: Live symbol diff display on file changes
- **Confidence-aware Pathfinding**: `safest_path()` preferring high-trust edges
- **Graph Node Types**: Package, BuildTarget, TestTarget, CIJob for CI/repository mapping
- **MCP Tools**: `resolution_report`, `ambiguous_symbols`, `unresolved_references`, `safest_path`, `verification_plan`, `blast_radius`, `agent_change_gate`, `diff_graph`, `next_files_to_read`, `review_plan`
- **CLI Flags**: `gm doctor --schema/--resolution/--repository`, `gm diff --review-plan`, `gm query --safe`, `gm watch --impact`, `gm snapshot create/list`, `gm gate --diff`, `gm graph schema/build-map/test-map/migrate`

### Changed
- CI workflow now includes trust check step

### Performance
- 339 tests across all modules

## v2 (unreleased)

### Added

- Graph schema versioning: `schema_version`, `graphenium_version`,
  `created_at`, `project_root`, `extraction_modes`, `languages` in metadata
- Edge and node provenance: `extractor` and `resolution_status` fields
  on every edge (tree-sitter edges marked `resolved`, LLM edges by confidence)
- Cross-file import resolver: marks import edges as `resolved` or `unresolved`
- Directed graph projections: `DirectedProjection` rebuilds directed views
  from the undirected petgraph using `src_original`/`tgt_original`
- PageRank on directed projections for architectural importance scoring
- Reverse reachability analysis: find all nodes that can reach a target
- Community boundary crossing scores: identifies cross-community connectors
- Rooted dominators: identifies mandatory gateways in dependency subgraphs
- `chokepoint_report`: combined ranking from PageRank, degree, and topology
- Confidence-aware ranking explanations for all chokepoint output
- `gm diff --before --after`: graph snapshot diffing with symbol inventory
- `symbol_inventory_diff`: detects added, removed, and community-changed symbols
- `downstream_impact`: reverse reachability for blast radius analysis
- `review_order`: recommended review order by risk (removed > changed > added)
- `gm query --mode`: lexical (default), structural (graph distance), hybrid
- Text embeddings: TF-based search with cosine similarity (`embed` module)
- Node2Vec structural embeddings: random walks + co-occurrence training
- Runtime telemetry overlay: OTEL-compatible trace import, hot path queries,
  runtime-weighted traversal with P50/P95/P99 latency percentiles
- `gm doctor` reports schema version, build version, extraction modes,
  and detected languages

### Changed

- `build_merged` now resolves imports before graph assembly
- MCP `graph_stats` output includes provenance breakdown (extractor counts)
  and graph metadata (schema version, modes, languages)
- Subgraph output in MCP tools shows `[extractor:status]` on connections

## [0.1.0] (2026-06-23)

### Added

- AST extraction for 9 languages via tree-sitter: Python, JavaScript,
  TypeScript, Rust, Go, Java, C, C++, C#
- MCP server with 13 graph tools: `query_graph`, `get_node`,
  `get_neighbors`, `get_community`, `god_nodes`, `graph_stats`,
  `shortest_path`, `architecture_summary`, `summarize_file`,
  `reload_graph`, `add_node`, `add_edge`, `remove_edge`
- Query CLI (`gm query`) with keyword-scored BFS/DFS traversal
- Louvain community detection and clustering
- AST-only extraction mode (no API key required)
- Semantic extraction mode with support for Anthropic, OpenAI,
  DeepSeek, OpenRouter, and OpenAI-compatible providers
- Watch mode (`gm watch`) with automatic rebuilds on file changes
- Incremental update support via mtime manifest
- Export to JSON and Markdown graph report
- Cross-file relationship inference
- God node and surprising connection detection
- Sensitive file detection and automatic skipping
- `.grapheniumignore` support for excluding directories
- Confidence model: EXTRACTED, INFERRED, AMBIGUOUS edge labelling
- Label disambiguation and qualified labels for duplicate symbol names
- `gm doctor` diagnostic command
- `gm setup` MCP config generator for Claude, Cursor, CodeWhale
- GitHub Actions CI with test, fmt, clippy jobs
- GitHub Actions release workflow for 5 platforms
- One-line curl installer (`install.sh`)
