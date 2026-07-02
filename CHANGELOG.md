# Changelog

All notable changes to Graphenium are documented in this file.

## v0.15.3 (2026-07-02) — Delta safety, degree-based disambiguation, hub detection

### Added
- **Large delta short-circuit**: `what_changed` returns count-only summary for diffs exceeding 5,000 changes, preventing server OOM
- **`resolve_id` degree tie-breaking**: Label collisions now always pick the highest-degree node (most likely the real implementation), via `max_by(degree)` with ID-length and name tie-breakers
- **Broadened namespace hub detection**: `is_namespace_aggregation_node` now catches external framework/namespace nodes without source spans that are imported by ≥5 distinct files

### Fixed
- `ambiguous_symbols` no longer reports same-file label collisions (overloads, partial classes — expected behavior)
- `downstream_impact` computation gated at >200 changes to avoid expensive reverse-reachability on large deltas

### Performance
- 350 tests pass, 0 clippy errors, formatting clean

## v0.15.2 (2026-07-02) — Empty section headers, extension-based language detection, test hardening

### Added
- **`format_explanation_report` now always emits section headers** — sections 1/3/5 now show "None found." when empty, matching the existing section-4 convention. No more "1, 2, 4" gap when callers section is empty.
- **Extension-based language integrity check** — `gm doctor` now detects languages from file extensions in the graph when `metadata.languages` is absent, so C#/C++/Python are checked even if the initial detection pass didn't populate the metadata field.
- **Synthetic test for aggregation predicate** — `test_is_namespace_aggregation_node_detects_import_only_hubs` verifies the degree-based hub detection works correctly.

### Fixed
- `explain_change` section gaps when "Direct Callers" list is empty — header now always emitted with "None found." message.

### Performance
- 350 tests pass (1 new), 0 clippy errors, formatting clean

## v0.15.1 (2026-07-02) — Downstream impact gating, degree-based ranking, hub filtering

### Added
- **downstream_impact gating**: Expensive reverse-reachability analysis now skipped for deltas >200 changes
- **Degree-based fragment ranking**: explain_change picks highest-degree node, not arbitrary .first()
- **CI hardening**: 9 commits of fixes since v0.15.0

### Fixed
- Downstream impact OOM on 18k->98k deltas; gates at 200 changes
- explain_change partial class fragment selection; now uses degree-ranking

### Performance
- 350 tests pass, 0 clippy errors, formatting clean

## v0.15.0 (2026-07-01) — Planning workspace persist, references_to tool, quality-of-life hardening

### Added
- **`is_namespace_aggregation_node` predicate** — Degree-based filter for import-only hub nodes. Used by `next_files_to_read` and `god_nodes` to filter namespace-hub noise
- **`references_to` MCP tool** — Structural reference lookup (containers, imports, inheritance). 100% AST-only safe
- **Planning workspace persist round-trip** — `plan_id` and `qualified_label` fields now serialized in `graph_to_value()` so planning data survives graph reload
- **`next_files_to_read` dedup + namespace filtering** — File-level deduplication and post-processing filter against namespace aggregation hubs
- **`what_changed` budget param** — New `budget` parameter caps output at a configurable character limit (default 10000) to prevent server OOM
- **`module_dependencies` hub bridging** — When both modules import the same namespace hub, crossing edges are now reported instead of "0 connections"
- **`god_nodes` test-hub filtering** — Namespace aggregation hubs and test-anchored paths are filtered from hub results with filtered count in output header
- **`ambiguous_symbols` collision detection** — Now reports label collisions (same label, different node IDs) alongside ambiguous edges
- **All `cargo install` commands now use `--locked`** — Prevents tree-sitter ABI drift, critical for C# support

### Fixed
- **16 CI compilation errors** across 5 categories: missing `#[tool(param)]` on planning workspace tools, missing `plan_id` fields in constructors, wrong arg counts in `resolve_symbols_to_ids`/`subgraph_to_text_with_match_details`, missing `format()` method on `SymbolChange`, formatting diffs

### Performance
- 350 tests pass, 0 clippy errors, formatting clean

## v0.14.0 (2026-07-01) — C# project support, build-map CLI, planning workspaces

### Added
- **C# project parser**: `src/extract/csharp_project.rs` parses `.sln` and `.csproj` files into `CSharpWorkspace`/`CSharpProject` structs with assembly names, root namespaces, and project reference chains.
- **C# dependency graph**: `csproj_to_extraction()` in `src/extract/ci.rs` injects project-boundary nodes and `depends_on` edges during `gm run`. Wired into `cmd_run` build pipeline.
- **`gm graph build-map` C# support**: CLI scans for `.sln`/`.csproj` files and lists solutions, projects, namespaces, and reference dependencies.
- **`gm diff --json`**: New structured JSON output flag.
- **`CacheManager` finalized**: `ExtractOptions.cache_manager: Option<Arc<CacheManager>>` replaces `cache_dir` for thread-safe singleton cache access.
- **C# boundary-aware resolver**: Existing `qualified_label` system handles namespace matching. Project reference boundaries stored as graph edges.
- **Parser failure logger**: `walker.rs`, `rust_lang.rs`, `go.rs` emit `eprintln!` on `set_language`/`parse` failures
- **C# extraction regression test**: `extract_csharp_file` unit test under `lang-csharp` feature
- **Language integrity check**: `check_language_extraction_integrity` in `gm doctor`
- **`references_to` MCP tool**: Structural reference lookup — containers, imports, inheritance (100% AST-only safe)
- **`explain_change` MCP + `gm explain` CLI**: Composite pre-edit orientation (hierarchy + community + callers + files)
- **Persistent planning workspaces**: `plan_id` on `Node`/`Edge` + `verify_plan` compliance engine + 3 MCP tools + `gm check --plan`
- **AST-only `blast_radius` warnings**: Safety guardrail for zero call-resolution graphs
- **Clearer `next_files_to_read`**: `resolve_symbols_to_ids` with fuzzy/comma-separated support, 3-state error messaging
- **Namespace-aware `module_dependencies`**: `is_node_in_module` with path + qualified_label matching
- **CPython noise filter**: Python stdlib labels in `FRAMEWORK_LABELS`; Windows backslash path normalization
- **Diff truncation**: `format_safe_diff` at 500-change budget, prevents server OOM on large deltas

### Changed
- `extract_file` uses `cache_manager.load_ast`/`save_ast` instead of ad-hoc paths.
- Binary call sites (`main.rs`, `watch.rs`) updated to use `CacheManager` via `Arc`.

### Fixed
- `query_transitive` MCP tool: `Arc<GrapheniumGraph>` dereference mismatch.
- `csproj_to_extraction` struct fields aligned with actual `Node`/`Edge` definitions.
- Binary build: `path.canonicalize().unwrap_or(path)` fixed to avoid `PathBuf` move.

## v0.13.0 (2026-06-30) — Telemetry struct, traversal stats, cache manager

### Added
- **`TelemetryCollector` struct**: Runtime overlay data container in `src/telemetry.rs` with `total_nodes`, `total_edges`, `hub_count`, `compression_ratio`.
- **`traversal_stats` in query output**: All `query_graph` responses include a compact stats line: `[traversal stats: X hubs, Y items, Z:1 compression ratio]`.
- **`CacheManager` with atomic persistence**: `load_ast`/`save_ast`/`load_semantic`/`save_semantic` methods with atomic temp-then-rename.
- **C# project parser (initial)**: `CSharpWorkspace`/`CSharpProject` structs, solution-file enumeration, `ProjectReference` tracking.

### Changed
- `ExtractOptions.cache_dir` replaced by `cache_manager: Option<Arc<CacheManager>>` for thread-safe singleton access.

### Performance
- 349 tests pass, zero clippy errors.
- CacheManager tests: save/load roundtrip, cache miss returns None, auto subdirectory creation.

## v0.12.0 (2026-06-30) — AST caching pipeline

### Added
- **AST content-hash caching**: Extraction now caches per-file `ExtractionResult` by SHA256 hash under `graphenium-out/cache/ast/`. Unchanged files skip tree-sitter parsing entirely on subsequent rebuilds.
- **`cache_dir` field on `ExtractOptions`**: Threaded through `main.rs` and `watch.rs` call sites. Incremental watch-mode rebuilds benefit from the same cache.
- **C# namespace resolution**: Existing cross-file resolver already indexes `qualified_label` and resolves `using` namespace imports — verified working with existing test suite.

### Performance
- Second `gm run` on an unchanged repo completes in under 1 second (100% cache hit).
- Incremental rebuilds after `.grapheniumignore` changes only re-parse files whose content actually changed.

## v0.11.0 (2026-06-30) — Large-repo robustness, pre-scan planning, JSON output

### Added
- **Progress heartbeats**: Extraction now prints progress every 500 files
- **`gm run --plan`**: Pre-scan mode reporting file stats and vendored/library directories without full extraction
- **`gm query --json`**: Machine-parseable JSON output for CLI tools
- **Trust banner in `graph_info`**: Shows graph mode, call resolution status, and edge-type guidance at session start
- **`query_transitive --budget`**: Character-budget parameter with depth-grouped truncation
- **Qualified labels in subgraph output**: Collision-aware display showing `qualified_label (label)`
- **`gm init` ignore patterns**: Now includes C++ (`obj/`, `.vs/`, `ipch/`) and C# (`*.Designer.cs`) build artifacts
- **`quality.json` fully populated**: `by_relation` (edge-type counts), `top_risks` (high-degree unresolved nodes)
- **`scripts/verify_robustness.sh`**: Automated robustness checks for server startup, warnings, typos

### Fixed
- `graphemium-snapshots` typo in `what_changed` (corrected to `graphenium-snapshots`)

## v0.10.0 (2026-06-30) — Windows onboarding, Claude Code setup, graceful startup

### Added
- **Graceful server startup**: `gm serve` now starts with an empty graph when `graph.json` is missing — no more "Failed to connect" on first launch. Watches parent directory for file creation; auto-reloads when graph appears.
- **`gm setup claude-code`**: New target printing `claude mcp add` command for Claude Code CLI, with skill installation instructions. `gm setup claude-desktop` now shows platform-specific config paths (macOS/Windows/Linux).
- **`install.ps1`**: PowerShell installer for Windows — detects Cargo, builds from source, installs the Claude Code skill, verifies installation.
- **Empty graph MCP guidance**: `query_graph` returns a helpful "run `gm run .`" message when the graph is empty.
- **Windows path normalization**: `clean_windows_path()` strips `\\?\` extended-length prefix from all displayed paths.

### Changed
- `gm setup claude` now prints available sub-targets (`claude-desktop`, `claude-code`) instead of a single snippet.
- `graph_info` MCP tool now displays clean (UNC-stripped) project root paths.

### Performance
- 342 tests pass, zero clippy warnings.

## v0.9.0 (2026-06-28) — Modular documentation restructuring

### Added
- **`docs/COMMAND_REFERENCE.md`** — Complete CLI reference: all 12 commands with flags, options tables, usage templates, and common workflows
- **`docs/MCP_TOOLS.md`** — Full MCP tool catalog: 22 tools across 5 categories (read, write, composite, trust, diff) with parameters and use cases
- **`docs/ARCHITECTURE.md`** — Three-tier model, graph schema, extraction pipeline, trust model, module map, and limitations
- **`docs/COMPARISON.md`** — Competitive comparison: 15-capability table across grep, AST tools, semantic search, and Graphenium
- **`docs/BENCHMARKING.md`** — Token-reduction benchmarks with methodology and self-analysis commands
- **`scripts/run_benchmarks.sh`** — Automated benchmark runner with character-count budgets

### Changed
- **`README.md`** — Restructured from 887 lines to 144-line landing page with quick start, MCP setup, feature summary, and links to modular docs
- Version bumped to 0.9.0

## v0.8.0 (2026-06-27) — Diagnostics, init, auto-watch, transitive direction

### Added
- **`gm doctor --json`**: Structured JSON diagnostics with binary, graph, quality, and API key status
- **`gm init`**: Workspace initializer — creates `.grapheniumignore` with sensible defaults for Rust, Python, Node.js
- **Auto-watch by default**: `gm serve` now automatically watches the graph file for changes and reloads — no more manual `reload_graph` calls
- **`query_transitive` direction param**: Support `forward` (outgoing edges), `reverse` (incoming), or `both` for dependency/impact analysis
- **Budget estimation**: Truncation messages now estimate remaining characters needed, helping agents calibrate their budget

### Fixed
- **AST-only policy gates**: `MinCallResolution` policy now auto-passes for AST-only graphs with clear `[SKIPPED]` message
- **`is_test_like_path` patterns**: Added `_test.rs` and `_tests.rs` to test exclusion patterns
- **Relative paths**: `analyse_symbol` and `module_dependencies` now use project-root-relative paths

## v0.7.0 (2026-06-27) — Path normalization, trust scaling, transitive queries

### Added
- **`relative_path` helper**: Project-root-relative paths in `subgraph_to_text_with_match_details` output — no more absolute host paths in query responses
- **`format_path_confidence`**: Trust breakdown for every step in `shortest_path` output (relation, confidence, provenance)
- **`query_transitive` MCP tool**: BFS transitive closure from a seed symbol — finds all reachable nodes with depth control and relation filtering
- **`is_test_like_path`**: Path-based test/spec detection in `filtered_node_ids` — more precise test exclusion patterns (handles `.spec.`, `/spec/`, `_bench` paths)
- **AST-only trust scaling**: `gm check` now dynamically skips call resolution metrics for AST-only graphs and shows `[SKIPPED]` with clear explanation

### Fixed
- **`displayed` counter bug**: Truncation warning in `subgraph_to_text_with_match_details` now correctly shows actual displayed count instead of always `0`

## v0.6.0 (2026-06-27) — Token optimization, composite tools, trust gating

### Added
- **Degree-grouped truncation**: `summarize_file` now splits hubs (degree > 5) from leaves; leaves hidden by default with `show_leaves=true` to expand. Payloads drop from 30KB to under 3KB for large files.
- **`include_tests` parameter**: Replaces `exclude_test_nodes` with inverted logic; defaults to `false` so test/spec nodes are excluded by default across `query_graph`, `get_node`, `get_neighbors`, and `god_nodes`.
- **`extracted_only` strict mode**: Added to `get_neighbors` — filters to only `EXTRACTED` confidence (source-backed ground truth). Zero heuristic/ambiguous edges.
- **Trust Profile summaries**: Every `query_graph` response now appends a confidence breakdown: `N EXTRACTED, N INFERRED, N AMBIGUOUS` so agents can gauge trust at a glance.
- **`analyse_symbol` MCP tool**: Single-turn composite analysis — resolves a symbol and groups behavioral connections (calls, uses, inherits) vs structural (imports, contains).
- **`module_dependencies` MCP tool**: Module-to-module dependency summary between two path fragments.
- **`what_changed` MCP tool**: Risk-sorted delta against a stored snapshot — shows removed symbols first, then community moves, then additions with downstream impact.

## v0.5.0 (2026-06-27) — Graph identity, relative paths, demo polish

### Added
- `graph_info` MCP tool exposing schema version, project root, build timestamp, extraction mode, languages, and counts
- Relative paths in all MCP tool outputs (get_node, get_neighbors, god_nodes, summarize_file, architecture_summary)
- Demo script uses relative directory discovery instead of hardcoded absolute paths

### Fixed
- AST-only `gm check` output now clearly explains that resolution requires the semantic pass

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
