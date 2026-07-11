# MCP Tools Reference

Graphenium exposes MCP tools so AI coding agents can query repository structure without reading every file.

The tools are grouped into six categories:

1. Read tools
2. Composite tools
3. Trust tools
4. Planning workspace tools
5. Write tools
6. Diff tools

## Agent rule of thumb

```text
Start with graph_info.
Use read tools to understand.
Use trust tools before acting.
Use planning tools for multi-file work.
Use diff tools after edits.
Use write tools only after source inspection.
```

## Read tools

### `graph_info`

Returns project root, schema version, build timestamp, extraction mode, languages, node counts, edge counts, graph path, and graph identity.

When the loaded `graph.json` is older than the serving `gm` binary or project source files, the response includes a **Graph may be stale** warning with the specific reason. The server still serves the existing graph so session starts stay fast on large repositories.

Use when:

- starting a session
- confirming the loaded graph
- checking whether the agent is using the right repository
- deciding whether to rebuild before trusting structural queries

If stale, rebuild and hot-swap:

```sh
gm run . --no-semantic --no-viz
```

Then call `reload_graph` (no path needed).

### `graph_stats`

Returns node, edge, hyperedge, community, node-type, and confidence distribution statistics.

Use when:

- checking graph scale
- assessing overall confidence profile
- deciding whether the graph is healthy enough to use

### `query_graph`

Searches the graph using keyword relevance and traversal.

Common parameters:

| Parameter | Purpose |
|---|---|
| `keywords` | Search query |
| `depth` | Traversal depth |
| `budget` | Output token budget |
| `dfs` | Use deeper, narrower traversal |
| `path_prefix` | Scope to a directory or module |
| `exclude_path` | Remove noisy paths |
| `include_relations` | Restrict relationship types |
| `exclude_relations` | Exclude relationship types |
| `node_types` | Restrict node kinds |
| `generated_code_mode` | Include, exclude, or only generated code |
| `include_tests` | Include test nodes |
| `min_degree` | Filter low-degree symbols |
| `ast_only_tuning` | Tune for AST-only graphs |

Use when:

- searching for feature-related code
- finding a target symbol
- exploring a module
- asking broad architecture questions

### `get_node`

Returns metadata for one node: label, file type, source file, source span, community, and degree.

Use when:

- resolving a symbol precisely
- checking source location
- disambiguating label collisions

### `get_neighbors`

Returns direct neighbors with relation types, confidence levels, and scores.

Useful options include relation filters, max neighbors, and extracted-only mode.

Use when:

- asking what calls a symbol
- asking what a symbol calls
- checking imports, uses, inherits, or implements relationships
- planning direct-impact changes

### `get_community`

Returns community-level context and optionally full member lists.

Use when:

- understanding architectural clusters
- finding module boundaries
- checking whether a symbol sits in the expected community

### `god_nodes`

Returns the most connected nodes after filtering obvious noise.

Use when:

- identifying hubs
- finding high-risk change targets
- spotting architectural chokepoints

### `shortest_path`

Returns a path between two nodes.

Modes:

| Mode | Meaning |
|---|---|
| `strict` | Fewest hops |
| `semantic` | Prefers meaningful relationships |

Use when:

- explaining how two parts connect
- checking whether a dependency path exists

### `summarize_file`

Returns graph symbols extracted from a file, grouped by kind or community.

Use when:

- answering what is in a file without reading the full source
- choosing whether a file deserves direct inspection

### `architecture_summary`

Returns repository-level architecture summary with major communities, cross-community connectors, and hotspots.

Use when:

- entering a new repository
- preparing an agent orientation
- creating a map before a multi-file change

### `query_transitive`

Returns a transitive closure from a seed symbol.

Parameters include seed, depth, relation, and direction.

Use when:

- finding all downstream consumers
- finding all upstream dependencies
- tracing impact across multiple hops

### `run_datalog`

Runs a Datalog query against the loaded graph. A standard library of predicates is pre-loaded on every query; you do not need to define transitive closure or hub detection yourself.

**Pre-loaded stdlib predicates:**

| Predicate | Purpose |
|---|---|
| `calls_transitive/2` | Transitive call reachability |
| `imports_transitive/2` | Transitive import reachability |
| `depends_transitive/2` | Transitive dependency (calls or imports) |
| `same_community/2` | Same Louvain community |
| `is_hub/1` | High-degree hub node |
| `is_orphan/1` | Node with no edges |
| `circular_dependency/2` | Mutual dependency cycle |
| `bypasses_layer/3` | Layering violation |

**Base EDB relations:** `calls/3`, `imports/3`, `contains/3`, `inherits/3`, `implements/3`, `degree/2`, `hub/1`, plus legacy `edge/5` and `node/5`.

Use when:

- asking declarative reachability questions
- finding constraint violations
- building custom graph analyses

Examples:

```text
?- calls_transitive("handlers_run_datalog", X).
?- is_hub(X).
?- circular_dependency(X, Y).
?- bypasses_layer(X, Y, Z).
```

Requires a `gm` binary that includes the Datalog stdlib (v0.19.0+). If results are empty on an old server process, restart MCP or confirm `gm --version`.

## Composite tools

### `analyse_symbol`

Returns a complete single-symbol analysis: node metadata, behavioral connections, structural connections, and trust profile.

Use when:

- preparing to edit a symbol
- creating a pre-edit safety plan

### `module_dependencies`

Summarizes dependency connections between two modules or directories.

Use when:

- checking boundary coupling
- explaining why two modules are connected
- reviewing architecture drift

### `what_changed`

Returns a risk-sorted delta against a snapshot: removed symbols, community moves, additions, and downstream impact.

Use when:

- reviewing an agent patch
- producing a pull request review plan

## Trust tools

### `resolution_report`

Returns import resolution, call resolution, ambiguous edge count, and unresolved references.

Use before trusting graph output for a high-risk change.

### `ambiguous_symbols`

Lists ambiguous edges and collisions that require source inspection.

Use when an agent needs to know what not to assume.

### `unresolved_references`

Lists references whose targets were not found in the graph.

Use when diagnosing missing extraction, unsupported language features, or ignore-rule issues.

### `safest_path`

Returns the highest-confidence path between two symbols, plus a safety score.

Use when correctness matters more than shortest route.

### `verification_plan`

Returns a prioritized verification plan for changed nodes.

Typical output tiers:

1. must-read files
2. affected public interfaces
3. downstream consumers
4. tests to inspect or run
5. ambiguous edges to verify
6. architecture gates
7. CI policy results

### `blast_radius`

Returns downstream impact for changed nodes: affected files, communities, and confidence profile.

Use before and after agent edits.

### `agent_change_gate`

Evaluates trust-quality policy gates such as minimum resolution and maximum ambiguous edges.

Optional parameter:

| Parameter | Purpose |
|---|---|
| `plan_id` | Run pre-flight architecture policy validation for a planning workspace |

Use in CI or pre-review agent workflows. When `plan_id` is provided, the response includes a pre-flight section alongside the trust gate table.

## Planning workspace tools

Planning tools support the design-then-verify loop with a **pre-flight gate** before coding and a **compliance audit** after implementation.

```mermaid
graph LR
    A[Create plan] --> B[Declare intended symbols]
    B --> P[Pre-flight policy check]
    P -->|pass| C[Write implementation]
    P -->|fail| R[Revise plan]
    C --> D[Compare planned graph to physical graph]
    D --> E[Report implemented, missing, unplanned]
```

Pre-flight rules load from `.graphenium/policy.json` in the project root (see [`docs/CI_AND_GOVERNANCE.md`](CI_AND_GOVERNANCE.md)).

### `create_planning_workspace`

Creates a virtual workspace and returns a plan ID.

Use when starting a multi-step architectural change.

### `add_planned_symbol`

Registers an intended symbol or relationship before implementation.

When `.graphenium/policy.json` defines architecture rules, this tool runs a pre-flight check automatically. If the proposed symbol or edge would violate policy, the tool returns `PRE_FLIGHT_VIOLATION` and does not persist the change.

Use when declaring the design the agent intends to implement.

### `validate_plan`

Performs pre-flight architecture policy validation on a planning workspace **before** writing code.

| Parameter | Purpose |
|---|---|
| `plan_id` | Planning workspace to validate |

Returns pass or fail with a list of structural violations (forbidden dependencies, banned symbols, layer bypasses). Use after declaring planned symbols and before implementation.

### `get_plan_details`

Returns the virtual subgraph of the plan and implementation status.

Use before review to compare intent with result.

Post-facto compliance checking is performed by reviewing plan details, using `verification_plan`, or running `gm check --plan`. The core library exposes `verify_plan` for embedded harnesses.

## Write tools

Write tools should be used carefully. Do not write guesses into the graph.

### `add_node`

Adds an architectural concept, rationale node, external system, or manually verified entity.

### `add_edge`

Adds a verified relationship.

Only use `EXTRACTED` confidence when the relationship was confirmed by source inspection.

### `remove_edge`

Removes a false positive or stale relationship.

### `recluster`

Re-runs community detection after meaningful manual edits.

### `reload_graph`

Hot-swaps the in-memory graph from a `graph.json` file without restarting the MCP server.

| Parameter | Purpose |
|---|---|
| `graph_path` | Optional path to a graph file; defaults to the path the server was launched with |

Use when:

- picking up changes after `gm run . --no-semantic --no-viz`
- pointing the server at a different repository's graph
- refreshing after the file watcher reloads on disk

The response includes node and edge counts. If the reloaded file is stale relative to source or binary, a warning is appended (same logic as `graph_info`).

## Diff tools

### `diff_graph`

Compares two graph files and returns added or removed nodes and edges.

### `review_plan`

Generates a full review plan from graph diffs.

Use when preparing pull request review for agent-authored changes.

## Tool selection guide

| Goal | Best first tool |
|---|---|
| Confirm graph identity | `graph_info` |
| Refresh graph after rebuild | `reload_graph` |
| Understand repository shape | `architecture_summary` |
| Find code related to a feature | `query_graph` |
| Understand a symbol | `analyse_symbol` |
| Find direct callers or dependencies | `get_neighbors` |
| Find multi-hop impact | `query_transitive` |
| Explain connection between two symbols | `shortest_path` and `safest_path` |
| Check graph trust quality | `resolution_report` |
| Plan verification after editing | `verification_plan` |
| Measure downstream impact | `blast_radius` |
| Review a changed graph | `what_changed` or `review_plan` |
| Enforce trust policy | `agent_change_gate` |
| Validate plan before coding | `validate_plan` |
| Run custom logic | `run_datalog` |

## Output interpretation

Treat Graphenium output as a map, not the territory.

| Evidence | Agent action |
|---|---|
| `EXTRACTED` and resolved | Safe to plan against, still read source before editing |
| `INFERRED` | Strong lead, verify at least one source file |
| `AMBIGUOUS` | Do not act until inspected |
| unresolved | Investigate missing symbol, dynamic code, generated code, or ignore rules |
