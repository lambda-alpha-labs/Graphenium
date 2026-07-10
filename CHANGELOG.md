# Changelog

This file summarizes notable releases and preserves the major documentation history.

## v0.19.0, 2026-07-10

Theme: Datalog Standard Library and goal-directed query evaluation.

Added:

- Pre-loaded Datalog Standard Library (`src/analyze/query/stdlib.dl`) embedded in the binary with predicates for transitive reachability (`calls_transitive`, `depends_transitive`), topology (`same_community`, `is_hub`, `is_orphan`), and architectural constraints (`circular_dependency`, `bypasses_layer`).
- Typed EDB relations (`calls`, `imports`, `contains`, `inherits`, `implements`, `degree`, `hub`) alongside legacy `edge` and `node` facts.
- Goal-directed rule selection: only stdlib rules reachable from query goals are evaluated; EDB-only queries skip fixpoint iteration entirely.
- Cached stdlib AST parsing via `OnceLock` and `parsed_stdlib_rules()` in `src/cache/query.rs`.
- Integration tests in `tests/datalog_stdlib_test.rs`.

Changed:

- `run_datalog_query` automatically merges stdlib rules into every query.
- `run_datalog` MCP tool description documents pre-loaded predicates and base EDB relations.
- Graphenium skill (`skills/graphenium/SKILL.md`) instructs agents to use stdlib predicates instead of hand-written recursion.

Fixed:

- Anonymous `_` variables in Datalog rules no longer collide across atoms (fixes negation in `is_orphan` and related predicates).
- Goal evaluation respects constant constraints (e.g. `same_community("x1", X)` no longer returns unrelated matches).
- Fixpoint solver returns an explicit error when the step budget is exhausted without convergence.

Performance:

- EDB queries on the 1,211-node self-analysis graph complete in under one second (previously hung indefinitely while evaluating full transitive closures).

## v0.18.0, 2026-07-03

Theme: working cross-file resolution improvements, especially for C#.

Highlights:

- Captures C# member-access calls such as `Helper.DoWork()`.
- Adds C# inherits and implements edges from `base_list` structures.
- Rewrites resolver uniqueness gating with same-language filtering, distinct-ID deduplication, and subsumption checks.
- Fixes serve-layer routing so MCP endpoints are not intercepted by static file handling.
- Fixes `blast_radius` and `verification_plan` label resolution.
- Improves AST-only banners so they reflect actual resolution status.

Reported result on a real 98k-node C# graph:

| Metric | Before | After |
|---|---:|---:|
| Calls resolved | 0 percent | 42 percent |
| Cross-file references resolved | 0 | 38,641 |
| Implements edges | 0 | 1,713 |
| Inherits edges | 0 | 2,219 |
| Dangling edges | unknown | 0 |
| Communities | 4,140 | 775 |

## v0.17.0, 2026-07-03

Theme: major enhancement set and documentation restructuring.

Added:

- Cross-file call resolution
- C# inherits and implements support
- Scope-narrowed resolution
- Datalog query engine
- Runtime telemetry overlay
- Salsa-backed extraction
- Hybrid retrieval modes

Changed:

- README slimmed and modular docs moved into `docs/`.
- Contributing guide expanded with new module definitions.
- Skill instructions updated with Datalog, query modes, C# guidance, and cross-file resolution instructions.
- AI setup expanded with Datalog, OpenTelemetry, Salsa, and hybrid retrieval sections.
- Cargo install documentation moved to `--locked`.

Quality:

- Binary build passed.
- CI passed with zero clippy errors and clean formatting.
- 363 tests passed.

## v0.16.x, 2026-07-02

Theme: Stack Graphs, OpenTelemetry, Salsa, Datalog, hybrid retrieval, C# support, and CI fixes.

Added:

- Cross-file reference resolution
- OpenTelemetry runtime overlay
- Salsa incremental computation
- Datalog query engine
- Hybrid lexical and structural retrieval
- C# inherits and implements edges
- Scope-narrowed call resolution
- `run_datalog` MCP tool

## v0.15.x, 2026-07-01 to 2026-07-02

Theme: planning workspaces, large-delta robustness, hub filtering, path disambiguation, and reviewer safety.

Added and fixed:

- Planning workspace persistence
- `references_to` MCP tool
- `what_changed` budget controls
- Large delta short-circuiting
- Downstream impact gating
- Degree-based disambiguation
- Namespace aggregation hub filtering
- Better handling of ambiguous symbols
- Installer hardening with `cargo install --locked`

## v0.14.0, 2026-07-01

Theme: C# project support and planning workspaces.

Added:

- C# solution and project parser
- C# dependency graph boundaries
- `gm graph build-map`
- `gm diff --json`
- Persistent planning workspaces
- `gm explain`
- AST-only blast-radius warnings
- Clearer `next_files_to_read`

## v0.13.0, 2026-06-30

Theme: telemetry data structures, traversal stats, and cache manager.

Added:

- `TelemetryCollector`
- traversal stats in query output
- atomic cache manager
- initial C# project parser

## v0.12.0, 2026-06-30

Theme: AST caching.

Added:

- content-hash AST cache
- cache directory support
- incremental watch-mode speedups
- verified C# namespace resolution behavior

## v0.11.0, 2026-06-30

Theme: large-repository robustness, pre-scan planning, and JSON output.

Added:

- extraction progress heartbeats
- `gm run --plan`
- `gm query --json`
- trust banner in `graph_info`
- `query_transitive --budget`
- qualified labels in query output
- stronger `.grapheniumignore` defaults
- populated `quality.json`
- robustness script

## v0.10.0, 2026-06-30

Theme: Windows onboarding, Claude Code setup, and graceful server startup.

Added:

- graceful `gm serve` startup with empty graph
- `gm setup claude-code`
- PowerShell installer
- helpful empty-graph MCP guidance
- Windows path normalization

## v0.9.0, 2026-06-28

Theme: modular documentation.

Added:

- command reference
- MCP tools reference
- architecture guide
- comparison guide
- benchmarking guide
- benchmark script
- streamlined README

## Earlier releases

Earlier releases introduced graph identity, relative paths, token optimization, composite tools, trust gating, transitive queries, incremental rebuild fixes, and improved traversal output.

## Changelog principle

Release notes should emphasize what changed for agent safety and reviewer confidence, not only implementation details.
