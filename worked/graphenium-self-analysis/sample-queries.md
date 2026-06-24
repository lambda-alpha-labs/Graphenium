# Sample queries: Graphenium self-analysis

Real output from `gm query` on Graphenium's own AST-only graph
(868 nodes, 1,766 edges, 19 communities).

## Query: "mcp server handlers"

```
# Graph Query: mcp server handlers

Found 32 relevant nodes (of 868)

## GrapheniumServer (code [community 3])
File: src/serve/handlers.rs L27:C1-L33:C2

Connections:
- GrapheniumServer `method` query_graph
- GrapheniumServer `method` get_node
- GrapheniumServer `method` get_neighbors
- GrapheniumServer `method` get_community
- GrapheniumServer `method` god_nodes
- GrapheniumServer `method` graph_stats
- GrapheniumServer `method` shortest_path
- GrapheniumServer `method` architecture_summary
- GrapheniumServer `method` summarize_file
- GrapheniumServer `method` add_node
- GrapheniumServer `method` add_edge
- GrapheniumServer `method` remove_edge
- GrapheniumServer `method` reload_graph
- GrapheniumServer `contains` handlers

## make_server (code [community 3])
File: src/serve/handlers.rs L1779:C5-L1797:C6

Connections:
- make_server `calls` add_edge
- make_server `calls` new
- make_server `contains` handlers

## get_node (code [community 3])
File: src/serve/handlers.rs L702:C5-L742:C6

Connections:
- get_node `calls` resolve_id
- get_node `calls` get_node_by_id
- get_node `calls` get_node_by_label
- get_node `method` GrapheniumServer
```

## Query: "graph build extraction"

```
## build_from_extraction (code [community 1])
File: src/build.rs L31:C1-L60:C2

Connections:
- build_from_extraction `calls` basic_build
- build_from_extraction `calls` build_merged
- build_from_extraction `contains` build

## parse_extraction (code [community 10])
File: src/semantic/parse.rs L31:C1-L33:C2

Connections:
- parse_extraction `calls` extract_json
- parse_extraction `calls` build_result
- parse_extraction `contains` parse
```

## Query: "community detection"

```
## community_stats (code [community 6])
File: src/cluster/cohesion.rs L43:C1-L106:C2

Connections:
- community_stats `calls` clique_has_cohesion_one
- community_stats `contains` cohesion

## summarize_community (code [community 3])
File: src/serve/handlers.rs L146:C5-L254:C6

Connections:
- summarize_community `calls` community_focus_label
- summarize_community `method` GrapheniumServer
```
