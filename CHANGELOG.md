# Changelog

This changelog summarizes key updates and engineering milestones for Graphenium, emphasizing relational codebase compilation, AI-agent write-safety, structural verification, and external governance gates.

---

## v0.24.0 - 2026-11-06

### Summary
Structural precision and containment-hardening release. This milestone removes major graph-topology artifacts, fixes cohesion scoring, closes post-edit policy loopholes, and makes watch-mode resolution consistent with full builds.

### Fixed
*   **Stdlib Method False Bindings (`src/resolver.rs`):** Bare ubiquitous method names (`len`, `new`, `clone`, `write`, `read`, …) no longer bind cross-file to unrelated project symbols. The `Manifest::len` god-node artifact is eliminated.
*   **Same-Named Module Collapse (`src/extract/{mod,rust_lang,walker,go}.rs`):** `mod.rs`, `lib.rs`, `main.rs`, `index.ts`, `__init__.py`, and `__main__` now use root-relative ID prefixes, so same-named files in different directories remain distinct. The AST cache format version was bumped so stale caches cannot keep serving collapsed IDs.
*   **Community Cohesion Above 1.0 (`src/cluster/cohesion.rs`):** Cohesion now counts unique undirected node pairs and ignores self-loops, keeping reported cohesion inside `[0.0, 1.0]` even with parallel edges.
*   **Missing ΔQ Violation (`src/harness.rs`):** `validate_plan` now emits an explicit modularity-decay violation when a plan fails on ΔQ alone.
*   **Post-Edit Verification Loopholes (`src/harness.rs`):** Planned nodes must be implemented in the planned file; planned edges require the correct direction; and physical edges originating from approved files are re-checked against `.graphenium/policy.json`. `PlanVerificationReport` gained `policy_violations`.
*   **Intra-File Label Collisions (`src/extract/{rust_lang,walker,go}.rs`):** Duplicate short names such as multiple `fn new()` definitions are treated as ambiguous instead of binding calls to an arbitrary implementation.
*   **Watch-Mode Resolution (`src/watch.rs`, `src/cache/query.rs`):** Incremental patches now run import and cross-file call resolution against the existing graph index, and watch-mode full rebuilds use the same cross-file resolver as `gm run`.
*   **Worked Case Study (`worked/graphenium-self-analysis/`):** Regenerated the self-analysis report, sample queries, and case-study summary against v0.24.0. The report no longer contains `Manifest::len` or `mod` god nodes, and all community cohesion values are `≤ 1.000`.

### Changed
*   **CLI and MCP Output:** `gm check --plan` and `verify_changes` now surface physical policy violations alongside missing nodes, missing edges, and scope creep.
*   **Version:** Bumped to `0.24.0`.

### Tests
*   Added regression tests for stdlib method denylisting, structural module IDs, parallel-edge cohesion, planned-file implementation checks, directed edge verification, physical policy violations, and ambiguous intra-file method names.
*   `cargo test --tests`: 438 tests pass (423 library, 10 Datalog, 4 planning, 1 trust benchmark).

---

## v0.23.1 - 2026-11-02

### Summary
Documentation-only patch release improving MCP tool definition quality for Glama TDQS scoring.

### Changed
*   **MCP Tool Descriptions (`src/serve/handlers.rs`):** Expanded the schema descriptions for `create_planning_workspace`, `add_planned_symbol`, `inspect_symbol`, `query_graph`, and `trace_path` with explicit read-only and mutating markers, sibling-tool routing guidance, prerequisites, idempotency, failure modes, and output-size guidance.
*   **Release Notes Workflow (`.github/workflows/release.yml`):** Updated generated release notes to describe the current 12-tool MCP surface.

### Tests
*   `cargo check --lib` and `cargo test` pass.

---

## v0.23.0 - 2026-11-01

### Summary
MCP tool surface consolidation from 37 tools to 12 cohesive verb_noun tools, with complete JSON schema parameter documentation and updated documentation and website references.

### Changed
*   **Consolidated MCP Surface (`src/serve/handlers.rs`):** Reduced the exposed `#[tool]` endpoints from 37 to 12. Orientation and health now use `get_graph_info`, `get_architecture_summary`, and `reload_graph`; search and tracing use `query_graph`, `inspect_symbol`, `trace_path`, and `run_datalog`; planning and containment use `create_planning_workspace`, `add_planned_symbol`, `validate_plan`, and `verify_changes`; manual mutations use `mutate_graph`.
*   **Preserved Backends:** Moved former tool implementations into plain backend methods so no analysis, traversal, planning, verification, or mutation capability was removed.
*   **Extra Legacy Diagnostics:** Folded `resolution_report` and `ambiguous_symbols` into `get_graph_info`, and `next_files_to_read`, `review_plan`, `what_changed`, and `diff_graph` into `verify_changes`.
*   **Parameter Schemas:** Documented every parameter of the 12 tools with `#[schemars(description = "...")]` so the generated MCP JSON schema has a description for every property.

### Tests
*   Updated handler unit tests to exercise the consolidated tools.
*   Added tests that assert exactly 12 tools are exposed and that every tool parameter has a schema description.

### Documentation
*   Rewrote `docs/MCP_TOOLS.md` around the 12-tool surface and added a migration map.
*   Updated `skills/graphenium/SKILL.md`, `README.md`, `README_improved.md`, `AI_SETUP.md`, and related `docs/` guides.
*   Updated the `graphenium.dev` website content and rebuilt the site with `12 MCP tools`.

---

## v0.22.0 - 2026-11-01

### Summary
Lean 4 proof-gap case study release: deterministic clustering, stable anonymous-instance IDs, import-stub source attribution, and a faster goal-directed taint query.

### Added
*   **FLT Lean 4 Case Study (`worked/flt-lean4-proof-gaps/README.md`):** End-to-end study of `ImperialCollegeLondon/FLT`, including the proof-gap/bottleneck map, the difference between module/import-propagated taint (11.8%) and strict declaration-level taint (1.93%), and a Lean-verified proof of the four `sorry`s in `WeierstrassCurve.galoisRepresentation`.
*   **Stable Anonymous Instance IDs (`src/extract/lean.rs`):** Anonymous `instance` declarations now derive IDs from a normalized header signature (up to `:=` or `where`) plus an FNV-1a suffix, with a per-file ordinal for duplicate signatures. Inserting lines above an anonymous instance no longer renames its graph node.
*   **Lean 4 Extractor Upgrades (`src/extract/lean.rs`):** Parses `public import`, `@[expose] public section`, named and anonymous `instance` declarations, and module-qualified IDs relative to the project root. FNV-1a suffixes disambiguate lossy Lean names such as `actionTensorCAlg'` versus `actionTensorCAlg`.

### Fixed
*   **Deterministic Louvain Clustering (`src/cluster/louvain.rs`):** Replaced `HashMap` with `BTreeMap` for neighbour and aggregation maps. Identical graphs no longer produce different community counts (previously 9–20) across runs.
*   **Import-Stub Source Attribution (`src/model/graph.rs`):** Import-site stubs no longer overwrite a real module node’s `source_file` and source span. Added a regression test.
*   **Goal-Directed Taint Query (`src/analyze/query/stdlib.dl`):** `tainted_by_proof_gap` now propagates backwards from gaps instead of materialising the full `depends_transitive` closure, keeping large Lean repositories inside the solver budget.

### Changed
*   **Documentation:** Linked the FLT case study from the root `README.md`, `MANIFEST.md`, and `docs/WORKED_EXAMPLES.md`.

---

## v0.21.0 - 2026-10-31

### Summary
Minor release closing post-v0.20.0 compliance, MCP audit, feature-gating, and adapter gaps.

### Fixed
*   **Planned-Edge Compliance:** `passes_compliance` now fails when a planned dependency edge is missing. `gm check --plan` and the MCP tools report implemented and missing edges.
*   **MCP Post-Edit Audits:** `agent_change_gate` and `get_plan_details` now run `verify_plan`, so MCP clients can detect missing nodes/edges and unplanned modified files after implementation.
*   **C# Inheritance Cleanup:** Removed the unreachable `base_list` branch and `handle_csharp_base_list` from `src/extract/import_handlers.rs`; canonical walker node IDs and the `I`-prefix heuristic remain.
*   **Scope-Audit Docs:** Corrected “git/manifest” wording to git status or explicit snapshot overrides; `verify_plan` does not read the manifest.

### Changed
*   **Feature Gating:** Added positive `mcp`, `watch`, and `semantic` features. `harness = []` remains for compatibility, and `default-features = false, features = ["harness"]` now produces a genuinely lightweight core library.
*   **Harness Adapter:** Fixed the reference adapter’s imports, edge-removal API, feature configuration, and tests. `cargo test` in `contrib/harness-adapter` passes without warnings.
*   **Repository Positioning:** Reframed Graphenium across the README, CLI help, rustdoc, MCP tool descriptions, and documentation as a **relational codebase compiler and spec-driven containment engine**, highlighting the dual value proposition (context compression + mechanical containment) and the five technical pillars.

---

## v0.20.0 - 2026-10-31

### Summary
This release delivers foundational compiler, Datalog, C# discovery, CLI, and formal-verification upgrades, including first-class Lean 4 indexing and proof-gap auditing.

### Added
*   **Import-Scoped Cross-File Resolution (`src/resolver.rs`):** Cross-file call, use, inheritance, and implementation edges now narrow colliding symbol names using the caller file's explicit imports, with same-directory fallback. This raises call-resolution recall without binding ambiguous names arbitrarily.
*   **Indexed Semi-Naive Datalog Evaluation (`src/analyze/query.rs`):** Positive and negated body atoms use first-column hash indexes for bound joins; evaluation now uses true semi-naive delta relations and stops early for fully-ground existence goals.
*   **Native Lean 4 Extractor (`src/extract/lean.rs`):** Pure-Rust support for `.lean` repositories, extracting imports, namespaces, `theorem`/`lemma`/`def`/`structure`/`class`/`inductive`/`axiom`/`opaque` declarations, dependency edges, and `sorry`/`admit` proof gaps as `AMBIGUOUS` `unverified_sorry` self-edges.
*   **Formal Verification Datalog Predicates (`src/analyze/query/stdlib.dl`):** Added `contains_transitive`, `aliased_container_call`, `unreachable_from`, `has_proof_gap`, and `tainted_by_proof_gap`.
*   **`gm graph migrate`:** Migrates older index files to schema `0.2.0` and records the current Graphenium version.
*   **`gm gate`:** Runs a critical-drift gate over two snapshots (`--diff <before> <after>`), or the standard `gm check` gate over `graphenium-out/graph.json` when no diff is supplied.
*   **Recursive C# Project Discovery (`src/main.rs`):** `.sln` and `.csproj` files are now found at any depth with `.gitignore` / `.grapheniumignore` and skip-directory awareness.
*   **Planned-Edge Verification:** `PlanVerificationReport` now reports `implemented_edges` and `missing_edges` alongside planned nodes.

### Changed
*   **Post-Edit Scope Audits:** `verify_plan` now compares against git-status modified files (or an explicit override) instead of treating every physical repository file as a potential unplanned edit. `verify_plan_with_modified_files` accepts explicit snapshot-diff inputs.
*   **Language Detection:** Lean 4 is recognized by `gm run` language metadata and `gm doctor` integrity checks.
*   **Release Metadata:** Version bumped to `0.20.0`; release notes advertise the expanded MCP tool catalog and the new compiler/gating capabilities.

### Fixed
*   **Scope-Creep False Positives:** Valid plans on repositories with many untouched files no longer fail compliance because of baseline files.
*   **Clippy Cleanliness:** Resolved pre-existing library and CLI warnings so `cargo clippy --lib -- -D warnings` and `cargo clippy --bin gm -- -D warnings` pass.

---

## v0.19.3 - 2026-07-13

### Summary
Patch release restoring Datalog stdlib usability on real-size codebases.

### Fixed
*   **Datalog EDB fact-cap rejection (`src/analyze/query.rs`):** Raised the base-relation backstop in `run_datalog_query` from 100,000 to 10,000,000 facts. The previous cap fired at EDB load time - before goal-directed evaluation could narrow anything - and rejected every stdlib query on graphs with >100k edges (e.g. `?- is_hub(X).` on a ~225k-edge codebase). Evaluation remains bounded by `step_budget`; the backstop is a sanity limit on materialized EDB size only.

---

## v0.19.2 - 2026-07-12

### Summary
Patch release fixing MCP tool schema compatibility with Google Gemini / Vertex AI function calling ([#37](https://github.com/lambda-alpha-labs/Graphenium/issues/37)).

### Fixed
*   **Gemini/Vertex MCP schema rejection:** Optional tool parameters (e.g. `add_edge.weight`) no longer emit nullable unions (`type: ["number", "null"]` or `anyOf` with sibling `description`/`format` keys) that Google's function-calling API rejects.
*   **`list_tools` schema sanitization (`src/serve/tool_schema.rs`):** Flattens nullable unions to plain types and relies on the `required` array for optionality before schemas are returned to MCP clients.

---

## v0.19.1 - 2026-07-12

### Summary
This release introduces **Zero-Drift Gating (Topological Entropy Guardrails)** - a configuration-free invariant gate that rejects agent plans which mathematically degrade repository modularity.

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

## v0.19.0 - 2026-07-11

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

## v0.18.0 - 2026-07-03

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

## v0.17.0 - 2026-07-03

### Summary
This release reorganizes documentation around external governance gating and introduces incremental AST index patching.

### Added
*   **AST-Proven Cross-File Call Resolution:** Maps calls across file boundaries using deterministic AST parsing.
*   **Incremental Index Patching (`replace_file_extraction`):** Re-extracts modified files, purges stale symbol data, and patches the cached index without executing a full project re-scan.
*   **Datalog Query Interpreter:** Core engine implementation for evaluating logical codebase constraints.
*   **Salsa-Backed Memoized Parsing:** Implements memoized incremental extraction to speed up file-watching recompilations.

---

## v0.16.x - 2026-07-02

### Summary
Introduced local Stack Graphs, runtime telemetry overlays, and initial C# project reference boundaries.

### Added
*   **Local Stack Graphs:** Deterministic cross-file symbol resolution based on AST bindings.
*   **Runtime Telemetry Overlay (`src/telemetry.rs`):** Imports OpenTelemetry JSON traces to overlay live call counts and latency percentiles onto the static AST index.
*   **C# Project Boundary Parser (`src/extract/csharp_project.rs`):** Parses Visual Studio `.sln` and `.csproj` structures to model assembly boundaries and project references.

---

## v0.15.x - 2026-07-01 to 2026-07-02

### Summary
Introduced virtual planning workspaces and post-edit verification.

### Added
*   **Planning Workspaces:** Provides persistent virtual draft states where agents must declare their design intent.
*   **`verification_plan` Generation:** Generates risk-sorted verification checklists (affected interfaces, dependent callers, covering tests) for changed symbols.
*   **`what_changed` Audits:** Diff-based reporting comparing cached index snapshots to highlight additions, removals, and community moves.

---

## v0.14.0 - 2026-07-01

### Summary
Initial C# integration and persistent design plans.

### Added
*   C# syntax extraction.
*   Initial draft workspaces with post-facto file-scope audits.

---

## v0.13.0 - 2026-06-30

### Summary
Introduced telemetry data structures, transaction-safe caches, and traversal metrics.

### Added
*   Telemetry collector structures.
*   Atomic cache manager to write index changes transactionally.

---

## v0.12.0 - 2026-06-30

### Summary
Introduced incremental watch-mode file updates and content-hashed caching.

### Added
*   File-content SHA256 hashing to manage the AST extraction cache.
*   File-system watch-mode support.

---

## v0.11.0 - 2026-06-30

### Summary
Optimized Graphenium for larger repositories and added pre-scan planning.

### Added
*   Progress bar and heartbeat logs for large-repository indexing.
*   Dry-run planning flag (`gm run --plan`) to inspect project scope.

---

## v0.10.0 - 2026-06-30

### Summary
Hardened deployment on Windows environments and added setup scripts.

### Added
*   PowerShell installer (`install.ps1`).
*   Automatic path normalization to resolve differences in Windows backslash paths.
*   Helpful warnings for workspace initialization.