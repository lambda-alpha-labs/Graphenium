# MCP Tools Reference

Graphenium exposes **22 MCP tools** across 5 categories: Read, Composite, Trust, Write, and Diff. Each tool accepts parameters as JSON and returns formatted Markdown text.

---

## Read Tools (11)

### `graph_info`
- **Returns**: Project root, schema version, build timestamp, extraction mode, languages, node/edge counts
- **Use when**: Confirming which graph the server has loaded before acting on results

### `graph_stats`
- **Returns**: Node/edge/hyperedge counts, communities, node-type breakdown, edge confidence distribution
- **Use when**: Getting a quick sense of graph size and quality

### `query_graph(keywords, depth?, budget?, ...)`
- **Returns**: Matching nodes + connections formatted as Markdown
- **Use when**: Searching the graph by keyword relevance
- **Parameters**: `keywords` (required), `depth` (1-6, default 3), `budget` (default 2000), `dfs`, `path_prefix`, `exclude_path`, `include_relations`, `exclude_relations`, `node_types`, `generated_code_mode`, `include_tests`, `min_degree`, `ast_only_tuning`

### `get_node(id)`
- **Returns**: Node label, file type, source file, source span, community, degree
- **Use when**: Looking up a specific symbol by ID or label

### `get_neighbors(node_id, relation?, max_neighbors?, extracted_only?)`
- **Returns**: Direct neighbors with edge relation types, confidence levels, and scores
- **Use when**: Exploring what a node connects to

### `get_community(community_id, include_members?)`
- **Returns**: Representative nodes, files, dominant relations; optionally full member list
- **Use when**: Understanding an architectural community

### `god_nodes(n?, path_prefix?, node_types?, ...)`
- **Returns**: Top N most connected nodes (hubs), excluding file-level stubs
- **Use when**: Finding architectural hotspots

### `shortest_path(from, to, mode?, ...)`
- **Returns**: Path between two nodes with relation details
- **Use when**: Finding how two symbols are connected
- **Mode**: `semantic` (prefers meaningful relations) or `strict` (fewest hops)

### `summarize_file(path, group_by?, show_leaves?, min_degree?)`
- **Returns**: All graph symbols extracted from a file, grouped by kind or community
- **Use when**: Answering "what's in this file?" without reading source
- **Token optimization**: Hubs shown by default; low-degree leaves hidden unless `show_leaves=true`

### `architecture_summary(...)`
- **Returns**: Repository-level summary with major communities, cross-community connectors, and hotspots
- **Use when**: Orienting on a codebase before digging into specific files

### `query_transitive(seed, depth?, relation?, direction?)`
- **Returns**: Full transitive closure from a seed symbol (BFS)
- **Parameters**: `seed` (required), `depth` (1-6, default 3), `direction` (forward/reverse/both)
- **Use when**: Finding all nodes reachable from a given symbol, or all nodes that can reach it

---

## Composite Tools (3)

### `analyse_symbol(symbol)`
- **Returns**: Single-turn composite analysis: node metadata + behavioral connections (calls, uses, inherits, implements) + structural connections (imports, contains) + trust profile
- **Use when**: Getting a comprehensive understanding of a symbol in one call

### `module_dependencies(module_a, module_b)`
- **Returns**: Summary of dependency connections between two modules/directories, grouped by relation type
- **Use when**: Understanding how two parts of the codebase relate

### `what_changed(snapshot_name?)`
- **Returns**: Risk-sorted delta: removed symbols (highest risk), community moves, added symbols, downstream impact
- **Use when**: Comparing current graph against a stored snapshot after analyzing changes

---

## Trust Tools (7)

### `resolution_report()`
- **Returns**: Import resolution %, call resolution %, ambiguous edge count, unresolved references
- **Use when**: Checking graph trust quality before acting on results

### `ambiguous_symbols()`
- **Returns**: List of ambiguous (low-confidence) edges
- **Use when**: Finding edges that need manual verification

### `unresolved_references()`
- **Returns**: List of import edges where the target symbol was not found
- **Use when**: Finding potentially missing dependencies

### `safest_path(from, to)`
- **Returns**: Path with highest-confidence edges, plus a safety score (0.0-1.0)
- **Use when**: Need a trustworthy path, not necessarily the shortest

### `verification_plan(changed_nodes)`
- **Returns**: Prioritized 7-tier plan: must-read files → tests → ambiguous edges → risk gates
- **Use when**: Planning what to verify after symbol changes

### `blast_radius(changed_nodes)`
- **Returns**: Downstream impact: affected files, communities, edge confidence distribution
- **Use when**: Understanding the blast radius of proposed changes

### `agent_change_gate(changed_nodes, min_resolution?, max_ambiguous?)`
- **Returns**: Policy gate evaluations (pass/fail table) with optional threshold overrides
- **Use when**: Running trust quality gates for CI before committing changes

---

## Write Tools (4)

### `add_node(id, label, file_type, source_file, ...)`
- **Returns**: Confirmation with total node+edge counts after save
- **Use when**: Registering architectural concepts, rationale nodes, or other entities the AST extractor doesn't capture

### `add_edge(source, target, relation, confidence, source_file, ...)`
- **Returns**: Confirmation with total node+edge counts
- **Use when**: Recording verified relationships between nodes

### `remove_edge(source, target, relation?)`
- **Returns**: Confirmation of removed edges
- **Use when**: Correcting false positives or removing stale relationships

### `recluster()`
- **Returns**: Re-runs Louvain community detection and re-assigns communities
- **Use when**: After adding nodes/edges via write tools, community assignments may be stale

---

## Diff Tools (2)

### `diff_graph(before_graph, after_graph)`
- **Returns**: Added/removed nodes and edges between two graph snapshots
- **Use when**: Comparing two exported graph.json files

### `review_plan(before_graph_path?, after_graph_path?)`
- **Returns**: Full review plan: symbol inventory changes + prioritized verification plan
- **Use when**: Generating a complete pre-commit review from graph diffs

---

## Tool Selection Guide

| Goal | Recommended Tool |
|---|---|
| "What does this repo look like?" | `run_datalog` | Run a Datalog query against the loaded graph. Supports rules, goals, facts, and negation (not). Budget-bounded. | Right column: `query: string`, `step_budget?: number` | Declarative reachability, constraint queries, custom graph analysis | Read |
| `graph_info` | + `architecture_summary` |
| "What does X depend on?" | `get_neighbors(X)` |
| "Find me code related to Y" | `query_graph("Y")` |
| "How are A and B connected?" | `shortest_path(A, B)` |
| "What's in this file?" | `summarize_file("path/to/file")` |
| "What changed since last snapshot?" | `what_changed()` |
| "What's the impact of changing Z?" | `blast_radius(Z)` |
| "Is the graph trustworthy?" | `resolution_report()` |
| "Plan my verification steps" | `verification_plan(nodes)` |
| "Full analysis of a symbol" | `analyse_symbol(symbol)` |
| "Multi-hop transitive closure" | `query_transitive(seed)` |
