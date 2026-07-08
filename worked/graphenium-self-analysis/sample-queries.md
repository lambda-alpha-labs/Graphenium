# Sample queries: Graphenium self-analysis

Real output from `gm query` on Graphenium's own AST + Stack Graphs graph
(1,211 nodes, 3,083 edges, 19 communities). Generated with gm 0.18.0.

## Query: "serve module handlers mcp"

```
# Graph Query: serve module handlers mcp

Found 62 relevant nodes (of 1211)

## module_dependencies (code [community 1])
File: src/serve/handlers.rs L1914:C5-L2048:C6
Match: score 3.00

Connections:
- module_dependencies `calls` is_ast_only [tree-sitter-stack-graphs:resolved]
- module_dependencies `calls` is_namespace_aggregation_node [tree-sitter-stack-graphs:resolved]
- module_dependencies `calls` node_data [tree-sitter-stack-graphs:resolved]
- module_dependencies `calls` edges_iter [tree-sitter-stack-graphs:resolved]
- module_dependencies `calls` GrapheniumServer::new [tree-sitter:heuristic]
- module_dependencies `method` GrapheniumServer [tree-sitter:resolved]

## handlers (code [community 1])
File: src/serve/handlers.rs L1:C1-L4557:C1
Match: score 2.00

Connections:
- handlers `contains` GrapheniumServer [tree-sitter:resolved]
- handlers `contains` make_server [tree-sitter:resolved]
- handlers `contains` query_graph [tree-sitter:resolved]
- handlers `contains` get_node [tree-sitter:resolved]
- handlers `contains` get_neighbors [tree-sitter:resolved]
- handlers `contains` get_community [tree-sitter:resolved]
- handlers `contains` god_nodes [tree-sitter:resolved]
- handlers `contains` shortest_path [tree-sitter:resolved]
- handlers `contains` architecture_summary [tree-sitter:resolved]
- handlers `contains` blast_radius [tree-sitter:resolved]
- handlers `contains` verification_plan [tree-sitter:resolved]

Trust Profile: 1328 EXTRACTED, 1755 INFERRED, 0 AMBIGUOUS
```

**What's new in v0.18.0:** Cross-file `calls` edges like
`module_dependencies → is_ast_only` carry
`[tree-sitter-stack-graphs:resolved]` provenance — real resolution,
not heuristic guesses.

## Query: "graph build extraction"

```
# Graph Query: graph build extraction

Found 36 relevant nodes (of 1211)

## build_from_extraction (code [community 9])
File: src/build.rs L29:C1-L58:C2
Match: score 3.00

Connections:
- build_from_extraction `calls` len [tree-sitter-stack-graphs:resolved]
- build_from_extraction `calls` upsert_node [tree-sitter-stack-graphs:resolved]
- build_from_extraction `calls` contains_node [tree-sitter-stack-graphs:resolved]
- build_from_extraction `calls` basic_build [tree-sitter:heuristic]
- build_from_extraction `calls` build_merged [tree-sitter:heuristic]
- build_from_extraction `contains` build [tree-sitter:resolved]

## build (code [community 9])
File: src/main.rs
Match: score 2.00

Connections:
- build `contains` build_from_extraction [tree-sitter:resolved]
- build `contains` build_merged [tree-sitter:resolved]
- build `contains` basic_build [tree-sitter:resolved]
- build `contains` BuildStats [tree-sitter:resolved]

## parse_extraction (code [community 2])
File: src/semantic/parse.rs L29:C1-L31:C2
Match: score 2.00

Connections:
- parse_extraction `calls` extract_json [tree-sitter:heuristic]
- parse_extraction `calls` build_result [tree-sitter:heuristic]
- parse_extraction `calls` process_batch [tree-sitter-stack-graphs:resolved]
- parse_extraction `contains` parse [tree-sitter:resolved]
```

**What's new:** The `build_from_extraction → len`, `→ upsert_node`, and
`→ contains_node` edges are now resolved cross-file, showing
that the build pipeline directly invokes model methods on `GrapheniumGraph`.

## Query: "community detection"

```
# Graph Query: community detection

Found 26 relevant nodes (of 1211)

## community_stats (code [community 4])
File: src/cluster/cohesion.rs L38:C1-L105:C2
Match: score 1.00

Connections:
- community_stats `calls` focus_label [tree-sitter-stack-graphs:resolved]
- community_stats `calls` node_data [tree-sitter-stack-graphs:resolved]
- community_stats `calls` len [tree-sitter-stack-graphs:resolved]
- community_stats `calls` edges_with_endpoints [tree-sitter-stack-graphs:resolved]
- community_stats `calls` cluster [tree-sitter-stack-graphs:resolved]
- community_stats `contains` cohesion [tree-sitter:resolved]

## summarize_community (code [community 1])
File: src/serve/handlers.rs L225:C5-L350:C6
Match: score 1.00

Connections:
- summarize_community `calls` len [tree-sitter-stack-graphs:resolved]
- summarize_community `calls` degree [tree-sitter-stack-graphs:resolved]
- summarize_community `calls` edges_with_endpoints [tree-sitter-stack-graphs:resolved]
- summarize_community `calls` nodes [tree-sitter-stack-graphs:resolved]
- summarize_community `calls` get_community [tree-sitter:heuristic]
- summarize_community `method` GrapheniumServer [tree-sitter:resolved]

## community_overviews (code [community 1])
File: src/serve/handlers.rs L546:C1-L623:C2

Connections:
- community_overviews `calls` community_focus_label [tree-sitter:heuristic]
- community_overviews `calls` node_data [tree-sitter-stack-graphs:resolved]
- community_overviews `calls` is_framework_noise_node [tree-sitter-stack-graphs:resolved]
- community_overviews `contains` handlers [tree-sitter:resolved]
```

**What's new:** Cross-file edges from `community_stats` to `node_data`,
`edges_with_endpoints`, and `cluster` are now resolved through Stack
Graphs. The community detection pipeline's dependency on the graph model
is source-backed instead of assumed.
