# Changelog

This changelog summarizes key updates and engineering milestones for Graphenium, emphasizing relational codebase compilation, AI-agent write-safety, structural verification, and external governance gates.

---

## v0.26.2 - 2026-11-13

### Fixed
*   **AST Cache Invalidation Across Releases (`src/extract/mod.rs`):** The AST cache key now includes the crate version in addition to the extractor format version and the content hash. Previously the key was `"{format}-{sha256}"`, so upgrading gm replayed extraction results written by the previous release whenever a file's bytes were unchanged. A 0.25-era cache entry has no `depends_on_hypothesis` edges, which made `gm trust` report a conditional Lean theorem as `UNCONDITIONAL` even though 0.26 extracted the binder correctly. Bumped `AST_CACHE_FORMAT_VERSION` to `4` and added a regression test for the key layout.
*   **Transparent Type Aliases Are Not Hypotheses (`src/extract/lean.rs`):** Binder types that resolve to a project `def`/`abbrev` are no longer emitted as `depends_on_hypothesis`. An `abbrev` is ordinary data (`abbrev Q3Q5Int := ...`), not an unconstructed certificate; flagging it made every arithmetic `def` and every theorem using it `CONDITIONAL`. Structure/class/inductive/opaque binders, and unresolved capitalized project types, keep the existing behaviour.
*   **Explicit `--hypothesis` Scope (`src/analyze/lean_trust.rs`):** An explicitly flagged hypothesis is now reported as `direct_or_transitive: "direct"`, matching auto-detected binders. Previously both explicit paths wrote `"transitive"`, so machine consumers saw two different records for the same assumption on the same theorem. Also stopped the dependency-cone walk from considering the audited theorem itself as a candidate assumption.
*   **Bound-Column Indexes for Positive Joins (`src/analyze/query.rs`):** Positive atoms now use the same bound-mask hash indexes as negated atoms. A join such as `p(A,C) :- e(A,B), e(B,C)` previously scanned `e` once per outer tuple (quadratic: 40k edges took 84s and hit the 30s budget; 80k edges exceeded 300s). The 80k-edge case now completes in ~4s, and the 84k-edge atlas transfer graph completes a one-hop join in ~7s instead of timing out. Added a regression test with 1,000 nodes x 10 edges.
*   **Consistent `--out-dir` Base (`src/main.rs`):** An explicitly supplied relative `--out-dir`/`--cache-dir` for `run`/`watch` now resolves against the current working directory, matching `--graph` on read commands. The default `graphenium-out` remains root-relative. Previously `gm run lean --out-dir lean/out` silently wrote `lean/lean/out`.
*   **AST Cache Pruning (`src/cache/manager.rs`, `src/main.rs`):** `gm run` removes AST cache entries whose key prefix belongs to another gm/extractor version. They can never be reused after the versioned-key change and previously accumulated on every upgrade.
*   **Datalog Single-Atom Overhead (`src/analyze/query.rs`):** Positive-join indexes are now built only when an atom actually has bound columns, and fully-ground negated atoms keep using the existing `full` tuple set instead of building a duplicate row index. The bound-column optimization from this release had pushed the 200k-edge stratified-negation regression from under 1s to ~1.23s; the single-atom path no longer clones the relation or pays for an unused index.

### Tests
*   `ast_cache_key_is_versioned`, `ignores_transparent_alias_binders`, `explicit_hypothesis_is_reported_as_direct`, plus the existing binder/hypothesis and trust fixtures.

---

## v0.26.1 - 2026-11-13

### Fixed
*   **Lean Trust Axiom Deduplication (`src/extract/lean_axioms.rs`, `src/analyze/lean_trust.rs`):** Axioms are now collected by canonical key before rendering, so `Quot.sound` and `quot_sound` (the normalized node ID) can no longer both appear in one report. Kernel labels are always rendered exactly as Lean reports them: `propext`, `Classical.choice`, `Quot.sound`. Axiom nodes/edges ingested from `lean-axioms.v1` reports use the same canonical spelling.
*   **`gm trust --json` Output Hygiene (`src/main.rs`, `src/error.rs`):** Machine-readable trust runs now print exactly one `lean-trust.v1` JSON document on stdout, including when evaluation cannot start (missing theorem, unreadable report). Failure reasons are carried in `failure_reasons` fields, and the CLI exits non-zero without writing `[graphenium] error:` lines to stderr.
*   **Structure-Parameter Auto-Detection (`src/extract/lean.rs`, `src/extract/lean_axioms.rs`):** The Lean extractor now scans multi-line `theorem`/`lemma`/`def` signatures up to `:=`, `|`, or `where`, and tracks active `section`/`namespace` `variable (...)` / `variable [...]` declarations. Cross-file hypothesis targets preserve their raw Lean spelling (`EdgeAtlasCertificate`) so the resolver can bind them to the real declaration instead of dropping a dangling normalized ID. `lean-axioms.v1` ingestion also flattens elaborated `ConstantInfo.type` (`forallE`) binders under `binders` or `type`. Typeclass constraints (`[CommRing R]`) and primitive data (`(n : Nat)`) are deliberately excluded from `CONDITIONAL` status.
*   **Datalog Negation at Scale (`src/analyze/query.rs`):** Frozen lower-stratum relations now build bound-mask hash indexes on demand, not just per-column indexes, so partially bound `not p(...)` lookups remain O(1) regardless of which column joins. Positive atoms bind all variables before `not` and `!=` / `/=` constraints are evaluated; unbound named variables in `not` still fail range-restriction validation. The 200k-edge regression test completes well inside the 1s release budget.

### Added
*   **`gm query --datalog --show-derived`:** Optionally include every materialized IDB relation (`derived[]`) in `datalog.v1` JSON or as extra text sections. Goal-only output remains the default.

### Changed
*   **Goal-Only Datalog Output (breaking, introduced in 0.26.0):** `gm query --datalog` now emits only the rows projected by explicit `?-` goals (plus EDB/IDB facts needed to answer them). Scripts written for 0.25 that relied on a single conjoined goal to print every derived IDB relation must either split the output into explicit labelled goals (`?- [bases] … .`, `?- [pairs] … .`) or pass `--show-derived`.
*   **Version:** Bumped to `0.26.1`.

### Tests
*   Canonical `Quot.sound` deduplication, cross-file/multi-line/section-variable hypothesis detection, `[CommRing R]` / `(n : Nat)` guardrail regression, JSON stderr hygiene, `--show-derived`/goal-only output, and the existing 200k-edge negation benchmark.

---

## v0.26.0 - 2026-11-13

### Added
*   **Hypothesis-Aware Lean Trust (`src/extract/lean.rs`, `src/analyze/lean_trust.rs`):** The Lean extractor now emits `depends_on_hypothesis` edges for theorem/lemma/def binder types, so a theorem of the form `theorem T (h : Cert) : Goal` is reported as `CONDITIONAL` instead of passing on kernel axioms alone. `gm trust` gained `--hypothesis`, `--allow-hypothesis`, `--warn-structure-params`, transitive assumption propagation, and explicit `UNCONDITIONAL` / `CONDITIONAL` / `TAINTED` statuses in `lean-trust.v1` JSON. `.graphenium/policy.json` can declare `hypotheses`, `allow_hypotheses`, `allow_axioms`, and `forbid_native_decide` either at the top level or under a `trust` block.
*   **Stratified Datalog Negation (`src/analyze/query.rs`, `src/analyze/query/stdlib.dl`):** Rules are partitioned into SCC-ordered strata before evaluation; unstratifiable negative recursion is rejected explicitly, and every negated relation is frozen and indexed with a full-tuple set plus per-column hash indexes before higher strata query it. Added `depends_on_hypothesis/2`, `inherits_hypothesis/2`, and `conditional_theorem/1` stdlib predicates.
*   **Lazy EDB Materialization:** `run_datalog` now loads only the EDB relations reachable from the query goals, and Datalog values share interned `Arc<str>` allocations.

### Changed
*   **Version:** Bumped to `0.26.0`.
*   **Lean Trust Surfacing:** `gm check`, `verify_changes`, and `get_graph_info` report conditional theorem counts and assumption names when Lean data is loaded. The MCP surface remains exactly 12 tools.

### Tests
*   End-to-end conditional/allowlisted/transitive hypothesis fixtures, unstratifiable-negation and range-restriction tests, and a 200k-edge stratified `not` benchmark (1s release budget, relaxed debug regression bound).

---

## v0.25.0 - 2026-11-13

### Added
*   **Datalog Ergonomics (`src/analyze/query.rs`):** Added `!=` / `/=` inequality constraints with grounding validation, multiple labelled `?-` queries sharing one EDB/fixpoint, and a machine-readable `datalog.v1` report via `gm query --datalog ... --json` and the `run_datalog` MCP `format = "json"` parameter.
*   **Portable Workspaces (`src/detect/path.rs`, `src/main.rs`, `src/watch.rs`):** Added `--out-dir` to `run`, `watch`, and read commands, plus `--cache-dir` to `run`/`watch`. Custom output/cache directories are excluded from the corpus walk, and persisted node/edge/manifest/cache paths are repo-relative. Bumped the AST cache format to `3`.
*   **CI Read-Only Hardening (`src/cache/manifest.rs`):** Manifests now record SHA-256 and byte length alongside mtime, with `is_changed_fast` (content-hash fallback for fresh checkouts) and `is_changed_strict`. Added `gm check --verify-fresh`, `gm gate --verify-fresh`, and `gm run --no-cache-write`.
*   **Semantic Lean Trust Auditing (`src/extract/lean_axioms.rs`, `src/analyze/lean_trust.rs`):** Ingest raw `#print axioms` output or `lean-axioms.v1` JSON, classify standard kernel / `sorryAx` / `native_decide` / external axioms, and audit theorem dependency cones with `gm trust`. Added `gm run --lean-axioms` and opt-in `--run-lean-axioms`, plus Datalog predicates `uses_axiom/2`, `has_untrusted_axiom/1`, `has_native_decide/1`, and `untrusted_theorem/1`.
*   **MCP Surface:** Folded Lean trust counts into `get_graph_info` and a `format` parameter into `run_datalog`; the surface remains exactly 12 tools.

### Changed
*   **Documentation:** Updated the command reference, architecture guide, trust model, MCP tool reference, getting-started guide, CI governance guide, and agent skill for the new CLI flags, Datalog syntax, portable workspaces, and semantic Lean auditing.
*   **Version:** Bumped to `0.25.0`.

### Tests
*   Added Datalog inequality/multi-query/JSON tests, portable-workspace and read-only CI tests, manifest freshness tests, and end-to-end `gm trust` fixtures.
*   `cargo test`: 464 tests pass (439 library, 15 Datalog, 2 Lean trust, 4 planning, 3 portable workspace, 1 trust benchmark).

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