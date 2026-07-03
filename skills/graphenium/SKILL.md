---
name: graphenium
description: Use when navigating a Graphenium knowledge graph: querying code structure, tracing dependencies, identifying hubs, or understanding module communities. Triggers on "what calls X", "what connects to Y", "what community", "shortest path", "god nodes", "hub nodes", "graph stats", or any structural question about the codebase that a knowledge graph can answer.
---

# Graphenium Skill

Graphenium is an active coordination and verification engine for the current
codebase. It runs `gm run` to build a trust-aware graph, then serves it via
MCP tools or the `gm query` CLI. Unlike passive search tools, Graphenium
closes the full agent lifecycle: pre-edit pathfinding, in-edit planning,
and post-edit compliance verification. This skill tells you which tool to
reach for and how to interpret the output.

## Session start: verify the loaded graph

The MCP server may be serving a graph from a **different project** than the
one you are currently working on. At the start of every session (and any time
you switch projects), call `graph_info` to confirm:

> Call `mcp_graphenium_graph_info()` and check that the **project root** or
> **graph source path** matches the workspace directory.

If the graph is from the wrong project, call `reload_graph` with the correct
path:

> Call `mcp_graphenium_reload_graph(graph_path: "/full/path/to/project/graphenium-out/graph.json")`

The `gm serve` server runs with `--watch` by default (since v0.8.0), so if
you run `gm run .` to rebuild the graph, the server picks up the changes
automatically.

## Detection: is the graph available?

Check whether `graphenium-out/graph.json` exists in the current project root.
If it does not, suggest `gm init && gm run .` (or `gm run . --no-semantic`
for fast AST-only without an API key).

If MCP tools (`query_graph`, `get_node`, `get_neighbors`, etc.) appear in
the available tool list, prefer them over the CLI. They give you richer
queries. When they are absent, use the `gm query` CLI fallback described
below.

## Tool selection (MCP connected)

| User asks | Use |
|-----------|-----|
| "What calls X?" / "What does X connect to?" | `get_neighbors(node=X, relation=calls)` |
| "Tell me about X" | `get_node(id_or_label=X)` |
| "What community/module is X in?" | `get_node(id_or_label=X)` → read `community` → `get_community(community=N)` |
| "What are the most-connected hubs?" | `god_nodes(count=10)` |
| "How are A and B connected?" | `shortest_path(source=A, target=B)` |
| "Give me an overview of the repo" | `graph_stats` + `architecture_summary` |
| "Summarize file X" | `summarize_file(path=X)` |
| General structural exploration | `query_graph(question="...", budget=2000)` |
| "What's the safest path from A to B?" | `mcp_graphenium_shortest_path(source=A, target=B, mode=semantic)`: prefers high-trust edges |
| "What's the blast radius of changing X?" | `blast_radius(symbol=X)`: shows downstream impact |
| "What symbols are unresolved?" | `unresolved_references()`: lists dangling references |
| "What symbols are ambiguous?" | `ambiguous_symbols()`: lists symbols with multiple definitions |
| "How trustworthy is this graph?" | `resolution_report()`: confidence breakdown, trust quality |
| "How should I verify these changes?" | `verification_plan()`: prioritized verification plan |
| "What files should I read next?" | `next_files_to_read(symbol=X)`: review order by risk |

`get_neighbors` output is ranked: behavioural edges (`calls`, `uses`,
`inherits`) appear first, structural edges (`contains`, `imports`) appear
last. The most informative relationships survive tight token budgets.

## Write-back tools (MCP)

When you have confirmed a relationship through code inspection, **write it
into the graph** so it persists across sessions:

| User action | Use |
|-------------|-----|
| "This module is the authentication boundary" | `add_node(id=auth_boundary, label="Auth Boundary", file_type=rationale, source_file=...)` |
| "I've confirmed UserService calls AuthProvider" | `add_edge(source=UserService, target=AuthProvider, relation=calls, confidence=EXTRACTED, source_file=...)` |
| "That import edge is wrong; delete it" | `remove_edge(source=X, target=Y, relation=imports)` |

**Confidence for AI-written edges:**
- Use **EXTRACTED** when you read the source and saw the relationship
- Use **INFERRED** when the naming/structure strongly suggests it but you haven't read both sides
- Do NOT use **AMBIGUOUS**. If you are uncertain, don't write the edge

All write tools persist to disk immediately. Edges you add survive server
restarts and `reload_graph`.

## Trust model (critical)

Every edge carries a confidence level and provenance metadata. **Weight your
conclusions by both.**

### Confidence tiers

- **EXTRACTED**: tree-sitter AST, resolver output, or AI-confirmed by code
  inspection. Ground truth. Act on it directly.
- **INFERRED**: LLM or behavioral heuristic. High-probability hint.
  Corroborate with one file read or grep before acting.
- **AMBIGUOUS**: LLM uncertainty. Do NOT act on directly. If an AMBIGUOUS
  edge is the only evidence for a claim, say so explicitly and recommend
  verification.

### Provenance metadata

Every edge also carries two provenance fields. Always check them before
trusting a connection:

- `extractor`: which system produced this edge. `tree-sitter` and
  `resolver` edges are deterministic; `llm` edges are inferred; `runtime-otel`
  edges come from trace data. A `resolver:resolved` edge is stronger than a
  `llm:inferred` edge with the same confidence tier.
- `resolution_status`: how the edge target was bound. `resolved` means the
  importer found a matching definition; `unresolved` means it could not;
  `heuristic` means a best-guess was used; `inferred` means an LLM proposed it.

Example from `query_graph` or `get_neighbors` output:
```text
- require_session `calls` validate_token [resolver:resolved]   <- high trust
- auth_service `uses` db_client [llm:inferred]                 <- inspect
- login_handler `imports` legacy_lib [resolver:unresolved]     <- dangling
```

`graph_stats` reports the confidence and provenance breakdowns. Prefer paths
that stay on `resolved` edges. Disclose provenance when recommending actions
based on graph evidence.

## v3 trust tools

The following MCP tools provide trust, verification, and impact analysis:

| Tool | Purpose |
|------|---------|
| `resolution_report` | Confidence breakdown, unresolved/ambiguous ratios, trust quality summary |
| `ambiguous_symbols` | Lists ambiguous edges and label collisions (same label, different node IDs) |
| `unresolved_references` | Lists dangling references that could not be resolved |
| `safest_path` | Pathfinding that prefers high-trust (`resolved`/`EXTRACTED`) edges |
| `verification_plan` | 7-tier prioritized verification plan from a graph diff |
| `blast_radius` | Downstream impact analysis via reverse reachability |
| `graph_info` | Full graph metadata: schema version, project root, build timestamp, languages, counts |
| `recluster` | Re-run community detection after manual node/edge edits |
| `query_transitive` | BFS transitive closure from a seed symbol with depth/direction control |
| `run_datalog(query)` | Run a Datalog query against the graph with rules, goals, and negation |
| `references_to` | Structural reference lookup (containers, imports, inheritance): 100% AST-only safe |
| `explain_change` | Composite pre-edit orientation: hierarchy + community + callers + files |

**Planning workspace tools (v0.14.0):**
| Tool | Purpose |
|---|---|
| `create_planning_workspace` | Create a virtual workspace to group intended changes |
| `add_planned_symbol` | Register an intended new/modified symbol linked to existing nodes |
| `get_plan_details` | Return the full virtual subgraph of a plan |
### When to use planning workspaces

Before implementing any multi-file architectural change, create a planning workspace and declare intended symbols. After writing the code, use `verification_plan` (Trust Tools) with the implemented symbol IDs to assess coverage against the graph, and `get_plan_details` to review the declared plan. This gives the user a formal audit trail: intended design versus actual implementation.

### Interpreting surprise scores and bridge nodes

When reviewing `architecture_summary` or generated questions, pay attention to:
- **High surprise-score edges**: these cross unexpected boundaries (community, directory, file type) and may indicate architectural erosion, leaky abstractions, or out-of-layer dependencies.
- **Bridge nodes (high betweenness centrality)**: files that are the sole conduit between two otherwise isolated communities. Changes to these nodes carry disproportionately high risk; inspect them before modifying.

Use this when the user asks about trust quality, change safety, or
verification: especially in CI or review contexts. For CI integration,
`gm check` enforces trust quality gates from the CLI (see AI_SETUP.md).

Before running `gm run`, use `gm run --plan` to pre-scan the workspace
(file count by extension, estimated extraction cost). For machine-readable
output, use `gm query --json` or `gm diff --json`.

For Windows users, guide them through `install.ps1`:
```powershell
powershell -ExecutionPolicy Bypass -File install.ps1
```

Note that `.NET`/C# repositories are supported: the extractor handles
`using` directives, `namespace` declarations, and `.csproj`/`.sln` files.

## CLI fallback (`gm query`)

When MCP tools are not available, use `gm query` via `exec_shell`. This
does keyword-scored BFS/DFS over the graph and returns a Markdown subgraph
within a token budget.

```
gm query "<keywords or question>" [flags]
gm query "--datalog ?- node(X, _, _, _, _)."
```

**Key flags:**

| Flag | Purpose |
|------|---------|
| `--budget N` | Token budget (default 2000). Raise for broader exploration. |
| `--mode MODE` | Query mode: `lexical` (TF-cosine keywords), `structural` (graph-distance proximity), or `hybrid` (combined). |
| `--path-prefix P` | Restrict to nodes whose source path contains `P`. Use to scope to a module or directory. |
| `--exclude-path P` | Exclude nodes whose source path contains `P`. |
| `--dfs` | Depth-first instead of default BFS (deeper but narrower). |
| `--generated-code-mode exclude` | Skip generated/template/vendor paths when they add noise. |
| `--graph path/to/graph.json` | Point to a non-default graph file. |
| `--safe` | Confidence-aware search: prefers paths on `resolved`/`EXTRACTED` edges, excludes `AMBIGUOUS` connections. |

### v0.7.0 transitive query

| MCP Tool | What it returns | When to use |
|---|---|---|
| `query_transitive(seed, depth?, relation?, direction?)` | BFS transitive closure: all reachable nodes grouped by depth; direction: forward (default), reverse, both | "What depends on this symbol?" / "What does this symbol depend on?" |

### v0.6.0 composite tools

| MCP Tool | What it returns | When to use |
|---|---|---|
| `analyse_symbol(symbol)` | Node info + behavioral connections (calls/uses/inherits) + structural (imports/contains) + trust profile | Single-turn symbol understanding instead of calling get_node + get_neighbors separately |
| `module_dependencies(mod_a, mod_b)` | Dependency summary: edges from module A to module B grouped by relation | "What does the auth module depend on from the core module?" |
| `what_changed(snapshot_name)` | Risk-sorted delta: removed symbols, community moves, additions + downstream impact | "What changed since my last snapshot?" |

### v0.6.0 new query parameters

| Parameter | Applies to | Effect |
|---|---|---|
| `include_tests=false` (default) | All query tools | Excludes test/spec nodes (replaces `exclude_test_nodes`) |
| `show_leaves=false` (default) | `summarize_file` | Hides low-degree leaf symbols; hubs always shown |
| `extracted_only=false` (default) | `get_neighbors` | Only `EXTRACTED` (source-backed) edges: zero heuristic/ambiguous |
| Trust Profile | All query responses | Appends `N EXTRACTED, N INFERRED, N AMBIGUOUS` summary |

**Examples:**

```sh
# What calls process_batch, scoped to the parser directory
gm query "process_batch" --path-prefix parser --budget 3000

# Explore the UserService neighborhood, excluding test files
gm query "UserService neighbors" --exclude-path _test --budget 4000

# Deep traversal through authentication code
gm query "authentication flow" --dfs --path-prefix auth --budget 5000
```

The output is Markdown. Read it directly; do not re-parse it through
another tool unless you need a specific field. The subgraph lists nodes
with their IDs, labels, communities, and edges with confidence levels.

## CLI fallback (`gm diff`)

When the user asks about the impact of changes, use `gm diff` to compare
two graph snapshots.

```
gm diff --before old-graph.json --after new-graph.json [--impact] [--review-plan]
```

- Without `--impact`: shows added/removed nodes and edges.
- With `--impact`: also shows downstream impact (reverse reachability),
  affected communities, edge confidence breakdown, and a recommended
  review order.
- With `--review-plan`: shows a 7-tier prioritized verification plan for
  the changes detected in the diff.

If no `--before` snapshot is available, suggest the user save a copy of
`graphenium-out/graph.json` before making changes so they can diff later.
