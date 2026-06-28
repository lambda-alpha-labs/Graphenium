# Graphenium Architecture

Graphenium models a codebase as nodes, edges, evidence, and topology.

The architectural goal is not compiler-perfect static analysis. The goal is durable, provenance-aware repository memory that an AI agent can query before reading or changing code.

---

## Repository model

### Nodes

Nodes represent meaningful entities:

- files;
- modules;
- functions;
- methods;
- classes;
- structs;
- traits;
- documents;
- images;
- build targets;
- CI jobs;
- test cases;
- dependencies;
- architectural concepts.

Each node carries metadata such as label, qualified label, file type, source file, source location, confidence, provenance, and community ID.

### Edges

Edges are typed, directed relationships.

| Relation | Meaning | Source |
|---|---|---|
| `imports` | Module-level import/include | AST / resolver |
| `contains` | Module/class contains a symbol | AST |
| `method` | Method belongs to a class/type | AST |
| `calls` | Function calls another function | AST / resolver / semantic |
| `uses` | Cross-file usage dependency | AST / resolver / semantic |
| `inherits` | OOP inheritance | AST / semantic |
| `implements` | Interface/trait implementation | AST / semantic |
| `depends_on` | Conceptual dependency or package dependency | AST / repository extraction / semantic |
| `tests` | Test case or test target verifies a symbol or module | AST / repository extraction |
| `runs_in` | Build target or test target runs in a CI job | CI extraction |
| `rationale_for` | Document/comment explains code | Semantic / manual |

### Evidence

Evidence records why a graph fact exists. Evidence should let an agent decide whether a relationship is safe to plan against or merely useful as a lead.

Important evidence fields:

- confidence: `EXTRACTED`, `INFERRED`, or `AMBIGUOUS`;
- extractor: tree-sitter, resolver, LLM, telemetry, manual MCP write, or repository extractor;
- source span: file path, line, column, and excerpt when available;
- resolution status: resolved, unresolved, heuristic, inferred, or manually confirmed;
- timestamp or graph build identifier;
- stale-evidence flag when applicable.

### Topology

Graphenium analyzes the graph to surface:

- communities;
- hub nodes;
- shortest paths;
- safest paths;
- surprising cross-community connections;
- architectural focus paths;
- stale evidence;
- policy drift;
- change impact.

---

## Data pipeline

A typical run follows this sequence:

```text
repository scan
  -> ignore and sensitivity filtering
  -> language-specific extraction
  -> repository metadata extraction
  -> resolver pass
  -> optional semantic extraction
  -> graph assembly
  -> confidence and provenance annotation
  -> clustering and topology analysis
  -> quality report
  -> JSON, Markdown, and HTML export
  -> MCP serving or CLI query
```

The structural graph should be useful without semantic extraction. Semantic extraction adds concepts, rationale, and framework behavior that local analysis may miss.

---

## Internal source layout

```text
src/
  extract/     tree-sitter syntax extraction for supported languages plus repository config extraction
  model/       graph, node, edge, claim, hyperedge schemas, and graph metadata
  resolver.rs  cross-file import binding and target resolution
  trust.rs     evidence spans, claim model, resolution reporting
  harness.rs   trust-gate check for CI
  policy.rs    policy-based quality gates
  embed.rs     TF-based cosine similarities and Node2Vec structural embeddings
  cluster/     Louvain community detection, split/focus clustering, and cohesion scoring
  detect/      file classification, sensitive skipping, and corpus health checks
  analyze/     PageRank, chokepoints, dominators, reverse reachability, gates, and surprise edges
  serve/       MCP server, tool handlers, and mode-aware query traversal
  semantic/    async LLM batch extraction client and response parser
  telemetry/   OTEL trace import, EMA percentile estimation, regression compare, and hot paths
  export/      JSON export, HTML visualization, and schema export
  cache/       mtime manifest, semantic extraction cache, and graph snapshots
  watch.rs     file-system watcher with incremental patching and live blast-radius display
```

---

## Confidence model

| Confidence | Intended meaning | Planning behavior |
|---|---|---|
| `EXTRACTED` | Directly produced by structural extraction, resolver, telemetry import, or confirmed manual inspection | Safe to use as source-backed orientation, then verify implementation details |
| `INFERRED` | Produced by semantic extraction, heuristics, or incomplete behavioral evidence | Use as a lead; inspect before depending on it |
| `AMBIGUOUS` | Conflicting, unresolved, or low-confidence relationship | Do not plan against it until verified |

The confidence model is agent-facing. It should appear in CLI and MCP outputs whenever the result could influence a change plan.

---

## Feature flags

Build with only the languages you need:

```sh
cargo build --release --no-default-features --features lang-python,lang-rust
```

Available language features:

- `lang-python`
- `lang-js`
- `lang-ts`
- `lang-rust`
- `lang-go`
- `lang-java`
- `lang-c`
- `lang-cpp`
- `lang-csharp`

Ruby support may be distributed as part of the default build or behind a feature depending on the release configuration. Keep the README language table aligned with the actual Cargo feature set.

---

## Outputs

Graphenium writes outputs to `graphenium-out/` inside the analyzed directory.

| File | Purpose |
|---|---|
| `graph.json` | Machine-readable graph for `gm serve` and `gm query` |
| `GRAPH_REPORT.md` | Markdown architecture report |
| `graph.html` | Self-contained visual graph inspection page |
| `manifest.json` | mtime index for incremental updates |
| `cache/` | Per-file semantic extraction cache, SHA256 keyed |
| `quality.json` | Structured quality report with resolution ratio, ambiguity, and per-file stats |

---

## Current limitations

- Local graphs are structural, not fully behavioral.
- Dynamic dispatch, reflection, generated code, macros, dependency injection, and framework-specific execution paths may require semantic extraction, manual graph writes, or telemetry overlays.
- Label collisions can happen for common names such as `new`, `run`, `main`, and `mod`.
- Large corpora should exclude vendored dependencies and generated artifacts with `.grapheniumignore`.
- Telemetry is an overlay, not a profiler.
- Quality gates are only as useful as the graph and policy thresholds behind them.
- Semantic extraction can introduce plausible but unverified relationships; these should not be treated as source-backed unless confirmed.
- Graph output should guide source reading, not replace it.

---

## Design principle

Graphenium should make uncertainty visible.

The product is most valuable when an agent can say:

```text
I know these paths are source-backed.
I think these inferred links are likely but need inspection.
I will not rely on these ambiguous edges until I verify them.
```
