# Graphenium MCP Tool Reference

Graphenium exposes compact graph tools through MCP so AI agents can ask structural questions without reading unrelated source files.

## Read tools

| Tool | Purpose |
|---|---|
| `graph_stats` | Node/edge counts, file types, confidence and provenance breakdowns |
| `architecture_summary` | Communities, focus paths, god nodes, and confidence summary |
| `query_graph` | Keyword-scored BFS/DFS traversal within a token budget |
| `get_node` | Full node details by ID or label |
| `get_neighbors` | Direct neighbours with edge types and confidence |
| `get_community` | All nodes in a community cluster |
| `god_nodes` | Top N most-connected hub nodes |
| `shortest_path` | Path between any two components |
| `summarize_file` | Extracted symbols from a source file |
| `reload_graph` | Hot-swap the graph without restarting the MCP server |

## Write tools

| Tool | Purpose |
|---|---|
| `add_node` | Register concepts the AST cannot capture |
| `add_edge` | Record relationships confirmed through inspection |
| `remove_edge` | Correct false positives or stale relationships |

All writes persist to disk immediately.

## Confidence-aware and policy-driven tools

| Tool | Purpose |
|---|---|
| `resolution_report` | Resolution quality statistics: resolved vs unresolved reference counts and ratios |
| `ambiguous_symbols` | List low-trust edges with `AMBIGUOUS` confidence for manual review |
| `unresolved_references` | Missing dependencies the resolver could not bind |
| `safest_path` | Confidence-aware pathfinding between nodes, preferring `EXTRACTED` edges |
| `verification_plan` | Prioritized verification plan based on impact and risk |
| `blast_radius` | Downstream impact analysis for affected files and symbols |
| `agent_change_gate` | Policy-based gate checks for CI pipelines |
| `diff_graph` | Snapshot comparison and symbol-level diff between graph versions |
| `next_files_to_read` | Reading-order recommendation derived from a verification plan |
| `graph_info` | Full graph metadata: schema version, project root, build timestamp, languages, and counts |
| `recluster` | Re-run community detection after manual node/edge edits |

## Composite and trust tools

| Tool | Purpose |
|---|---|
| `analyse_symbol` | Single-turn composite: resolves a symbol and groups behavioral/structural connections with trust profile |
| `module_dependencies` | Module-to-module dependency summary between two directory paths |
| `what_changed` | Risk-sorted delta against a stored snapshot: removed symbols, community moves, additions |
| `query_transitive` | BFS transitive closure from a seed symbol with depth, direction, and relation filtering |

## Query behavior worth highlighting

- `summarize_file` supports `show_leaves`; default `false` hides low-degree leaf symbols to save tokens.
- `query_graph` and related tools support `include_tests`; default `false` excludes test/spec nodes.
- `get_neighbors` supports `extracted_only` for strict source-backed traversal.
- Query responses include a trust profile line such as `N EXTRACTED, N INFERRED, N AMBIGUOUS`.
- `shortest_path` output includes per-hop confidence breakdowns.
- Query outputs use project-root-relative paths.

## Agent usage pattern

Recommended order for architectural tasks:

1. `graph_info` or `architecture_summary`
2. `query_graph` or `analyse_symbol`
3. `shortest_path`, `safest_path`, or `query_transitive`
4. `next_files_to_read`
5. Source-code reading in the selected files
6. `blast_radius`, `verification_plan`, or `agent_change_gate` before editing or review

