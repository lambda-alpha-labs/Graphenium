# Worked example: Graphenium (self-analysis)

**Repo:** [lambda-alpha-labs/Graphenium](https://github.com/lambda-alpha-labs/Graphenium)  
**Language:** Rust  
**Graphenium version:** 0.18.0  
**Graphenium mode:** AST + Stack Graphs  
**Schema version:** 0.2.0  
**Nodes:** 1,211  
**Edges:** 3,083  
**Communities:** 19  
**Generated:** 2026-07-08  

## What Graphenium got right

### Cross-file call resolution (new in v0.18.0)

The v0.18.0 Stack Graphs resolver found **927 cross-file references**.
This means cross-file `calls` edges now carry
`[tree-sitter-stack-graphs:resolved]` provenance — source-backed ground
truth instead of heuristics. Import resolution is at **100%** (267/267),
and cross-file call resolution is at **38%** (662/1,755).

The trust profile breaks down as:
- **43% EXTRACTED** (1,328 edges) — tree-sitter + resolver, ground truth
- **57% INFERRED** (1,755 edges) — heuristic, corroborate before acting
- **0% AMBIGUOUS** — no garbage edges

### Community detection mirrors directory structure

The 19 communities map cleanly to source areas:

- **Community 1** (162 nodes): Core surface — `handlers.rs` (110 nodes),
  the MCP server, plus analysis pipelines (verifier, impact, traversal).
  527 internal edges, 254 cross-community `calls` edges.
- **Community 3** (7 nodes): The graph model — `GrapheniumGraph`,
  `upsert_node`, graph integrity checks.
- **Community 9**: Build pipeline — `build_from_extraction`, `BuildTarget`,
  `build_merged`.

### Hub nodes match architectural importance

| Node | Degree | File |
|------|--------|------|
| `Manifest::len` | 79 | `src/cache/manifest.rs` |
| `GrapheniumGraph::upsert_node` | 62 | `src/model/graph.rs` |
| `Edge::extracted` | 58 | `src/model/edge.rs` |
| `GrapheniumServer` | 56 | `src/serve/handlers.rs` |
| `GrapheniumGraph::node_data` | 45 | `src/model/graph.rs` |

The hub list reveals that the manifest cache (`Manifest::len`, 79°) is
actually the highest-degree node — a surprise that only graph analysis
surfaces. The graph model and MCP server are the other central hubs, as
expected.

### Shortest path accuracy

`GrapheniumServer → add_planned_symbol → upsert_node → GrapheniumGraph`
(3 hops, cost 3.60). The path is source-backed at each step:
- `method` [tree-sitter:resolved] (EXTRACTED)
- `calls` [tree-sitter-stack-graphs:resolved] (INFERRED)
- `method` [tree-sitter:resolved] (EXTRACTED)

## What it missed

Without semantic extraction, some relationships are still inferred:

- **Cross-file calls at 38% resolution.** Stack Graphs cover many cases
  but not all — the remaining 62% of cross-file calls are heuristic.
  Running `gm run` without `--no-semantic` would fill these in via LLM.
- **Conceptual relationships.** Edges like `delegates_to`,
  `rationale_for`, and `implements` are only available through LLM
  inference.
- **Dynamic dispatch.** Rust trait objects and dynamic method calls are
  not resolved by tree-sitter or Stack Graphs.

## Most useful queries

```sh
# What is the MCP server surface?
gm query "serve module handlers mcp"

# How does the build pipeline work?
gm query "graph build extraction"

# How is community detection implemented?
gm query "community detection"
```

## MCP-style questions

- "What is the shortest path from `GrapheniumServer` to `GrapheniumGraph`?"
  → 3 hops: `GrapheniumServer → add_planned_symbol → upsert_node → GrapheniumGraph`

- "What are the god nodes?"
  → `Manifest::len` (79°), `upsert_node` (62°), `Edge::extracted` (58°),
  `GrapheniumServer` (56°), `node_data` (45°)

- "What community does `handlers.rs` belong to?"
  → Community 1 (the core surface, 162 nodes, 527 internal edges)

- "How trustworthy is this graph?"
  → 43% EXTRACTED, 57% INFERRED, 0% AMBIGUOUS. All imports resolved.
  38% cross-file calls resolved via Stack Graphs.

## Would I use this again?

Yes. The v0.18.0 cross-file resolution is a step change: going from
0 cross-file `calls` edges to 662 resolved ones means you can now trace
behavioural dependencies across module boundaries on source-backed edges.
The architectural map saves 10+ minutes of orientation per session.

The managed graph (this worked example) carries provenance on every edge.
The `graph_stats` MCP tool reports extractor and resolution status
breakdowns. Try `gm diff --before graph.json --after graph.json --impact`
to see the empty diff output, or `gm query "authentication" --mode hybrid`
for combined lexical and structural retrieval.
