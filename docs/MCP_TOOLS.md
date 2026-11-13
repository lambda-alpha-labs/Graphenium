# MCP Tool Catalog and Containment Protocol

Graphenium exposes a Model Context Protocol (MCP) interface with 12 consolidated tools. The surface is deliberately small and cohesive: each tool combines related read, trace, planning, verification, or mutation operations behind explicit parameters and modes. This keeps tool selection unambiguous for agents while preserving every backend capability.

Every parameter is documented with a JSON schema description generated from Rust source. No exposed tool parameter is left undocumented.

---

## 1. Operating Rules of Thumb for Agents

Before modifying code, agents must follow this strict tool sequence:

```text
Handshake and health   -> get_graph_info
                             |
                             v
Orientation            -> get_architecture_summary
                             |
                             v
Symbol inspection      -> inspect_symbol / query_graph / trace_path
                             |
                             v
Pre-flight design      -> create_planning_workspace
                             |
                             v
Declare intent         -> add_planned_symbol
                             |
                             v
Policy and delta gate  -> validate_plan
                             |
                             v (passes)
Implementation         -> edit files locally
                             |
                             v
Compliance audit       -> verify_changes
```

---

## 2. Group A: Orientation and Health

### `get_graph_info`

Combines `graph_info`, `graph_stats`, `unresolved_references`, `resolution_report`, and `ambiguous_symbols`.

**Purpose:** Handshake and health check. Always returns project root, schema version, build timestamp, extraction modes, source languages, node/edge/hyperedge/community counts, active policy gates, graph trust mode, the index path when known, and a Lean trust summary (sorry gaps, `native_decide` shortcuts, external axiom edges) whenever semantic `#print axioms` facts are present.

**Optional flags:**

| Parameter | Description |
|---|---|
| `include_stats` | Append scoped statistics: node and edge counts, node-type breakdown, edge confidence distribution, and extractor provenance. |
| `include_unresolved` | Append imports whose targets are missing from the graph. |
| `include_resolution` | Append import, call, and method resolution rates plus ambiguous edge counts. |
| `include_ambiguous` | Append ambiguous edges and cross-file label collisions. |
| `path_prefix` | Optional path prefix or fragment to include in statistics. |
| `exclude_path` | Optional path prefix or fragment to exclude from statistics. |
| `node_types` | Optional file types to include in statistics. |
| `generated_code_mode` | Generated-like code filter: `include`, `exclude`, or `only`. |
| `ast_only_tuning` | AST-only noise suppression override. Omitted means automatic. |

**Staleness guard:** Automatically warns when physical source files or the Graphenium binary are newer than the cached index. If stale, run `gm run` and then `reload_graph`.

**When to use:** At the start of every session, or after a branch checkout.

### `get_architecture_summary`

Combines `architecture_summary`, `get_community`, and `god_nodes`.

**Purpose:** Repository-level architectural orientation. Returns major communities, cross-community connectors, boundary-heavy communities, and architectural hotspots.

**Parameters:**

| Parameter | Description |
|---|---|
| `community_id` | Optional community ID. When set, the response focuses on that community instead of the repository overview. |
| `include_community_members` | Append the full member list for the focused community. |
| `include_god_nodes` | Append the top hub nodes after the summary. |
| `god_node_limit` | Number of hub nodes to return. Default 10, maximum 50. |
| `path_prefix` | Optional path prefix or fragment to include. |
| `exclude_path` | Optional path prefix or fragment to exclude. |
| `node_types` | Optional file types to include. |
| `generated_code_mode` | Generated-like code filter: `include`, `exclude`, or `only`. |
| `ast_only_tuning` | AST-only noise suppression override. |
| `max_communities` | Maximum number of communities in the overview. Default 5, maximum 10. |

**When to use:** When the agent needs a global structural map before drilling into files.

### `reload_graph`

Hot-swaps the in-memory index from a local `graph.json` file without restarting the MCP server.

**Parameters:**

| Parameter | Description |
|---|---|
| `graph_path` | Optional filesystem path to a `graph.json` file. If omitted, reloads from the path the server was launched with. |

**When to use:** After running `gm run` locally to sync the server with physical file edits.

---

## 3. Group B: Search, Inspect, and Trace

### `query_graph`

Executes structural queries radiating outward from keyword-matched seed nodes.

**Parameters:**

| Parameter | Description |
|---|---|
| `keywords` | Keywords or phrase to search for. |
| `depth` | Traversal depth, 1 to 6. Default 3. |
| `budget` | Approximate output token budget. Default 2000. |
| `dfs` | Use depth-first search instead of BFS. |
| `path_prefix` | Optional path prefix or fragment to include. |
| `exclude_path` | Optional path prefix or fragment to exclude. |
| `node_types` | Optional file types to include. |
| `include_relations` | Optional relation filters to include during traversal. |
| `exclude_relations` | Optional relation filters to exclude during traversal. |
| `generated_code_mode` | Generated-like code filter: `include`, `exclude`, or `only`. |
| `ast_only_tuning` | AST-only noise suppression override. |
| `include_tests` | Include test and spec nodes. Default false. |
| `min_degree` | Minimum node degree to include. |

**When to use:** To locate code symbols related to a feature, folder, or API surface.

### `inspect_symbol`

Combines `get_node`, `get_neighbors`, `references_to`, `analyse_symbol`, `explain_change`, and `summarize_file`.

**Purpose:** Single-call symbol inspection. Always returns canonical node details, then appends the requested sections.

**Parameters:**

| Parameter | Description |
|---|---|
| `target` | Node ID or label to inspect. When `include_file_summary` is set, a file path or filename suffix is also accepted. |
| `include_neighbors` | Append direct neighbors with relation types, confidence, and scores. |
| `include_callers` | Append structural references: containers, imports, inheritance, and implementations. |
| `include_analysis` | Append behavioral and structural dependency analysis with a confidence summary. |
| `include_file_summary` | Append every symbol extracted from the target's source file. |
| `include_explanation` | Append hierarchy, community context, entry points, must-read files, and test scaffolding. |
| `relation` | Optional substring used to filter neighbors by relation type. |
| `max_neighbors` | Maximum number of neighbors to return. Default 50. |
| `extracted_only` | Show only EXTRACTED confidence neighbors. |
| `group_by` | File summary grouping: `kind` or `community`. |
| `show_leaves` | Include low-degree leaf symbols of degree 5 or less. |

**When to use:** Before modifying a public API, or when disambiguating identifier collisions.

### `trace_path`

Combines `shortest_path`, `safest_path`, `query_transitive`, and `module_dependencies`.

**Purpose:** Unified connectivity tracing. The `mode` parameter selects the backend behavior while shared filters remain available.

**Parameters:**

| Parameter | Description |
|---|---|
| `source` | Starting node ID or label. For `module_dependencies`, the source module or directory fragment. |
| `target` | Destination node ID or label. Required for `shortest`, `safest`, and `module_dependencies`. |
| `mode` | Routing mode: `shortest` (default), `safest`, `transitive_forward`, `transitive_reverse`, or `module_dependencies`. |
| `depth` | Maximum traversal depth for transitive modes. Default 3, maximum 6. |
| `direction` | Optional transitive direction override: `forward`, `reverse`, or `both`. |
| `budget` | Approximate output character budget for transitive modes. Default 2000. |
| `path_prefix` | Optional path prefix or fragment to include for shortest paths. |
| `exclude_path` | Optional path prefix or fragment to exclude for shortest paths. |
| `node_types` | Optional file types to include for shortest paths. |
| `include_relations` | Optional relation filters to include for shortest paths. |
| `exclude_relations` | Optional relation filters to exclude for shortest paths. |
| `path_mode` | Shortest path strategy: `semantic` (default) or `strict`. |
| `exclude_framework_noise` | Exclude framework and import-noise bridge nodes. |
| `generated_code_mode` | Generated-like code filter for shortest paths. |
| `ast_only_tuning` | AST-only noise suppression override for shortest paths. |

**Mode behavior:**

- `shortest`: semantic or strict path between `source` and `target`.
- `safest`: path with the highest aggregate confidence profile.
- `transitive_forward`: outgoing reachability closure from `source`.
- `transitive_reverse`: incoming reachability closure from `source`.
- `module_dependencies`: dependency edges between two module path fragments.

**When to use:** To verify dependency chains, map blast radius, or audit cross-module contracts.

### `run_datalog`

Executes a declarative first-order logic program over the compiled EDB.

**Standard library predicates:** `calls_transitive`, `imports_transitive`, `depends_transitive`, `same_community`, `is_hub`, `is_orphan`, `circular_dependency`, `bypasses_layer`, `contains_transitive`, `aliased_container_call`, `unreachable_from`, `has_proof_gap`, `tainted_by_proof_gap`, `uses_axiom`, `has_untrusted_axiom`, `has_native_decide`, and `untrusted_theorem`.

**Syntax:** bodies accept `A != B` / `A /= B` (variables must be bound by a positive atom), and a program may contain multiple `?-` blocks, optionally labelled: `?- [proof_gaps] has_proof_gap(X).`

**Parameters:**

| Parameter | Description |
|---|---|
| `query` | Datalog program text. Supports goals (`?-`), user rules (`:-`), facts, negation (`not`), inequality (`!=`), and multiple labelled queries. |
| `step_budget` | Maximum solver steps. Default 1000, clamped to 100 through 10000. |
| `format` | `text` (default, Markdown) or `json` (`datalog.v1` report with per-query rows, variables, and step metadata). |

**When to use:** To mathematically prove transitive boundary violations, circular dependencies, or orphan nodes.

---

## 4. Group C: Spec-Driven Planning and Containment

### `create_planning_workspace`

Initializes a virtual in-memory planning workspace.

**Parameters:**

| Parameter | Description |
|---|---|
| `plan_id` | Unique identifier for the planning workspace. |
| `description` | Human-readable description of the plan's purpose. |

**When to use:** When starting any multi-file task or refactoring ticket.

### `add_planned_symbol`

Registers an intended symbol and links it to an existing node in the workspace.

**Parameters:**

| Parameter | Description |
|---|---|
| `plan_id` | Planning workspace ID. |
| `label` | Label for the new planned symbol. |
| `file_path` | File path where the symbol will be added. |
| `relation` | Relation type such as `calls`, `depends_on`, or `implements`. |
| `target_symbol` | ID or label of the existing node to link to. |

**Automated gating:** If `.graphenium/policy.json` is present, this tool runs a pre-flight policy solve. A violation returns `PRE_FLIGHT_VIOLATION` and blocks registration.

**When to use:** To declare the architectural design the agent intends to write.

### `validate_plan`

Combines `validate_plan` and `evaluate_delta_gate`.

**Purpose:** Pre-flight design validation. Always runs the Louvain modularity delta gate and surprise analysis. Unless `skip_policy_rules` is true, it also applies explicit `.graphenium/policy.json` rules.

**Parameters:**

| Parameter | Description |
|---|---|
| `plan_id` | Planning workspace identifier. |
| `skip_policy_rules` | Skip explicit policy rules and run only the delta gate. Default false. |
| `modularity_tolerance` | Maximum allowed modularity drop. Default -0.02. |
| `surprise_threshold` | Surprise score threshold for planned edges. Default 5.0. |

**Pass criteria:** Delta Q is at or above `modularity_tolerance`, no planned edge exceeds `surprise_threshold`, and explicit policy rules pass when enabled.

**Response format:**

```markdown
### Topological Delta Report for Plan: `refactor-session-handling`

- **Status:** FAILED
- **Modularity Delta (Delta Q):** -0.0341 (Baseline: 0.3841 -> Virtual: 0.3500)

#### High-Surprise Edges Proposed:
- `plan:UserView` -> `src/db/helper` (Confidence: 6.5)
  *Reason:* cross-community, peripheral-to-hub

#### Structural Drift Warnings:
- **Cross-boundary edges**: cross-boundary edges increased: 42 -> 45
```

**Interpretation:**

- `PASSED`: The plan preserves modularity and introduces no high-surprise edges. Proceed.
- `FAILED` with negative Delta Q: The plan degrades community cohesion. Re-route dependencies through existing domain services.
- `FAILED` with high-surprise edges: Inspect each entry and decouple the planned edge across community boundaries.
- Drift warnings: Informational. Review alongside surprise edges.

**When to use:** After completing the virtual plan and before editing source files.

### `verify_changes`

Combines `agent_change_gate`, `get_plan_details`, `verification_plan`, `blast_radius`, `next_files_to_read`, `review_plan`, `what_changed`, and `diff_graph`.

**Purpose:** Unified post-edit verification report. Returns the scope creep audit, blast radius, verification checklist, and must-read files in one response.

**Parameters:**

| Parameter | Description |
|---|---|
| `plan_id` | Optional planning workspace ID. Adds pre-flight policy checks, post-edit compliance, and the full planned subgraph. |
| `target_symbols` | Optional list of changed node IDs or labels. Entries may be exact IDs or case-insensitive labels. |
| `min_resolution` | Optional minimum import resolution percentage for the change gate. Default 80.0. |
| `max_ambiguous` | Optional maximum allowed ambiguous edges for the change gate. Default 10. |
| `snapshot_name` | Optional snapshot name. Appends a risk-sorted what-changed report. |
| `before_graph_path` | Optional path to a before `graph.json` file. |
| `after_graph_path` | Optional path to an after `graph.json` file. |

**Included sections:**

- Agent change gate with policy gate pass/fail table.
- Blast radius with affected files, communities, and edge confidence distribution.
- Verification plan with must-read files, tests, and risk gates.
- Next files to read, deduplicated by file.
- Plan details and post-edit compliance when `plan_id` is set.
- What-changed, graph diff, and review plan sections when the optional inputs are supplied.

**When to use:** Before committing, opening a PR, or completing an agent session.

---

## 5. Group D: Manual Mutations

### `mutate_graph`

Combines `add_node`, `add_edge`, `remove_edge`, and `recluster`.

**Purpose:** Single entry point for manual index mutations. Use sparingly and only after direct source inspection. Successful mutations persist to disk immediately.

**Parameters:**

| Parameter | Description |
|---|---|
| `action` | Mutation action: `add_node`, `add_edge`, `remove_edge`, or `recluster`. |
| `id` | Node identifier for `add_node`. |
| `label` | Display name for `add_node`. |
| `file_type` | Node type for `add_node`: `code`, `document`, `paper`, `image`, or `rationale`. |
| `source_file` | Relative source file for `add_node`, or the file where an `add_edge` relationship was observed. |
| `source_location` | Optional source location hint such as `L42`. |
| `qualified_label` | Optional qualified scope-prefixed label for `add_node`. |
| `source` | Source node ID or label for `add_edge` and `remove_edge`. |
| `target` | Target node ID or label for `add_edge` and `remove_edge`. |
| `relation` | Relation type for `add_edge`, or optional relation filter for `remove_edge`. |
| `confidence` | Edge confidence for `add_edge`: `EXTRACTED`, `INFERRED`, or `AMBIGUOUS`. |
| `weight` | Optional traversal weight override for `add_edge`, clamped to 0.0 through 1.0. |

**Safety rule:** Only write with `EXTRACTED` confidence when the connection has been proven by direct file-level code review.

**When to use:** To register verified concepts, record confirmed relationships, remove false positives, or recluster after manual overrides.

---

## 6. Migration Map

| Previous tool | Consolidated tool | Routing |
|---|---|---|
| `graph_info` | `get_graph_info` | Always included |
| `graph_stats` | `get_graph_info` | `include_stats=true` |
| `unresolved_references` | `get_graph_info` | `include_unresolved=true` |
| `resolution_report` | `get_graph_info` | `include_resolution=true` |
| `ambiguous_symbols` | `get_graph_info` | `include_ambiguous=true` |
| `architecture_summary` | `get_architecture_summary` | Omit `community_id` |
| `get_community` | `get_architecture_summary` | Set `community_id` |
| `god_nodes` | `get_architecture_summary` | `include_god_nodes=true` |
| `get_node` | `inspect_symbol` | Always included |
| `get_neighbors` | `inspect_symbol` | `include_neighbors=true` |
| `references_to` | `inspect_symbol` | `include_callers=true` |
| `analyse_symbol` | `inspect_symbol` | `include_analysis=true` |
| `explain_change` | `inspect_symbol` | `include_explanation=true` |
| `summarize_file` | `inspect_symbol` | `include_file_summary=true` |
| `shortest_path` | `trace_path` | `mode=shortest` |
| `safest_path` | `trace_path` | `mode=safest` |
| `query_transitive` | `trace_path` | `mode=transitive_forward` or `transitive_reverse` |
| `module_dependencies` | `trace_path` | `mode=module_dependencies` |
| `validate_plan` | `validate_plan` | Always runs the delta gate |
| `evaluate_delta_gate` | `validate_plan` | `skip_policy_rules=true` |
| `agent_change_gate` | `verify_changes` | Always included |
| `blast_radius` | `verify_changes` | When `target_symbols` is set |
| `verification_plan` | `verify_changes` | When `target_symbols` is set |
| `next_files_to_read` | `verify_changes` | When `target_symbols` is set |
| `get_plan_details` | `verify_changes` | When `plan_id` is set |
| `what_changed` | `verify_changes` | When `snapshot_name` is set |
| `diff_graph` | `verify_changes` | When both graph paths are set |
| `review_plan` | `verify_changes` | When a graph path is set |
| `add_node` | `mutate_graph` | `action=add_node` |
| `add_edge` | `mutate_graph` | `action=add_edge` |
| `remove_edge` | `mutate_graph` | `action=remove_edge` |
| `recluster` | `mutate_graph` | `action=recluster` |
