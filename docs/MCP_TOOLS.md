# Graphenium MCP Tool Reference

This document describes every MCP tool exposed by the Graphenium server. Tools are
grouped by their primary role: **Read**, **Composite**, **Trust**, **Write**, and **Diff**.

---

## 1. Read Tools

Read-only tools that inspect the loaded knowledge graph without modifying it.

---

### `graph_stats`

Return summary statistics for the loaded knowledge graph: node/edge/hyperedge counts,
number of communities, node-type breakdown, and edge-confidence distribution.

**Key parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `path_prefix` | `Option<String>` | Include only nodes whose source file starts with this path prefix (case-insensitive) |
| `exclude_path` | `Option<String>` | Exclude nodes whose source file contains this path fragment (case-insensitive) |
| `node_types` | `Option<Vec<String>>` | Filter to specific file types, e.g. `["code", "document", "rationale"]` |
| `generated_code_mode` | `Option<String>` | `"include"` (default), `"exclude"`, or `"only"` generated/template/vendor paths |
| `ast_only_tuning` | `Option<bool>` | Enable AST-only noise suppression; auto-detects from graph metadata |

**Example**

```json
{
  "name": "graph_stats",
  "arguments": {}
}
```

**When to use** — Start here when you first connect to a server. Gives you the
high-level shape of the graph before deciding which nodes or communities to explore.

---

### `architecture_summary`

Return a repository-level architectural summary with major communities,
cross-community connectors, and architectural hotspots.

**Key parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `path_prefix` | `Option<String>` | Include only nodes under this path prefix |
| `exclude_path` | `Option<String>` | Exclude nodes under this path fragment |
| `node_types` | `Option<Vec<String>>` | Filter to specific file types |
| `generated_code_mode` | `Option<String>` | `"include"`, `"exclude"`, or `"only"` |
| `ast_only_tuning` | `Option<bool>` | Enable AST-only noise suppression |
| `max_communities` | `Option<i32>` | Max communities to summarize (default 5, max 10) |

**Example**

```json
{
  "name": "architecture_summary",
  "arguments": {
    "max_communities": 5
  }
}
```

**When to use** — Forked before reading any files in an unfamiliar repository.
Gives a bird's-eye view of the codebase decomposition into logical communities,
inter-community connectors (architectural hotspots), and dominant relation types.

---

### `query_graph`

Query the knowledge graph with keywords. Scores nodes by keyword match and traverses
the graph via BFS (default) or DFS. Returns matching nodes and their connections
formatted as Markdown.

**Key parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `keywords` | `String` | **Required.** Space-separated keywords to search for |
| `depth` | `Option<i32>` | Traversal depth (1–6, default 3) |
| `budget` | `Option<i32>` | Approximate output token budget (default 2000) |
| `dfs` | `Option<bool>` | Use depth-first search instead of BFS (default false) |
| `path_prefix` | `Option<String>` | Include only nodes under this path |
| `exclude_path` | `Option<String>` | Exclude nodes under this path |
| `node_types` | `Option<Vec<String>>` | Filter by file type |
| `include_relations` | `Option<Vec<String>>` | Only show these relation types, e.g. `["calls", "uses"]` |
| `exclude_relations` | `Option<Vec<String>>` | Hide these relation types, e.g. `["imports"]` |
| `generated_code_mode` | `Option<String>` | `"include"`, `"exclude"`, or `"only"` |
| `ast_only_tuning` | `Option<bool>` | Enable AST-only noise suppression |
| `include_tests` | `Option<bool>` | Include test/spec nodes (default false) |
| `min_degree` | `Option<i32>` | Minimum node degree to include (filters low-degree noise) |

**Example**

```json
{
  "name": "query_graph",
  "arguments": {
    "keywords": "authentication middleware",
    "depth": 3,
    "include_relations": ["calls", "uses"]
  }
}
```

**When to use** — Your primary search tool. Use it when you have a conceptual
target (a feature name, type name, error message) and want to find related nodes.
Prefer `summarize_file` when you already know the file path; prefer `get_node`
when you already have the exact symbol name.

---

### `get_node`

Get full details for a node by ID or label (case-insensitive). Returns the node's
label, file type, source file, source span/location, community assignment, and degree.

**Key parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | `String` | **Required.** Node ID or label to look up |

**Example**

```json
{
  "name": "get_node",
  "arguments": { "id": "auth_service" }
}
```

**When to use** — You already know (or suspect) a node ID or label and want
its metadata, file location, and community membership. Does not show neighbors —
use `get_neighbors` for that.

---

### `get_neighbors`

Get all direct neighbors of a node, including edge relation types, confidence
levels, and scores. An optional relation filter narrows results to edges whose
relation name contains the given substring.

**Key parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `node_id` | `String` | **Required.** Node ID or label to query |
| `relation` | `Option<String>` | Substring filter, e.g. `"calls"` or `"imports"` |
| `max_neighbors` | `Option<i32>` | Max neighbors to return (default 50, cap output for hub nodes) |
| `extracted_only` | `Option<bool>` | Show only EXTRACTED-confidence edges (source-backed ground truth) |

**Example**

```json
{
  "name": "get_neighbors",
  "arguments": {
    "node_id": "auth_service",
    "relation": "calls",
    "max_neighbors": 20
  }
}
```

**When to use** — After `get_node` or `query_graph` has identified an interesting
node, use `get_neighbors` to explore its immediate connections. The `relation`
filter lets you zoom in on behavioural edges (`calls`, `uses`) vs. structural ones
(`imports`, `contains`). For multi-hop exploration use `query_transitive`.

---

### `get_community`

Summarize a community by its integer community ID. Returns representative nodes,
files, and dominant relations. Set `include_members` to `true` to append the full
member list.

**Key parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `community_id` | `i32` | **Required.** Integer community ID (0-indexed, community 0 is the largest) |
| `include_members` | `Option<bool>` | Append the full member list after the summary (default false) |

**Example**

```json
{
  "name": "get_community",
  "arguments": {
    "community_id": 0,
    "include_members": true
  }
}
```

**When to use** — After `architecture_summary` or `graph_stats` shows a community
that looks relevant, drill into it with `get_community`. Community 0 is typically
the largest logical grouping (e.g. core framework code). Use `include_members: true`
sparingly — it can produce long output for large communities.

---

### `god_nodes`

Return the top N most connected nodes ("god nodes" or hubs) in the graph.
File-level hubs and stub nodes (degree ≤ 1) are filtered out. Useful for finding
architectural hotspots.

**Key parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `n` | `Option<i32>` | Number of hub nodes to return (default 10, max 50) |
| `path_prefix` | `Option<String>` | Scope to a specific path prefix |
| `exclude_path` | `Option<String>` | Exclude a path fragment |
| `node_types` | `Option<Vec<String>>` | Filter by file type |
| `generated_code_mode` | `Option<String>` | `"include"`, `"exclude"`, or `"only"` |
| `ast_only_tuning` | `Option<bool>` | Enable AST-only noise suppression |

**Example**

```json
{
  "name": "god_nodes",
  "arguments": { "n": 15 }
}
```

**When to use** — Complement to `architecture_summary`. While `architecture_summary`
shows community-level grouping, `god_nodes` shows the individual symbols with the
highest connectivity — these are the integration points, controllers, and central
types you must understand first.

---

### `shortest_path`

Find a path between two nodes. By default, semantic mode prefers calls/uses/contains-
style relationships over imports. Set `mode='strict'` for the exact fewest-hop path.
Accepts node IDs or labels (case-insensitive).

**Key parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `from` | `String` | **Required.** Starting node ID or label |
| `to` | `String` | **Required.** Destination node ID or label |
| `path_prefix` | `Option<String>` | Include only nodes under this path |
| `exclude_path` | `Option<String>` | Exclude nodes under this path |
| `node_types` | `Option<Vec<String>>` | Filter by file type |
| `include_relations` | `Option<Vec<String>>` | Only follow these relation types through the path |
| `exclude_relations` | `Option<Vec<String>>` | Skip these relation types during traversal |
| `mode` | `Option<String>` | `"semantic"` (default) or `"strict"` |
| `exclude_framework_noise` | `Option<bool>` | Exclude framework/import-noise bridge nodes (default false) |
| `generated_code_mode` | `Option<String>` | `"include"`, `"exclude"`, or `"only"` |
| `ast_only_tuning` | `Option<bool>` | Enable AST-only noise suppression |

**Example**

```json
{
  "name": "shortest_path",
  "arguments": {
    "from": "auth_service",
    "to": "database",
    "mode": "semantic"
  }
}
```

**When to use** — When you need to understand *how* two known symbols are connected.
Semantic mode (default) is best for understanding actual runtime/behavioural paths;
strict mode is for minimal-hop dependency chains. Use `safest_path` when you care
more about confidence than hop count.

---

### `safest_path`

Find the safest path between two nodes. Prefers edges with highest confidence and
resolution status over shortest hop count. Returns both the path and a safety score
(0.0–1.0).

**Key parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `from` | `String` | **Required.** Starting node ID or label |
| `to` | `String` | **Required.** Destination node ID or label |

**Example**

```json
{
  "name": "safest_path",
  "arguments": {
    "from": "auth_service",
    "to": "database"
  }
}
```

**When to use** — Same scenario as `shortest_path`, but you care more about
traversing only well-established (extracted/high-confidence) edges. Useful when
you are using the result as evidence for a code review decision and need to avoid
heuristic or ambiguous edges.

---

### `query_transitive`

Multi-turn transitive query: starting from a seed symbol, follow edges outward
through successive hops and return the full transitive closure. Useful for finding
all nodes reachable from a given symbol.

**Key parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `seed` | `String` | **Required.** Starting node ID or label |
| `depth` | `Option<i32>` | Maximum traversal depth (default 3, max 6) |
| `relation` | `Option<String>` | Only follow edges whose relation name contains this substring |
| `direction` | `Option<String>` | `"forward"` (default, outgoing), `"reverse"` (incoming), or `"both"` |

**Example**

```json
{
  "name": "query_transitive",
  "arguments": {
    "seed": "main",
    "depth": 4,
    "direction": "forward",
    "relation": "calls"
  }
}
```

**When to use** — When you need to discover the full reachable dependency graph
from a single starting point. `get_neighbors` gives you one hop; `query_transitive`
gives you N hops. Use `direction: "reverse"` to find all nodes that depend on the
seed (incoming edges), `direction: "forward"` for what the seed depends on.

---

### `summarize_file`

List all graph symbols extracted from a given source file. The path is matched
case-insensitively as a suffix against node source files, so you can pass either
a full path or just the filename. Symbols are grouped by node kind (default) or
by community.

**Key parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `path` | `String` | **Required.** File path (full or suffix). Case-insensitive. |
| `group_by` | `Option<String>` | Grouping: `"kind"` (default) or `"community"` |
| `min_degree` | `Option<i32>` | Minimum node degree to include |
| `show_leaves` | `Option<bool>` | Show low-degree leaf symbols (degree ≤ 5, default false) |

**Example**

```json
{
  "name": "summarize_file",
  "arguments": {
    "path": "src/auth/service.rs",
    "show_leaves": true
  }
}
```

**When to use** — The fastest way to answer "what's in this file?" without reading
the file itself. You already know the file path (from `get_node`, `blast_radius`,
or project knowledge) and want to see all its extracted functions, types, and
constants with their degrees and community membership.

---

### `graph_info`

Return metadata about the currently loaded graph: project root, schema version,
build timestamp, extraction mode, languages, and node/edge counts.

**Key parameters**

None.

**Example**

```json
{
  "name": "graph_info",
  "arguments": {}
}
```

**When to use** — Confirm which graph the server is serving, check what language(s)
were extracted, or verify the extraction mode before acting on results. Also useful
after `reload_graph` to confirm the new graph loaded correctly.

---

## 2. Composite Tools

Tools that combine multiple analysis passes into a single response.

---

### `analyse_symbol`

Single-turn composite analysis of a symbol. Returns node metadata, behavioral
connections (`calls`, `uses`, `inherits`, `implements`), and structural connections
(`imports`, `contains`) with confidence summaries. Prioritizes behavioral
dependencies over structural ones.

**Key parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `symbol` | `String` | **Required.** Node ID or label to analyse |

**Example**

```json
{
  "name": "analyse_symbol",
  "arguments": { "symbol": "auth_service" }
}
```

**When to use** — The quickest way to understand a symbol in context. It combines
what you would get from three separate calls (`get_node` + `get_neighbors` twice)
into one structured response with behavioural/structural separation and a trust
profile. Use this before deciding which files to read or which other symbols to
trace.

---

### `module_dependencies`

Show dependency connections between two modules/directories. Iterates over all
edges and groups them by modules containing the given path fragments.

**Key parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `module_a` | `String` | **Required.** Source module/directory path fragment |
| `module_b` | `String` | **Required.** Target module/directory path fragment |

**Example**

```json
{
  "name": "module_dependencies",
  "arguments": {
    "module_a": "src/auth",
    "module_b": "src/db"
  }
}
```

**When to use** — When you want to understand the dependency relationship between
two directories at a macro level. Unlike `query_graph` (which is keyword-centric)
or `analyse_symbol` (which is single-symbol-centric), `module_dependencies` groups
connections by relation type across all symbols in two directory trees.

---

### `what_changed`

Compare the current graph against a stored snapshot. Returns a risk-sorted delta
with removed symbols (highest risk), community moves, added symbols, and downstream
impact analysis.

**Key parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `snapshot_name` | `Option<String>` | Snapshot name (default: `"backup"`) |

**Example**

```json
{
  "name": "what_changed",
  "arguments": {}
}
```

**When to use** — After re-running `gm run` or `reload_graph`, call `what_changed`
to see what symbols were added, removed, or moved between communities since the
last snapshot. The risk-sorted output helps prioritise review of breaking changes.

---

## 3. Trust Tools

Tools that assess graph quality, surface low-confidence edges, and plan
verification work.

---

### `resolution_report`

Return a resolution-quality report for the loaded graph. Shows import resolution,
call resolution, ambiguous edges, and unresolved references.

**Key parameters**

None.

**Example**

```json
{
  "name": "resolution_report",
  "arguments": {}
}
```

**When to use** — Before trusting the graph's results for decision-making. A low
resolution score or many ambiguous edges means you should verify findings manually
or use `get_neighbors` with `extracted_only: true`.

---

### `ambiguous_symbols`

List all ambiguous edges in the graph. Ambiguous edges have low confidence and
should be verified manually before being used as evidence for decisions.

**Key parameters**

None.

**Example**

```json
{
  "name": "ambiguous_symbols",
  "arguments": {}
}
```

**When to use** — When `resolution_report` shows ambiguous edges and you want
to investigate each one. Each entry shows the source label, relation, target
label, and source file so you can inspect the relevant code manually.

---

### `unresolved_references`

List all unresolved references (import edges where the target symbol was not found
in the graph). These represent potentially missing dependencies or incorrect
import paths.

**Key parameters**

None.

**Example**

```json
{
  "name": "unresolved_references",
  "arguments": {}
}
```

**When to use** — When the graph has missing symbols or broken import chains.
Unresolved references reduce the trustworthiness of transitive queries and
path-finding results.

---

### `verification_plan`

Build a verification plan for a set of changed nodes. Given node IDs for symbols
that have changed, returns a prioritized plan: must-read files, tests to run,
ambiguous edges to inspect, and risk gates.

**Key parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `changed_nodes` | `String` | **Required.** Comma-separated list of changed node IDs or labels |

**Example**

```json
{
  "name": "verification_plan",
  "arguments": {
    "changed_nodes": "auth_service, user_model, login_handler"
  }
}
```

**When to use** — After modifying code or the graph, call this to get a structured
verification checklist. The output tells you which files to read, which edges to
inspect, and which community boundaries you may have impacted.

---

### `next_files_to_read`

Return the "must-read" files from a verification plan for a set of changed nodes.
Each entry lists a file path and the reason it needs to be reviewed. Useful for
quickly seeing what files an agent should read next.

**Key parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `changed_nodes` | `String` | **Required.** Comma-separated list of changed node IDs or labels |

**Example**

```json
{
  "name": "next_files_to_read",
  "arguments": {
    "changed_nodes": "auth_service"
  }
}
```

**When to use** — Lighter version of `verification_plan` when you only need the
file list, not the full plan with test suggestions and risk gates. Optimised for
the common agent workflow: "what changed, what do I need to read?"

---

### `blast_radius`

Compute the blast radius (downstream impact) for a set of changed symbols. Shows
affected files, communities, and edge confidence distribution.

**Key parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `changed_nodes` | `String` | **Required.** Comma-separated list of changed node IDs or labels |

**Example**

```json
{
  "name": "blast_radius",
  "arguments": {
    "changed_nodes": "auth_service, login_handler"
  }
}
```

**When to use** — When you have a set of (proposed or actual) changes and need
to assess how many files and communities they touch. Unlike `verification_plan`
(which tells you *what to do*), `blast_radius` tells you *what is affected*.

---

### `agent_change_gate`

Evaluate policy gates for a set of changed nodes. Builds a resolution report from
the current graph, evaluates default policies (MinResolution, MaxAmbiguous, etc.)
with optional threshold overrides, and returns a Markdown table of pass/fail for
each gate.

**Key parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `changed_nodes` | `String` | **Required.** Comma-separated list of changed node IDs or labels |
| `min_resolution` | `Option<f64>` | Override: minimum import resolution percentage (default 80.0) |
| `max_ambiguous` | `Option<usize>` | Override: maximum allowed ambiguous edges (default 10) |

**Example**

```json
{
  "name": "agent_change_gate",
  "arguments": {
    "changed_nodes": "auth_service",
    "min_resolution": 90.0,
    "max_ambiguous": 5
  }
}
```

**When to use** — At the end of an agent workflow, before declaring changes
complete. The gate evaluates whether the graph meets quality thresholds. A single
❌ FAIL means the agent should inspect ambiguous edges, improve resolution, or
re-run extraction before proceeding.

---

## 4. Write Tools

Tools that mutate the in-memory graph and persist it to disk immediately.

---

### `add_node`

Add or update a node in the knowledge graph. Use this to register architectural
concepts, rationale nodes, or other logical entities the AST extractor does not
capture. If a node with the given ID already exists, it is updated in place.
The graph is persisted to disk immediately.

**Key parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | `String` | **Required.** Stable node identifier (normalized automatically) |
| `label` | `String` | **Required.** Human-readable display name |
| `file_type` | `String` | **Required.** One of: `"code"`, `"document"`, `"paper"`, `"image"`, `"rationale"` |
| `source_file` | `String` | **Required.** Relative source file path to associate with this node |
| `source_location` | `Option<String>` | Optional source location hint, e.g. `"L42"` |
| `qualified_label` | `Option<String>` | Optional qualified (scope-prefixed) label for disambiguation |

**Example**

```json
{
  "name": "add_node",
  "arguments": {
    "id": "arch_overview",
    "label": "Architecture Overview",
    "file_type": "rationale",
    "source_file": "docs/architecture.md",
    "source_location": "L1"
  }
}
```

**When to use** — When you need to record a design rationale, mark a documentation
file, or create a node that the AST extractor wouldn't find on its own. After
adding the node, use `add_edge` to connect it to related code symbols.

---

### `add_edge`

Add a directed edge between two existing nodes. Resolves endpoints by ID first,
then by case-insensitive label match. The graph is persisted to disk immediately.

**Key parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `source` | `String` | **Required.** Source node ID or label |
| `target` | `String` | **Required.** Target node ID or label |
| `relation` | `String` | **Required.** Relation type, e.g. `"calls"`, `"uses"`, `"delegates_to"`, `"rationale_for"` |
| `confidence` | `String` | **Required.** `"EXTRACTED"`, `"INFERRED"`, or `"AMBIGUOUS"` |
| `source_file` | `String` | **Required.** Source file where this relationship was observed |
| `source_location` | `Option<String>` | Optional source location hint, e.g. `"L72"` |
| `weight` | `Option<f64>` | Optional traversal weight override (defaults to confidence-based) |

**Example**

```json
{
  "name": "add_edge",
  "arguments": {
    "source": "arch_overview",
    "target": "auth_service",
    "relation": "rationale_for",
    "confidence": "EXTRACTED",
    "source_file": "docs/architecture.md",
    "source_location": "L10"
  }
}
```

**When to use** — After adding a node, connect it to existing symbols to record
relationships the extractors don't capture (design rationale, cross-cutting
concerns, architectural decisions). Use `"EXTRACTED"` confidence for relationships
you have verified through code reading.

---

### `remove_edge`

Remove edges between two nodes. If a relation filter is provided, only matching
edges are removed; otherwise all edges between the two nodes are removed. The
graph is persisted to disk immediately.

**Key parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `source` | `String` | **Required.** Source node ID or label |
| `target` | `String` | **Required.** Target node ID or label |
| `relation` | `Option<String>` | Optional relation filter. If omitted, all edges are removed. |

**Example**

```json
{
  "name": "remove_edge",
  "arguments": {
    "source": "auth_service",
    "target": "old_dep",
    "relation": "imports"
  }
}
```

**When to use** — To correct false-positive edges or remove stale relationships
after refactoring. The optional `relation` filter lets you surgically remove only
one type of edge between two nodes without affecting other connections.

---

### `recluster`

Re-run community detection on the loaded graph. Communities are re-assigned based
on the current edge structure. Useful after adding nodes or edges via
`add_node` / `add_edge`.

**Key parameters**

None.

**Example**

```json
{
  "name": "recluster",
  "arguments": {}
}
```

**When to use** — After a batch of `add_node` and `add_edge` calls, run `recluster`
so community assignments reflect the new topology. Then call `get_community` or
`architecture_summary` to see how the new nodes integrated.

---

### `reload_graph`

Reload the knowledge graph from a `graph.json` file without restarting the MCP
server. If no path is given, reloads from the path the server was launched with.
Use this to point a running server at a different repository's graph, or to pick
up changes after re-running `gm run`.

**Key parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `graph_path` | `Option<String>` | Optional filesystem path to a `graph.json` file |

**Example**

```json
{
  "name": "reload_graph",
  "arguments": {
    "graph_path": "/path/to/other_repo/graph.json"
  }
}
```

**When to use** — After re-running `gm run` to pick up fresh extractions, or to
switch the server to a different repository's graph without a restart.

---

## 5. Diff Tools

Tools that compare two graph snapshots (on disk, not the loaded graph) to show
what changed between them.

---

### `diff_graph`

Compare two graph JSON files and show added/removed nodes and edges. Both paths
must point to valid `graph.json` files exported by Graphenium. Returns a summary
with counts and detailed listings.

**Key parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `before_graph` | `String` | **Required.** Filesystem path to the "before" `graph.json` file |
| `after_graph` | `String` | **Required.** Filesystem path to the "after" `graph.json` file |

**Example**

```json
{
  "name": "diff_graph",
  "arguments": {
    "before_graph": "graphenium-snapshots/backup.json",
    "after_graph": "graph.json"
  }
}
```

**When to use** — When you have two explicit graph JSON files on disk and want a
direct node-by-node, edge-by-edge comparison. This is an offline diff — it does
not use the currently loaded graph. Use `what_changed` if you want to diff the
loaded graph against a named snapshot.

---

### `review_plan`

Generate a complete review plan by diffing two graph snapshots (before and after)
and producing a verification plan. If `before_graph_path` is `None`, uses an
empty graph as the baseline. If `after_graph_path` is `None`, uses the currently
loaded graph. The result includes symbol inventory changes and the full
verification plan (must-read files, tests, edges to inspect, risk gates).

**Key parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `before_graph_path` | `Option<String>` | Optional path to the "before" `graph.json` file (defaults to empty graph) |
| `after_graph_path` | `Option<String>` | Optional path to the "after" `graph.json` file (defaults to currently loaded graph) |

**Example**

```json
{
  "name": "review_plan",
  "arguments": {
    "before_graph_path": "graphenium-snapshots/backup.json"
  }
}
```

**When to use** — End-to-end code review workflow: diff two graph snapshots and
immediately get a structured review plan with files to read, tests to run, and
risk assessments. Unlike `diff_graph` (raw diff) + `verification_plan` (two steps),
`review_plan` does both in one call.

---

## Tool Selection Guide

Use the following decision table to choose the right tool for your current goal.

| Your Goal | Recommended Tool | Why |
|-----------|-----------------|-----|
| **Orient on a new codebase** | `architecture_summary` | Bird's-eye view of communities and hotspots |
| **Get graph metadata / connection params** | `graph_info` | Project root, schema, languages, counts |
| **Check graph size and health** | `graph_stats` | Node/edge counts, type breakdown, confidence distribution |
| **Find symbols related to a concept** | `query_graph` | Keyword search with BFS/DFS traversal |
| **Get full details on a known symbol** | `get_node` | Metadata, file location, community, degree |
| **Explore a symbol's connections** | `get_neighbors` | Direct neighbors with relation types and confidence |
| **Multi-hop reachability from a seed** | `query_transitive` | Full transitive closure in a direction |
| **Understand a symbol in one call** | `analyse_symbol` | Metadata + behavioural + structural connections |
| **Path between two symbols (best hops)** | `shortest_path` | Semantic or strict shortest-path routing |
| **Path between two symbols (most trusted)** | `safest_path` | Highest-confidence edges, not fewest hops |
| **What's in a file?** | `summarize_file` | All symbols in a file by path suffix |
| **Drill into a community** | `get_community` | Community members and dominant relations |
| **Find architectural hotspots** | `god_nodes` | Highest-degree nodes (hubs) |
| **Module-to-module dependency analysis** | `module_dependencies` | Cross-module dependency grouping by relation type |
| **Assess graph quality** | `resolution_report` | Resolution percentages, ambiguous count |
| **List low-confidence edges** | `ambiguous_symbols` | Every ambiguous-confidence edge |
| **List broken references** | `unresolved_references` | Import edges with unresolved targets |
| **Get verification checklist for changes** | `verification_plan` | Files to read, tests to run, edges to inspect |
| **Get files to review (compact)** | `next_files_to_read` | Lighter version of verification_plan |
| **Assess impact of changes** | `blast_radius` | Affected files, communities, edge confidence |
| **Pre-merge policy gate** | `agent_change_gate` | Pass/fail table for resolution and ambiguity thresholds |
| **Add a new concept node** | `add_node` | Register architectural/rationale nodes |
| **Connect two nodes** | `add_edge` | Record a verified relationship |
| **Remove a bad edge** | `remove_edge` | Correct false positives |
| **Recompute communities after mutations** | `recluster` | Reassign communities based on new edges |
| **Load a different graph** | `reload_graph` | Switch to another `graph.json` without restart |
| **Diff two graph files on disk** | `diff_graph` | Raw node/edge-added/removed comparison |
| **Diff + full review plan** | `review_plan` | One-call diff + verification plan |
| **Diff loaded graph vs snapshot** | `what_changed` | Risk-sorted delta with downstream impact |
