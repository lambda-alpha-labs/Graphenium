# Changelog

This changelog summarizes key updates and engineering milestones for Graphenium, emphasizing improvements in AI-agent write-safety, structural verification, and external governance gates.

---

## v0.19.1 — 2026-07-12

### Summary
This release introduces **Zero-Drift Gating (Topological Entropy Guardrails)** — a configuration-free invariant gate that rejects agent plans which mathematically degrade repository modularity.

### Added
*   **Topological Delta Core (`src/analyze/delta.rs`):** Partitions physical and virtual subgraphs, computes Louvain modularity deltas (ΔQ), profiles surprise edges, and detects community drift for planning workspaces.
*   **`evaluate_delta_gate` MCP Tool:** Exposes in-memory modularity delta checks and surprise analysis to AI agents for real-time design validation.
*   **`gm check --delta`:** CLI topological entropy gate with `--plan`, `--mod-tolerance` (default: `-0.02`), and `--surprise-threshold` (default: `5.0`) flags.
*   **Zero-Config Fallback in `validate_plan`:** Orchestrates explicit `.graphenium/policy.json` rules first, then applies Dynamic Delta Gating as an invariant fallback when no policy file is configured.
*   **Policy Gates Banner in `graph_info`:** Reports active containment layers (explicit policy rules + Dynamic Delta Gating).

### Changed
*   `validate_plan` (MCP) now runs policy rules and dynamic delta gating in sequence.
*   Agent skill (`skills/graphenium/SKILL.md`) updated with `evaluate_delta_gate` tool guidance and topological delta failure resolution steps.
*   `install.sh` now installs the Claude Code skill to `~/.claude/skills/graphenium/SKILL.md` on Unix installs.
*   Documentation ecosystem updated for zero-config CI gates, dual-graph delta solver, and topological entropy trust model.

---

## v0.19.0 — 2026-07-11

### Summary
This release introduces the pre-loaded Datalog Standard Library, pre-flight architectural policy validation, and deployment hardening for agent containment.

### Added
*   **Datalog Standard Library (`src/analyze/query/stdlib.dl`):** Compiled directly into the binary, providing pre-loaded first-order logic predicates for transitive reachability analysis (`calls_transitive`, `depends_transitive`), structural topology (`is_hub`, `is_orphan`), and architectural constraints (`circular_dependency`, `bypasses_layer`).
*   **Strongly-Typed EDB Relations:** Introduced typed Extensional Database (EDB) relations (`calls`, `imports`, `contains`, `inherits`, `implements`, `degree`, `hub`) to operate alongside low-level structural facts.
*   **Goal-Directed Query Pruning:** Optimizes solver performance by evaluating only the stdlib rules reachable from the query's goals. EDB-only queries completely bypass fixed-point iteration, preventing execution hangs on large codebases.
*   **Pre-Flight Architectural Policy Engine (`src/policy.rs` & `src/harness.rs`):** Implements declarative rule evaluations loaded from `.graphenium/policy.json`. Supports `forbidden_dependency`, `strict_layering`, and `banned_symbol` validation.
*   **Transitive Layer Checks:** Integrated the Datalog `depends_transitive` closure into the pre-flight engine to mathematically prove layer-bypassing violations before an agent writes code.
*   **`validate_plan` MCP Tool:** Exposes explicit, pre-flight structural checks on virtual planning workspaces.
*   **Automated Workspace Gates:** Gated `add_planned_symbol` to automatically run pre-flight policy checks, returning `PRE_FLIGHT_VIOLATION` to block invalid agent designs.
*   **`agent_change_gate` Upgrades:** Added an optional `plan_id` parameter to run combined pre-flight policy checks and post-facto compliance audits in a single workflow.
*   **freshness detection (`src/serve/freshness.rs`):** Compares the cached index modification times (`graph.json`) against the running binary and physical source files, appending warnings to `graph_info` and `reload_graph` if the index is stale.

### Changed
*   `run_datalog_query` now automatically merges Graphenium's Datalog standard library into all custom queries.
*   The `run_datalog` MCP tool description has been updated to document pre-loaded standard library predicates and EDB relations.
*   The system skill instruction set (`skills/graphenium/SKILL.md`) now directs agents to use pre-loaded Datalog predicates instead of implementing manual recursive rules.
*   `gm check --plan` now executes two gates in sequence: pre-flight policy validation, followed by post-facto compliance auditing.

### Fixed
*   Resolved an issue where anonymous `_` variables in Datalog rules collided across atoms, correcting the behavior of negation filters like `is_orphan`.
*   Corrected goal evaluation constraints so that scoped queries (e.g., `same_community("node_x", X)`) do more than project arbitrary tuples.
*   Fixed-point solver now returns an explicit execution error instead of spinning indefinitely if its step budget is exhausted before convergence.

---

## v0.18.0 — 2026-07-03

### Summary
This release hardens cross-file symbol resolution across the compilation pipeline, focusing heavily on enterprise C# codebase structures.

### Added
*   **C# Scope-Narrowed Call Resolution:** Captures member-access expressions (such as `Helper.DoWork()`) and binds them to their unique, AST-proven type definition.
*   **C# Inheritance Analysis:** Extracts C# type inheritance (`inherits`) and interface implementations (`implements`) from AST `base_list` structures.
*   **Language-Family Resolver Isolation:** Constrains Graphenium's cross-file binder to candidates of the same language family, preventing name collisions across multi-language projects (e.g., separating C# methods from similarly named C++ headers).

### Fixed
*   Rewrote Graphenium's cross-file reference resolver to filter out sub-symbol granularity overlaps (subsumption checks), preventing double-counting or ambiguous bindings.
*   Fixed serve-layer routing to prevent background MCP endpoints from being intercepted by static file handling.
*   Corrected target labels within `blast_radius` and `verification_plan` calculations.

---

## v0.17.0 — 2026-07-03

### Summary
This release reorganizes documentation around external governance gating and introduces incremental AST index patching.

### Added
*   **AST-Proven Cross-File Call Resolution:** Maps calls across file boundaries using deterministic AST parsing.
*   **Incremental Index Patching (`replace_file_extraction`):** Re-extracts modified files, purges stale symbol data, and patches the cached index without executing a full project re-scan.
*   **Datalog Query Interpreter:** Core engine implementation for evaluating logical codebase constraints.
*   **Salsa-Backed Memoized Parsing:** Implements memoized incremental extraction to speed up file-watching recompilations.

---

## v0.16.x — 2026-07-02

### Summary
Introduced local Stack Graphs, runtime telemetry overlays, and initial C# project reference boundaries.

### Added
*   **Local Stack Graphs:** Deterministic cross-file symbol resolution based on AST bindings.
*   **Runtime Telemetry Overlay (`src/telemetry.rs`):** Imports OpenTelemetry JSON traces to overlay live call counts and latency percentiles onto the static AST index.
*   **C# Project Boundary Parser (`src/extract/csharp_project.rs`):** Parses Visual Studio `.sln` and `.csproj` structures to model assembly boundaries and project references.

---

## v0.15.x — 2026-07-01 to 2026-07-02

### Summary
Introduced virtual planning workspaces and post-edit verification.

### Added
*   **Planning Workspaces:** Provides persistent virtual draft states where agents must declare their design intent.
*   **`verification_plan` Generation:** Generates risk-sorted verification checklists (affected interfaces, dependent callers, covering tests) for changed symbols.
*   **`what_changed` Audits:** Diff-based reporting comparing cached index snapshots to highlight additions, removals, and community moves.

---

## v0.14.0 — 2026-07-01

### Summary
Initial C# integration and persistent design plans.

### Added
*   C# syntax extraction.
*   Initial draft workspaces with post-facto file-scope audits.

---

## v0.13.0 — 2026-06-30

### Summary
Introduced telemetry data structures, transaction-safe caches, and traversal metrics.

### Added
*   Telemetry collector structures.
*   Atomic cache manager to write index changes transactionally.

---

## v0.12.0 — 2026-06-30

### Summary
Introduced incremental watch-mode file updates and content-hashed caching.

### Added
*   File-content SHA256 hashing to manage the AST extraction cache.
*   File-system watch-mode support.

---

## v0.11.0 — 2026-06-30

### Summary
Optimized Graphenium for larger repositories and added pre-scan planning.

### Added
*   Progress bar and heartbeat logs for large-repository indexing.
*   Dry-run planning flag (`gm run --plan`) to inspect project scope.

---

## v0.10.0 — 2026-06-30

### Summary
Hardened deployment on Windows environments and added setup scripts.

### Added
*   PowerShell installer (`install.ps1`).
*   Automatic path normalization to resolve differences in Windows backslash paths.
*   Helpful warnings for workspace initialization.