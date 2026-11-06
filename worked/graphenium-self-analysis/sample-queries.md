# Sample Queries: Graphenium Self-Analysis

The following examples illustrate the command-line output of Graphenium's structural query engine (`gm query`) running on Graphenium's own compiled Rust codebase (1,410 symbols, 3,606 boundaries, and 19 cohesive communities).

These queries demonstrate how Graphenium resolves cross-file call boundaries deterministically using AST-proven parsing rather than fuzzy semantic guessing.

---

## Query 1: "serve module handlers mcp"

This query targets the public interfaces and handlers of Graphenium's background server layer.

```text
$ gm query "serve module handlers mcp" --budget 800

# Graph Query: serve module handlers mcp

Found 74 relevant nodes (of 1410)

AST-only tuning active: suppressing common import/generated-code noise by default

Filter: generated/template/vendor paths excluded

Found 74 node(s)

## module_dependencies (code [community 3])
File: src/serve/handlers.rs
Span: L1637:C5-L1763:C6
Match: direct keyword match on label, id, path for `module`, `serve`, `handlers` (score 3.00)

Connections:
- module_dependencies `calls` is_ast_only [tree-sitter-stack-graphs:resolved]
- module_dependencies `calls` is_namespace_aggregation_node [tree-sitter-stack-graphs:resolved]
- module_dependencies `calls` nodes [tree-sitter-stack-graphs:resolved]
- module_dependencies `calls` is_node_in_module [tree-sitter:heuristic]
- module_dependencies `calls` node_data [tree-sitter-stack-graphs:resolved]
- module_dependencies `calls` edges_iter [tree-sitter-stack-graphs:resolved]
- module_dependencies `calls` GrapheniumServer::new (new) [tree-sitter:heuristic]
- module_dependencies `calls` GrapheniumServer::graph (graph) [tree-sitter:heuristic]
- module_dependencies `calls` trace_path [tree-sitter:heuristic]
- module_dependencies `method` GrapheniumServer [tree-sitter:resolved]

## trace_path_routes_shortest_safest_transitive_and_module_modes (code [community 3])
File: src/serve/handlers.rs
Span: L5691:C5-L5813:C6
Match: direct keyword match on label, id, path for `module`, `handlers`, `serve` (score 3.00)

Connections:
- trace_path_routes_shortest_safest_transitive_and_module_modes `calls` trace_path [tree-sitter:heuristic]
- trace_path_routes_shortest_safest_transitive_and_module_modes `calls` make_server [tree-sitter:heuristic]
- trace_path_routes_shortest_safest_transitive_and_module_modes `contains` handlers [tree-sitter:resolved]

## is_node_in_module (code [community 3])
File: src/serve/handlers.rs
Span: L203:C5-L220:C6
Match: direct keyword match on label, id, path for `module`, `serve`, `handlers` (score 3.00)

Connections:
- is_node_in_module `calls` module_dependencies [tree-sitter:heuristic]
- is_node_in_module `method` GrapheniumServer [tree-sitter:resolved]

## handlers (code [community 3])
File: src/serve/handlers.rs
Span: L1:C1-L5894:C1
Match: direct keyword match on label, id, path for `handlers`, `serve` (score 2.00)

Connections:
- handlers `contains` mutate_graph_routes_add_node_add_edge_remove_edge_and_recluster [tree-sitter:resolved]
- handlers `contains` validate_plan_always_runs_the_delta_gate [tree-sitter:resolved]
- handlers `contains` trace_path_routes_shortest_safest_transitive_and_module_modes [tree-sitter:resolved]
- handlers `contains` inspect_symbol_composes_detail_neighbors_references_analysis_and_file_summary [tree-sitter:resolved]
- handlers `contains` get_architecture_summary_can_focus_on_community_and_include_god_nodes [tree-sitter:resolved]
- handlers `contains` get_graph_info_can_append_stats_and_diagnostics [tree-sitter:resolved]
- handlers `contains` verify_changes_reports_missing_planned_edges [tree-sitter:resolved]
- handlers `contains` mutate_graph_persist_roundtrip_preserves_all_edges [tree-sitter:resolved]
- handlers `contains` reload_graph_reports_parse_error [tree-sitter:resolved]
- handlers `contains` inspect_symbol_file_summary_can_group_by_community [tree-sitter:resolved]
- handlers `contains` inspect_symbol_file_summary_no_match_returns_clean_message [tree-sitter:resolved]
- handlers `contains` inspect_symbol_file_summary_handles_backslash_paths [tree-sitter:resolved]
- handlers `contains` inspect_symbol_file_summary_lists_symbols_matching_suffix [tree-sitter:resolved]
- handlers `contains` reload_graph_missing_path_without_default [tree-sitter:resolved]
- handlers `contains` reload_graph_swaps_state [tree-sitter:resolved]
- handlers `contains` list_tools_sanitizes_gemini_incompatible_nullable_schemas [tree-sitter:resolved]
- handlers `contains` every_tool_parameter_has_a_schema_description [tree-sitter:resolved]
- handlers `contains` list_tools_exposes_exactly_twelve_consolidated_tools [tree-sitter:resolved]
- handlers `contains` get_info_has_tool_capabilities [tree-sitter:resolved]
- handlers `contains` trace_path_shortest_auto_tunes_for_ast_only_graphs [tree-sitter:resolved]
- handlers `contains` trace_path_rejects_unknown_path_mode [tree-sitter:resolved]
- handlers `contains` trace_path_shortest_honors_relation_filters [tree-sitter:resolved]
- handlers `contains` trace_path_shortest_same_node [tree-sitter:resolved]
- handlers `contains` trace_path_shortest_no_path [tree-sitter:resolved]
- handlers `contains` trace_path_shortest_found [tree-sitter:resolved]
- handlers `contains` get_architecture_summary_auto_tunes_for_ast_only_graphs [tree-sitter:resolved]
- handlers `contains` get_architecture_summary_honors_generated_code_mode [tree-sitter:resolved]
- handlers `contains` get_architecture_summary_reports_major_sections [tree-sitter:resolved]
- handlers `contains` get_graph_info_stats_honor_node_type_filter [tree-sitter:resolved]
- handlers `contains` get_graph_info_stats_honor_path_scope [tree-sitter:resolved]
- handlers `contains` get_graph_info_can_include_stats [tree-sitter:resolved]
- handlers `contains` get_architecture_summary_god_nodes_honor_node_type_filter [tree-sitter:resolved]
- handlers `contains` get_architecture_summary_god_nodes_honor_path_scope [tree-sitter:resolved]
- handlers `contains` get_architecture_summary_can_include_god_nodes [tree-sitter:resolved]
- handlers `contains` get_architecture_summary_reports_missing_community [tree-sitter:resolved]
- handlers `contains` get_architecture_summary_can_include_full_community_member_list [tree-sitter:resolved]
- handlers `contains` get_architecture_summary_returns_community_members [tree-sitter:resolved]
- handlers `contains` inspect_symbol_deduplicates_neighbor_rows [tree-sitter:resolved]
- handlers `contains` inspect_symbol_neighbor_relation_filter [tree-sitter:resolved]
- handlers `contains` inspect_symbol_can_include_neighbors [tree-sitter:resolved]
- handlers `contains` inspect_symbol_reports_missing_node [tree-sitter:resolved]
- handlers `contains` inspect_symbol_returns_node_by_label [tree-sitter:resolved]
- handlers `contains` inspect_symbol_returns_node_by_id [tree-sitter:resolved]
- handlers `contains` query_graph_auto_tunes_for_ast_only_graphs [tree-sitter:resolved]
- handlers `contains` query_graph_honors_generated_code_mode [tree-sitter:resolved]
- handlers `contains` query_graph_honors_node_type_and_relation_filters [tree-sitter:resolved]
- handlers `contains` query_graph_honors_path_scope [tree-sitter:resolved]
- handlers `contains` query_graph_returns_text [tree-sitter:resolved]
- handlers `contains` make_architecture_server [tree-sitter:resolved]
- handlers `contains` make_ast_only_generated_server [tree-sitter:resolved]
- handlers `contains` make_generated_server [tree-sitter:resolved]
- handlers `contains` make_scoped_server [tree-sitter:resolved]
- handlers `contains` make_server [tree-sitter:resolved]
- handlers `contains` top_bridge_nodes [tree-sitter:resolved]
- handlers `contains` community_overviews [tree-sitter:resolved]
- handlers `contains` relative_path [tree-sitter:resolved]
- handlers `contains` format_plan_verification [tree-sitter:resolved]
- handlers `contains` normalize_display_path [tree-sitter:resolved]
- handlers `contains` common_path_focus [tree-sitter:resolved]
- handlers `contains` community_focus_label [tree-sitter:resolved]
- handlers `contains` top_counts [tree-sitter:resolved]
- handlers `contains` CommunityOverview [tree-sitter:resolved]
- handlers `contains` GrapheniumServer [tree-sitter:resolved]


[... output truncated: showing 4 of 74 matches. Remaining 70 matches would require approximately an additional 17500 characters of budget. Increase the 'budget' parameter or decrease 'depth' to refine your query.]

---
**Trust Profile**: 1562 EXTRACTED (source-backed), 2044 INFERRED (heuristics), 0 AMBIGUOUS (review recommended) connections in this view.
```

### Architectural Analysis:
The cross-file call boundary `module_dependencies ──► node_data` is marked with explicit `[tree-sitter-stack-graphs:resolved]` provenance. This means the connection is compiler-proven: Graphenium has verified that the `module_dependencies` function physically executes a call to `node_data` in another file, establishing a high-trust boundary. The same view also shows heuristic call sites separately, so agents can distinguish structural certainty from inferred relationships.

---

## Query 2: "graph build extraction"

This query targets the compilation and index assembly pipeline of Graphenium.

```text
$ gm query "graph build extraction" --budget 800

# Graph Query: graph build extraction

Found 35 relevant nodes (of 1410)

AST-only tuning active: suppressing common import/generated-code noise by default

Filter: generated/template/vendor paths excluded

Found 35 node(s)

## build_from_extraction (code [community 0])
File: src/build.rs
Span: L29:C1-L58:C2
Match: direct keyword match on label, id, path for `build`, `extraction`, `graph` (score 3.00)

Connections:
- build_from_extraction `calls` upsert_node [tree-sitter-stack-graphs:resolved]
- build_from_extraction `calls` contains_node [tree-sitter-stack-graphs:resolved]
- build_from_extraction `calls` hyperedges_added_to_graph [tree-sitter:heuristic]
- build_from_extraction `calls` semantic_overrides_ast [tree-sitter:heuristic]
- build_from_extraction `calls` last_write_wins_for_duplicate_id [tree-sitter:heuristic]
- build_from_extraction `calls` dangling_edge_dropped [tree-sitter:heuristic]
- build_from_extraction `calls` basic_build [tree-sitter:heuristic]
- build_from_extraction `calls` build_merged [tree-sitter:heuristic]
- build_from_extraction `contains` build [tree-sitter:resolved]

## BuildTarget (code [community 0])
File: src/model/extraction.rs
Span: L76:C1-L80:C2
Match: direct keyword match on label, id, path for `build`, `extraction`, `graph` (score 3.00)

Connections:
- BuildTarget `contains` extraction [tree-sitter:resolved]

## build (code [community 0])
File: src/build.rs
Span: L1:C1-L207:C1
Match: direct keyword match on label, id, path for `build`, `graph` (score 2.00)

Connections:
- build `contains` token_counts_summed_in_merged [tree-sitter:resolved]
- build `contains` hyperedges_added_to_graph [tree-sitter:resolved]
- build `contains` semantic_overrides_ast [tree-sitter:resolved]
- build `contains` build_merged_combines_results [tree-sitter:resolved]
- build `contains` last_write_wins_for_duplicate_id [tree-sitter:resolved]
- build `contains` dangling_edge_dropped [tree-sitter:resolved]
- build `contains` basic_build [tree-sitter:resolved]
- build `contains` tests::edge (edge) [tree-sitter:resolved]
- build `contains` tests::node (node) [tree-sitter:resolved]
- build `contains` build_merged [tree-sitter:resolved]
- build `contains` build_from_extraction [tree-sitter:resolved]
- build `contains` BuildStats [tree-sitter:resolved]

## parse_extraction (code [community 4])
File: src/semantic/parse.rs
Span: L30:C1-L32:C2
Match: direct keyword match on label, id, path for `extraction`, `graph` (score 2.00)

Connections:
- parse_extraction `calls` extract_json [tree-sitter:heuristic]
- parse_extraction `calls` build_result [tree-sitter:heuristic]
- parse_extraction `calls` empty_input_returns_empty [tree-sitter:heuristic]
- parse_extraction `calls` completely_unparseable_returns_empty [tree-sitter:heuristic]
- parse_extraction `calls` confidence_score_clamped [tree-sitter:heuristic]
- parse_extraction `calls` source_location_is_preserved_when_present [tree-sitter:heuristic]
- parse_extraction `calls` valid_and_invalid_items_mixed [tree-sitter:heuristic]
- parse_extraction `calls` hyperedge_with_three_nodes_kept [tree-sitter:heuristic]
- parse_extraction `calls` hyperedge_with_two_nodes_dropped [tree-sitter:heuristic]
- parse_extraction `calls` missing_source_edge_dropped [tree-sitter:heuristic]
- parse_extraction `calls` empty_id_node_dropped [tree-sitter:heuristic]
- parse_extraction `calls` layer3_embedded_json [tree-sitter:heuristic]
- parse_extraction `calls` layer2_plain_code_block [tree-sitter:heuristic]
- parse_extraction `calls` layer2_json_code_block [tree-sitter:heuristic]
- parse_extraction `calls` layer1_direct_parse [tree-sitter:heuristic]
- parse_extraction `contains` parse [tree-sitter:resolved]
- parse_extraction `calls` process_batch [tree-sitter-stack-graphs:resolved]


[... output truncated: showing 4 of 35 matches. Remaining 31 matches would require approximately an additional 7750 characters of budget. Increase the 'budget' parameter or decrease 'depth' to refine your query.]

---
**Trust Profile**: 1562 EXTRACTED (source-backed), 2044 INFERRED (heuristics), 0 AMBIGUOUS (review recommended) connections in this view.
```

### Architectural Analysis:
The returned connections, such as `build_from_extraction ──► upsert_node`, are confirmed stack-graph relationships. The same query also surfaces the `contains` boundaries that tie the build tests and helpers back to `src/build.rs`, making the module's internal surface explicit without opening the source file.

---

## Query 3: "community detection"

This query targets Graphenium's Louvain community clustering and cohesion scoring modules.

```text
$ gm query "community detection" --budget 800

# Graph Query: community detection

Found 23 relevant nodes (of 1410)

AST-only tuning active: suppressing common import/generated-code noise by default

Filter: generated/template/vendor paths excluded

Found 23 node(s)

## community_stats (code [community 11])
File: src/cluster/cohesion.rs
Span: L38:C1-L112:C2
Match: direct keyword match on label, id for `community` (score 1.00)

Connections:
- community_stats `calls` focus_label [tree-sitter-stack-graphs:resolved]
- community_stats `calls` node_data [tree-sitter-stack-graphs:resolved]
- community_stats `calls` edges_with_endpoints [tree-sitter-stack-graphs:resolved]
- community_stats `calls` cluster [tree-sitter-stack-graphs:resolved]
- community_stats `calls` two_communities_reported [tree-sitter:heuristic]
- community_stats `calls` parallel_edges_do_not_exceed_cohesion_one [tree-sitter:heuristic]
- community_stats `calls` singleton_cohesion_zero [tree-sitter:heuristic]
- community_stats `calls` partial_connectivity [tree-sitter:heuristic]
- community_stats `calls` no_internal_edges_cohesion_zero [tree-sitter:heuristic]
- community_stats `calls` clique_has_cohesion_one [tree-sitter:heuristic]
- community_stats `contains` cohesion [tree-sitter:resolved]

## community_overviews (code [community 3])
File: src/serve/handlers.rs
Span: L643:C1-L720:C2
Match: direct keyword match on label, id for `community` (score 1.00)

Connections:
- community_overviews `calls` community_focus_label [tree-sitter:heuristic]
- community_overviews `calls` degree [tree-sitter-stack-graphs:resolved]
- community_overviews `calls` Node::display_label (display_label) [tree-sitter-stack-graphs:resolved]
- community_overviews `calls` is_framework_noise_node [tree-sitter-stack-graphs:resolved]
- community_overviews `calls` node_data [tree-sitter-stack-graphs:resolved]
- community_overviews `calls` node_edges [tree-sitter-stack-graphs:resolved]
- community_overviews `calls` GrapheniumServer::new (new) [tree-sitter:heuristic]
- community_overviews `calls` render_architecture_summary [tree-sitter:heuristic]
- community_overviews `contains` handlers [tree-sitter:resolved]

## summarize_community (code [community 3])
File: src/serve/handlers.rs
Span: L272:C5-L397:C6
Match: direct keyword match on label, id for `community` (score 1.00)

Connections:
- summarize_community `calls` community_focus_label [tree-sitter:heuristic]
- summarize_community `calls` degree [tree-sitter-stack-graphs:resolved]
- summarize_community `calls` edges_with_endpoints [tree-sitter-stack-graphs:resolved]
- summarize_community `calls` GrapheniumServer::new (new) [tree-sitter:heuristic]

## community_focus_label (code [community 3])
File: src/serve/handlers.rs
Span: L499:C1-L520:C2
Match: direct keyword match on label, id for `community` (score 1.00)

Connections:
- community_focus_label `calls` community_overviews [tree-sitter:heuristic]
- community_focus_label `calls` summarize_community [tree-sitter:heuristic]
- community_focus_label `contains` handlers [tree-sitter:resolved]

## all_nodes_assigned_a_community (code [community 2])
File: src/cluster/mod.rs
Span: L139:C5-L148:C6
Match: direct keyword match on label, id for `community` (score 1.00)

Connections:
- all_nodes_assigned_a_community `calls` cluster [tree-sitter:heuristic]


[... output truncated: showing 5 of 23 matches. Remaining 18 matches would require approximately an additional 4500 characters of budget. Increase the 'budget' parameter or decrease 'depth' to refine your query.]

---
**Trust Profile**: 1562 EXTRACTED (source-backed), 2044 INFERRED (heuristics), 0 AMBIGUOUS (review recommended) connections in this view.
```

### Architectural Analysis:
Multi-hop boundaries connecting the cohesion analyzer (`community_stats`) back to the core data models (`node_data` and `edges_with_endpoints` in `src/model/graph.rs`) are resolved with compiler-level certainty. Notably, the old `len` artifact is gone: `.len()` calls on collections no longer bind to `Manifest::len`. The cohesion tests now include `parallel_edges_do_not_exceed_cohesion_one`, which locks in the guarantee that reported cohesion never exceeds `1.000`.
