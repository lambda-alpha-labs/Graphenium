# CLI Command Reference

The Graphenium command-line interface (`gm`) serves as both a relational codebase compiler and an automated gatekeeper. It compiles codebases into provenance-tagged relational indexes, runs indexed semi-naive Datalog invariants, and executes Virtual AST pre-flight and post-edit containment checks.

---

## Command Map

| Command | Primary Purpose |
|---|---|
| [`gm init`](#gm-init) | Initialize a workspace and configure `.grapheniumignore`. |
| [`gm run`](#gm-run) | Compile the codebase's AST symbols, cross-file imports, and namespaces. |
| [`gm query`](#gm-query) | Execute structural searches, path traces, or declarative Datalog constraints. |
| [`gm check`](#gm-check) | Enforce pre-flight architectural policies and post-facto scope audits. |
| [`gm serve`](#gm-serve) | Initialize the background MCP server on stdio. |
| [`gm watch`](#gm-watch) | Re-compile the index in real-time as physical file modifications occur. |
| [`gm doctor`](#gm-doctor) | Diagnose index integrity, schema compliance, and resolution health. |
| [`gm diff`](#gm-diff) | Compare structural snapshots to identify additions, removals, and drift. |
| [`gm explain`](#gm-explain) | Generate a pre-edit structural orientation summary for a target symbol. |
| [`gm setup`](#gm-setup) | Generate workspace MCP configurations for IDEs and assistants. |
| [`gm graph`](#gm-graph) | Subcommands to inspect compiled metadata, build targets, and test mappings. |
| [`gm snapshot`](#gm-snapshot) | Create and list indexed codebase snapshots. |
| [`gm trust`](#gm-trust) | Audit Lean 4 theorem trust boundaries, inherited axioms, and binder hypotheses. |
| [`gm gate`](#gm-gate) | Execute automated containment gates over index differences. |

---

## `gm init`
Initializes Graphenium's configuration inside the target directory and generates a default `.grapheniumignore` file to filter out build artifacts and external vendor code.

```sh
gm init [path]
```

### Arguments:
*   `path` (string, optional, default: `.`) — The path to the repository root directory.

---

## `gm run`
Parses codebase source files, extracts declarations (classes, structs, functions, methods, interfaces, types), resolves cross-file import paths and calls, partitions cohesive domains (communities), and writes the structural index. C# `.sln` / `.csproj` files are discovered recursively, and Lean 4 `.lean` files are indexed with proof-gap auditing.

```sh
gm run [path] [flags]
```

### Arguments:
*   `path` (string, optional, default: `.`) — The repository directory to analyze.

### Flags:
*   `--mode <mode>` (string, default: `standard`) — Extraction depth: `standard` for fast AST + Stack Graphs or `deep` for aggressive inference.
*   `--no-semantic` (bool) — Explicitly skips remote LLM passes, guaranteeing 100% offline, local execution.
*   `--no-viz` (bool) — Disables the generation of the local `graph.html` visualization file.
*   `--no-report` (bool) — Disables compilation of the local `GRAPH_REPORT.md` summary.
*   `--exclude-dirs <dirs>` (string) — Comma-separated folder patterns to skip during analysis.
*   `--plan` (bool) — Executes a dry-run analysis, reporting scanned file boundaries without writing the index.
*   `--update` (bool) — Activates incremental patching, compiling only changed or affected source files.
*   `--provider <provider>` (string, default: `anthropic`) — AI provider: `anthropic`, `openai`, `openrouter`, `deepseek`, or `openai-compatible`.
*   `--api-key <key>` (string) — API key (overrides the provider-specific env var).
*   `--api-base <url>` (string) — API base URL for `openai-compatible` provider.
*   `--model <model>` (string) — Model to use (defaults to provider-specific default).
*   `--out-dir <path>` (string, default: `graphenium-out`) — Output directory for `graph.json`, `manifest.json`, `GRAPH_REPORT.md`, and the default cache. The default `graphenium-out` is placed inside `path`; an explicitly supplied relative path resolves against the current working directory, matching `--graph` on read commands.
*   `--cache-dir <path>` (string, default: `<out-dir>/cache`) — Extraction cache directory. Configured output/cache directories are excluded from the corpus walk automatically.
*   `--no-cache-write` (bool) — Run without writing AST/semantic caches (ephemeral CI).
*   `--lean-axioms <path>` (string) — Ingest a `#print axioms` report (raw text or `lean-axioms.v1` JSON) and bake `depends_on_axiom`, `unverified_sorry`, and `unverified_native_decide` edges into the graph.
*   `--run-lean-axioms <lean-file>` (string) — Opt in to running `lake env lean <lean-file>` and ingesting its `#print axioms` output. Graphenium never invokes Lean unless this flag is passed.

---

## `gm query`
Executes structural queries against Graphenium's local index. Supports lexical searches, topological path searches, and first-order Datalog queries.

```sh
gm query "<query>" [flags]
```

### Arguments:
*   `query` (string, required) — The keyword query or declarative Datalog program.

### Flags:
*   `--mode <mode>` (string, default: `lexical`) — Search strategy:
    *   `lexical`: Relevance matching against symbol names, qualified paths, and source files.
    *   `structural`: Topological distance sorting radiating outward from matched seed nodes.
    *   `hybrid`: Combined score weighting lexical relevance (60%) and structural distance (40%).
*   `--datalog <program>` (string) — Executes a declarative Datalog program against Graphenium's relational EDB. Standard library rules (`stdlib.dl`) are automatically merged pre-flight. Evaluation uses goal-directed pruning, stratified negation over SCC-ordered strata, semi-naive delta relations, and full-tuple plus bound-mask hash indexes for both positive joins and negation, with early stopping for fully-ground existence goals. Unstratifiable negative recursion is rejected explicitly. Solver execution is bounded to 1,000 inference steps; base EDB relations are accepted up to 10 million facts. Prefer bound queries (`?- calls('seed', X).`) over unbounded predicates (`?- is_hub(X).`) on large graphs.
    *   **Output is goal-only by default (breaking change in 0.26.0):** only the rows projected by explicit `?-` goals are returned. A single conjoined goal no longer prints every derived IDB relation. Emit one labelled goal per output (`?- [bases] basis(B).`, `?- [pairs] mub_pair(B1, B2).`) or pass `--show-derived`.
    *   **Negation:** `not p(...)` is evaluated only after all positive atoms have bound their variables; frozen lower-stratum relations are indexed by full tuple and by the exact set of bound columns, so negation does not scan large relations. A named variable that appears only in a negated atom is rejected by range-restriction validation.
    *   **Inequality:** rule and query bodies accept `A != B` (and `A /= B`). Variables must be bound by a positive body atom; an ungrounded inequality is a validation error.
    *   **Multiple queries:** a program may contain several `?-` blocks, optionally labelled (`?- [proof_gaps] has_proof_gap(X).`). The EDB and fixpoint are materialized once and shared by every query.
    *   Built-in predicates include `calls_transitive`, `depends_transitive`, `circular_dependency`, `bypasses_layer`, `contains_transitive`, `aliased_container_call`, `unreachable_from`, `has_proof_gap`, `tainted_by_proof_gap`, `uses_axiom`, `has_untrusted_axiom`, `has_native_decide`, `untrusted_theorem`, `depends_on_hypothesis`, `inherits_hypothesis`, and `conditional_theorem`.
*   `--show-derived` (bool) — With `--datalog`, also include every materialized IDB relation (JSON `derived[]` or extra text sections). Default is goal-only.
*   `--budget <tokens>` (integer, default: `2000`) — Limits Graphenium's printed output to remain within the specified context-token limit.
*   `--dfs` (bool) — Uses Depth-First Search for structural tracing (default: BFS).
*   `--safe` (bool) — Restricts query-tracing strictly to AST-proven (`EXTRACTED`) dependencies.
*   `--graph <path>` (string, default: `<out-dir>/graph.json`) — Overrides the source index file location.
*   `--out-dir <path>` (string, default: `graphenium-out`) — Resolves the default graph path.
*   `--json` (bool) — With `--datalog`, emits a `datalog.v1` report (`schema_version`, `edb_facts`, per-query `name`, `query`, `variables`, `rows`, `row_count`, `steps_used`, `truncated`, and optional `derived[]` with `--show-derived`). Without `--datalog`, outputs lexical/structural results as JSON.
*   `--path-prefix <path>` (string) — Restricts results to symbols located inside the specified directory path.
*   `--exclude-path <path>` (string) — Filters out results located inside the specified directory path.
*   `--generated-code-mode <mode>` (string, default: `include`) — How to handle files classified as generated: `include`, `exclude`, or `only`.
*   `--ast-only-tuning <mode>` (string, default: `auto`) — AST-only tuning: `auto`, `on`, or `off`.

---

## `gm check`
Enforces global resolution thresholds and validates planning workspaces. This is Graphenium's primary CI/CD and pre-commit containment hook. When the graph contains Lean axiom or hypothesis facts, `gm check` also prints the `UNCONDITIONAL` / `CONDITIONAL` / `TAINTED` theorem summary.

```sh
gm check [flags]
```

### Flags:
*   `--graph <path>` (string, default: `<out-dir>/graph.json`) — Path to the codebase index file.
*   `--out-dir <path>` (string, default: `graphenium-out`) — Resolves the default graph and manifest paths.
*   `--verify-fresh` (bool) — Read-only CI drift check: hashes current corpus files against `<out-dir>/manifest.json` and fails with exit code `1`, listing modified/added/deleted repo-relative paths. Mtime-only changes (fresh checkout) are ignored. Never writes to disk.
*   `--min-resolution <pct>` (float, default: `80.0`) — Fails the check if Graphenium's AST import resolution ratio drops below the target percentage.
*   `--max-ambiguous <count>` (integer, default: `10`) — Fails the check if Graphenium detects more than `<count>` unresolved name collisions. Lean 4 `sorry` / `admit` proof gaps are emitted as `AMBIGUOUS` edges and count toward this threshold.
*   `--strict` (bool) — Fails the check on any index, parser, or configuration warnings.
*   `--plan <id>` (string) — Executes a double-gate verification for a planning workspace: runs pre-flight policy validation, followed by a post-facto audit of implemented/missing nodes, planned dependency edges, and unplanned modified files.
*   `--delta` (bool) — Activates **Topological Delta Gating**: in-memory modularity delta (ΔQ) and surprise edge analysis on a planning workspace. Requires `--plan`.
*   `--mod-tolerance <val>` (float, default: `-0.02`) — Maximum acceptable modularity decay when `--delta` is set.
*   `--surprise-threshold <val>` (float, default: `5.0`) — Surprise score above which a planned edge is flagged when `--delta` is set.

### Modes:
*   **Baseline quality gate** (default): `--min-resolution` / `--max-ambiguous` thresholds.
*   **Plan compliance gate**: `--plan <id>` — policy validation + node/edge/scope-creep audit.
*   **Topological delta gate**: `--delta --plan <id>` — modularity delta + surprise profiling. Exits non-zero on failure.

```sh
# Topological entropy gate for a planning workspace
gm check --delta --plan refactor-session-handling

# Custom tolerance thresholds
gm check --delta --plan my-plan --mod-tolerance -0.01 --surprise-threshold 4.0
```

### Example Output (Pass)

```text
## Topological Delta Gate: refactor-session-handling
Modularity Delta (ΔQ): 0.0012 (Baseline: 0.3841 → Virtual: 0.3853)

[PASS] Plan respects current modularity patterns.
```

### Example Output (Failure)

When the gate fails, `gm check --delta` exits with code `1`. Use the structured output to diagnose the rejection:

```text
## Topological Delta Gate: refactor-session-handling
Modularity Delta (ΔQ): -0.0341 (Baseline: 0.3841 → Virtual: 0.3500)

[FAIL] High-Surprise Connections Proposed:
  - plan:UserView ──► src/db/helper (Score: 6.5)
    Reason(s): cross-community, peripheral→hub

Structural Drift Events Detected:
  - [Cross-boundary edges]: cross-boundary edges increased: 42 → 45

[FAIL] Plan violates topological entropy boundaries.
```

**Interpreting failures:**
*   **Negative ΔQ** — the proposed plan degrades Louvain modularity beyond `--mod-tolerance`. Route dependencies through existing intermediate services.
*   **High-surprise edges** — planned connections score above `--surprise-threshold` (e.g., `cross-community`, `peripheral→hub`). These indicate architectural shortcuts, not regex folder violations.
*   **Drift events** — informational warnings about community boundary changes triggered by the plan.

---

## `gm serve`
Starts Graphenium's Model Context Protocol (MCP) server, allowing AI coding assistants to invoke codebase tools natively via JSON-RPC.

```sh
gm serve [flags]
```

### Flags:
*   `--graph <path>` (string, default: `<out-dir>/graph.json`) — Path to Graphenium's compiled codebase index.
*   `--out-dir <path>` (string, default: `graphenium-out`) — Resolves the default graph path.
*   `--watch` (bool) — Monitors the compiled index file and hot-reloads the server's state in-memory as modifications occur.

---

## `gm watch`
Watches filesystem sources in real-time, executing incremental AST patching and boundary re-compilation as code edits occur.

```sh
gm watch <path> [flags]
```

### Arguments:
*   `path` (string, required) — The directory path to monitor.

### Flags:
*   `--debounce <secs>` (float, default: `3.0`) — Wait time in seconds after the last file modification before compiling changes.
*   `--incremental` (bool, default: `true`) — Restricts re-extraction to modified files and their direct importers, skipping full repository re-scans.
*   `--impact` (bool) — Prints the structural blast radius (affected callers, dependent domains) immediately after compiling an update.
*   `--out-dir <path>` (string, default: `graphenium-out`) — Output directory. The default `graphenium-out` is placed inside `<path>`; an explicitly supplied relative path resolves against the current working directory, matching `--graph` on read commands.
*   `--cache-dir <path>` (string, default: `<out-dir>/cache`) — Extraction cache directory.

---

## `gm doctor`
Diagnoses the health of Graphenium's environment, compilation boundaries, index schema compliance, and resolution ratios.

```sh
gm doctor [flags]
```

### Flags:
*   `--graph <path>` (string, default: `<out-dir>/graph.json`) — Path of Graphenium's compiled codebase index.
*   `--out-dir <path>` (string, default: `graphenium-out`) — Resolves the default graph path.
*   `--schema` (bool) — Displays graph.json schema version and compiler versions.
*   `--resolution` (bool) — Outputs a detailed resolution report (imports, calls, methods, and evidence freshness status).
*   `--repository` (bool) — Show repository info from graph metadata.

---

## `gm diff`
Compares two index files to audit structural modifications (additions, removals, and domain drifts).

```sh
gm diff --before <path> --after <path> [flags]
```

### Flags:
*   `--before <path>` (string, required) — Baseline index snapshot.
*   `--after <path>` (string, required) — Post-change index snapshot.
*   `--impact` (bool) — Evaluates the downstream transitive impact of all added and removed symbols.
*   `--review-plan` (bool) — Generates a risk-sorted review plan for PR reviews.

---

## `gm explain`
Provides a structural orientation summary for a target symbol. Explains where the symbol lives, what is compiler-proven to call it, and what dependencies it imports.

```sh
gm explain <symbol> [flags]
```

### Arguments:
*   `symbol` (string, required) — The class, method, or function name to explain.

### Flags:
*   `--graph <path>` (string, default: `<out-dir>/graph.json`) — Path of Graphenium's codebase index.
*   `--out-dir <path>` (string, default: `graphenium-out`) — Resolves the default graph path.

---

## `gm setup`
Generates MCP server configuration snippets for target developer tools.

```sh
gm setup <target> [flags]
```

### Arguments:
*   `target` (string, required) — The environment to configure: `claude`, `cursor`, `codewhale`.

### Flags:
*   `--gm-path <path>` (string) — Explicitly overrides the absolute path to Graphenium's compiled CLI binary.
*   `--graph <path>` (string) — Explicitly overrides the target index file path.

---

## `gm graph`
Subcommand suite to inspect index metadata, compilation mappings, and build targets.

```sh
gm graph <subcommand>
```

### Subcommands:
*   `schema` — Displays the index schema version and build metadata.
*   `migrate` — Migrates older index files to the current `0.2.0` schema and records the active Graphenium version.
*   `build-map` — Prints the extracted build-to-source mapping, including recursively discovered C# projects.
*   `test-map` — Prints the extracted test-to-source validation mapping.

---

## `gm snapshot`
Manages index snapshots, facilitating change gating and manual structural reviews.

```sh
gm snapshot <subcommand>
```

### Subcommands:
*   `create <name>` — Generates a named snapshot of Graphenium's current index under `graphenium-snapshots/`.
*   `list` — Lists all available snapshots, timestamps, and compiled sizes.

---

## `gm trust`
Audits Lean 4 theorem trust boundaries. Resolves the transitive dependency cone of a theorem and reports inherited kernel axioms, proof gaps (`sorryAx`), native-decision shortcuts (`Lean.ofReduceBool` / `Lean.trustCompiler`), external user axioms, and hypothesis/structure parameters that make a theorem conditional.

```sh
gm trust [flags]
```

### Flags:
*   `--graph <path>` (string, default: `<out-dir>/graph.json`) — Index to audit.
*   `--out-dir <path>` (string, default: `graphenium-out`) — Resolves the default graph path.
*   `--axioms <path>` (string) — Optional `#print axioms` report (raw text or `lean-axioms.v1` JSON). Omit it when axiom edges were already baked in with `gm run --lean-axioms`.
*   `--theorem <name>` (string) — Audit one theorem (qualified Lean name or node ID). Omit to audit every declaration with axiom, proof-gap, or hypothesis data.
*   `--forbid-native-decide` (bool) — Fail when `Lean.ofReduceBool` / `Lean.trustCompiler` is inherited.
*   `--allow-axiom <name>` (repeatable) — Allow a specific external axiom. Standard kernel axioms are always allowed; `sorryAx` is never allowed.
*   `--hypothesis <name>` (repeatable) — Explicitly flag a hypothesis by declaration label, qualified name, or node ID. Useful when a binder is hidden behind notation or generated code. Explicit flags report `direct_or_transitive: "direct"`, matching auto-detected binders.
*   `--allow-hypothesis <name>` (repeatable) — Accept an inherited hypothesis. Without this (or a policy allowance), a `CONDITIONAL` theorem exits non-zero.
*   `--warn-structure-params` (bool) — Warn about unconstructed `structure` / `class` / `opaque` parameters that are inherited without an explicit allowance.
*   `--json` (bool) — Emit a `lean-trust.v1` report. stdout contains exactly one valid JSON document (failure reasons appear in `failure_reasons` fields); stderr stays clean on gate failures and the exit code communicates pass/fail.

Each audited theorem receives one of three statuses:

| Status | Meaning |
|---|---|
| `UNCONDITIONAL` | Only standard kernel axioms; no proof gaps, forbidden native decisions, external axioms, or hypotheses. |
| `CONDITIONAL` | Kernel-clean but dependent on one or more binder hypotheses/structure parameters. |
| `TAINTED` | Inherits `sorryAx`, a forbidden `native_decide` shortcut, or a disallowed external axiom. |

Exits non-zero for `TAINTED`, or for `CONDITIONAL` unless every inherited hypothesis is allowed.

Binder detection is automatic for single- and multi-line signatures, active `section`/`namespace` `variable` declarations, cross-file certificate types, and elaborated `forallE` binders in `lean-axioms.v1` JSON. Transparent `def`/`abbrev` aliases, square-bracket typeclass constraints (`[CommRing R]`), and primitive data parameters (`(n : Nat)`) are ignored. Kernel axiom labels are canonicalized, so reports never contain both `Quot.sound` and `quot_sound`.

Hypotheses can also be configured in `.graphenium/policy.json`:

```json
{
  "rules": [],
  "trust": {
    "hypotheses": ["EdgeAtlasCertificate"],
    "allow_hypotheses": ["ReviewedCertificate"],
    "allow_axioms": ["MyReviewedAxiom"],
    "forbid_native_decide": true
  }
}
```

The same keys are accepted at the top level of the policy file. CLI flags layer on top of policy values.

```sh
gm trust --axioms axioms.json --theorem MUB6.no_four_mubs_dim6 --forbid-native-decide --json
gm trust --theorem NoFourMUBs.EdgeXFinal.no_four_mubs_seed --allow-hypothesis EdgeAtlasCertificate --json
```

---

## `gm gate`
Executes automated PR containment gates over index differences, or runs the standard trust check when no diff is supplied.

```sh
gm gate [--diff <before> <after>]
```

### Flags:
*   `--diff <before> <after>` (strings, optional) — The baseline and post-change index files to gate. The command reports architectural drift and exits non-zero if any `Critical` drift event is detected.
*   `--out-dir <path>` (string, default: `graphenium-out`) — Resolves the default graph/manifest paths.
*   `--verify-fresh` (bool) — Reads `<out-dir>/manifest.json`, hashes the current corpus, and fails on drift without mutating disk.
*   When `--diff` is omitted, `gm gate` runs the standard `gm check` gate against `<out-dir>/graph.json` with `--min-resolution 80`, `--max-ambiguous 10`, and `--strict` semantics.

### Examples:
```sh
# Compare two compiled snapshots
gm gate --diff graphenium-snapshots/before-change.json graphenium-out/graph.json

# Run the default trust gate on the current index
gm gate
```
