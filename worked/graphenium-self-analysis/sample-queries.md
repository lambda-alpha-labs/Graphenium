# Sample Queries: Graphenium Self-Analysis

The following examples illustrate the command-line output of Graphenium's structural query engine (`gm query`) running on Graphenium's own compiled Rust codebase (1,211 symbols, 3,083 boundaries, and 19 cohesive domains).

These queries demonstrate how Graphenium resolves cross-file call boundaries deterministically using AST-proven parsing rather than fuzzy semantic guessing.

---

## Query 1: "serve module handlers mcp"

This query targets the public interfaces and handlers of Graphenium's background server layer.

```text
$ gm query "serve module handlers mcp" --budget 2000

# Codebase Structural Query: serve module handlers mcp

Found 62 relevant nodes (of 1211)

## module_dependencies (code [domain 1])
File: src/serve/handlers.rs L1914:C5-L2048:C6
Relevance: score 3.00

AST-Proven Connections:
- module_dependencies ──► calls ──► is_ast_only [tree-sitter-stack-graphs:resolved]
- module_dependencies ──► calls ──► is_namespace_aggregation_node [tree-sitter-stack-graphs:resolved]
- module_dependencies ──► calls ──► node_data [tree-sitter-stack-graphs:resolved]
- module_dependencies ──► calls ──► edges_iter [tree-sitter-stack-graphs:resolved]
- module_dependencies ──► calls ──► GrapheniumServer::new [tree-sitter:heuristic]
- module_dependencies ──► method ──► GrapheniumServer [tree-sitter:resolved]

## handlers (code [domain 1])
File: src/serve/handlers.rs L1:C1-L4557:C1
Relevance: score 2.00

AST-Proven Connections:
- handlers ──► contains ──► GrapheniumServer [tree-sitter:resolved]
- handlers ──► contains ──► make_server [tree-sitter:resolved]
- handlers ──► contains ──► query_graph [tree-sitter:resolved]
- handlers ──► contains ──► get_node [tree-sitter:resolved]
- handlers ──► contains ──► get_neighbors [tree-sitter:resolved]
- handlers ──► contains ──► get_community [tree-sitter:resolved]
- handlers ──► contains ──► god_nodes [tree-sitter:resolved]
- handlers ──► contains ──► shortest_path [tree-sitter:resolved]
- handlers ──► contains ──► architecture_summary [tree-sitter:resolved]
- handlers ──► contains ──► blast_radius [tree-sitter:resolved]
- handlers ──► contains ──► verification_plan [tree-sitter:resolved]

Trust Profile: 1328 EXTRACTED, 1755 INFERRED, 0 AMBIGUOUS
```

### Architectural Analysis:
The cross-file call boundary `module_dependencies ──► node_data` is marked with explicit `[tree-sitter-stack-graphs:resolved]` provenance. This means the connection is compiler-proven: Graphenium has mathematically verified that the `module_dependencies` function physically executes a call to `node_data` in another file, establishing a high-trust boundary.

---

## Query 2: "graph build extraction"

This query targets the compilation and index assembly pipeline of Graphenium.

```text
$ gm query "graph build extraction" --safe --budget 2000

# Codebase Structural Query: graph build extraction

Found 36 relevant nodes (of 1211)

## build_from_extraction (code [domain 9])
File: src/build.rs L29:C1-L58:C2
Relevance: score 3.00

AST-Proven Connections:
- build_from_extraction ──► calls ──► len [tree-sitter-stack-graphs:resolved]
- build_from_extraction ──► calls ──► upsert_node [tree-sitter-stack-graphs:resolved]
- build_from_extraction ──► calls ──► contains_node [tree-sitter-stack-graphs:resolved]
- build_from_extraction ──► calls ──► basic_build [tree-sitter:heuristic]
- build_from_extraction ──► calls ──► build_merged [tree-sitter:heuristic]
- build_from_extraction ──► contains ──► build [tree-sitter:resolved]

## build (code [domain 9])
File: src/main.rs
Relevance: score 2.00

AST-Proven Connections:
- build ──► contains ──► build_from_extraction [tree-sitter:resolved]
- build ──► contains ──► build_merged [tree-sitter:resolved]
- build ──► contains ──► basic_build [tree-sitter:resolved]
- build ──► contains ──► BuildStats [tree-sitter:resolved]

## parse_extraction (code [domain 2])
File: src/semantic/parse.rs L29:C1-L31:C2
Relevance: score 2.00

AST-Proven Connections:
- parse_extraction ──► calls ──► extract_json [tree-sitter:heuristic]
- parse_extraction ──► calls ──► build_result [tree-sitter:heuristic]
- parse_extraction ──► calls ──► process_batch [tree-sitter-stack-graphs:resolved]
- parse_extraction ──► contains ──► parse [tree-sitter:resolved]
```

### Architectural Analysis:
Using the `--safe` flag restricts Graphenium's query engine strictly to `EXTRACTED` (AST-proven) dependencies. The returned connections, such as `build_from_extraction ──► upsert_node`, are confirmed compile-time relationships. This prevents an agent from planning edits based on speculative or unverified semantic assumptions.

---

## Query 3: "community detection"

This query targets Graphenium's Louvain domain clustering and cohesion scoring modules.

```text
$ gm query "community detection" --budget 2000

# Codebase Structural Query: community detection

Found 26 relevant nodes (of 1211)

## community_stats (code [domain 4])
File: src/cluster/cohesion.rs L38:C1-L105:C2
Relevance: score 1.00

AST-Proven Connections:
- community_stats ──► calls ──► focus_label [tree-sitter-stack-graphs:resolved]
- community_stats ──► calls ──► node_data [tree-sitter-stack-graphs:resolved]
- community_stats ──► calls ──► len [tree-sitter-stack-graphs:resolved]
- community_stats ──► calls ──► edges_with_endpoints [tree-sitter-stack-graphs:resolved]
- community_stats ──► calls ──► cluster [tree-sitter-stack-graphs:resolved]
- community_stats ──► contains ──► cohesion [tree-sitter:resolved]

## summarize_community (code [domain 1])
File: src/serve/handlers.rs L225:C5-L350:C6
Relevance: score 1.00

AST-Proven Connections:
- summarize_community ──► calls ──► len [tree-sitter-stack-graphs:resolved]
- summarize_community ──► calls ──► degree [tree-sitter-stack-graphs:resolved]
- summarize_community ──► calls ──► edges_with_endpoints [tree-sitter-stack-graphs:resolved]
- summarize_community ──► calls ──► nodes [tree-sitter-stack-graphs:resolved]
- summarize_community ──► calls ──► get_community [tree-sitter:heuristic]
- summarize_community ──► method ──► GrapheniumServer [tree-sitter:resolved]
```

### Architectural Analysis:
Multi-hop cross-file boundaries connecting the cohesion analyzer (`community_stats`) back to the core data models (`node_data` and `edges_with_endpoints` in `src/model/graph.rs`) are resolved with compiler-level certainty. Graphenium traces these dependencies across independent modules instantly, establishing an external verification boundary that prevents AI-generated code from quietly breaking decoupling contracts.
