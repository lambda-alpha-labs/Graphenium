# Graphenium Architecture

Graphenium models a codebase as nodes, edges, evidence, and topology.

## Repository model

### Nodes

Nodes represent meaningful entities:

- functions
- methods
- classes
- modules
- structs
- traits
- documents
- images
- build targets
- CI jobs
- test cases
- dependencies
- architectural concepts

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
| `rationale_for` | Document/comment explains code | Semantic |

### Topology

Graphenium analyzes the graph to surface:

- communities
- hub nodes
- shortest paths
- safest paths
- surprising cross-community connections
- architectural focus paths
- stale evidence
- policy drift
- change impact

## Internal source layout

```text
src/
  extract/     tree-sitter syntax extraction for 9 languages plus repository config extraction
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

## Current limitations

- Local graphs are structural, not fully behavioral.
- Dynamic dispatch, reflection, generated code, and framework-specific execution paths may require semantic extraction, manual graph writes, or telemetry overlays.
- Label collisions can happen for common names such as `new`, `run`, and `mod`.
- Large corpora should exclude vendored dependencies with `.grapheniumignore`.
- Telemetry is an overlay, not a profiler.
- Quality gates are only as useful as the graph and policy thresholds behind them.

