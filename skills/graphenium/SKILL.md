---
name: graphenium
description: Use when navigating a Graphenium knowledge graph: querying code structure, tracing dependencies, identifying hubs, or understanding module communities. Triggers on "what calls X", "what connects to Y", "what community", "shortest path", "god nodes", "hub nodes", "graph stats", or any structural question about the codebase that a knowledge graph can answer.
---

# Graphenium Skill

Graphenium is a knowledge graph engine for the current codebase. It runs
`gm run` to extract structure (AST + optional LLM), then serves the result
via MCP tools or the `gm query` CLI. This skill tells you which tool to
reach for and how to interpret the output.

## Detection: is the graph available?

Check whether `graphenium-out/graph.json` exists. If it does not, suggest
`gm run .` (or `gm run . --no-semantic` for fast AST-only without an API
key).

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

Every edge carries a confidence level. **Weight your conclusions by it.**

- **EXTRACTED**: tree-sitter AST or AI-confirmed by code inspection. Ground truth. Act on it directly.
- **INFERRED**: Claude API (high confidence) or plausible-but-unverified. Treat as a strong hint.
  Corroborate with one file read or grep before acting.
- **AMBIGUOUS**: Claude API (uncertain). Do NOT act on directly. These
  are questions to investigate, not answers. If an AMBIGUOUS edge is the
  only evidence for a claim, say so explicitly and recommend verification.

`graph_stats` reports the confidence breakdown. A graph dominated by
EXTRACTED edges is more reliable than one heavy on INFERRED/AMBIGUOUS.

## CLI fallback (`gm query`)

When MCP tools are not available, use `gm query` via `exec_shell`. This
does keyword-scored BFS/DFS over the graph and returns a Markdown subgraph
within a token budget.

```
gm query "<keywords or question>" [flags]
```

**Key flags:**

| Flag | Purpose |
|------|---------|
| `--budget N` | Token budget (default 2000). Raise for broader exploration. |
| `--path-prefix P` | Restrict to nodes whose source path contains `P`. Use to scope to a module or directory. |
| `--exclude-path P` | Exclude nodes whose source path contains `P`. |
| `--dfs` | Depth-first instead of default BFS (deeper but narrower). |
| `--generated-code-mode exclude` | Skip generated/template/vendor paths when they add noise. |
| `--graph path/to/graph.json` | Point to a non-default graph file. |

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
