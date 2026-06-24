# Worked example: Graphenium (self-analysis)

**Repo:** [lambda-alpha-labs/Graphenium](https://github.com/lambda-alpha-labs/Graphenium)  
**Language:** Rust  
**Graphenium mode:** AST-only  
**Nodes:** 868  
**Edges:** 1,766  
**Communities:** 19  

## What Graphenium got right

### Community detection mirrors directory structure

The 19 communities map cleanly to source directories:

- **Community 3** (83 nodes): `src/serve/`, the MCP server surface,
  including `GrapheniumServer` and all 13 tool handlers.
- **Community 0** (127 nodes): Core reporting, analysis, and validation
  types from `src/report.rs`, `src/analyze/`, `src/validate.rs`.
- **Community 1** (117 nodes): Build pipeline, watch mode, and semantic
  extraction orchestration.
- **Community 7**: The graph model: `GrapheniumGraph`, `Edge`,
  `Node`, `ReplaceStats`.

### Hub nodes match architectural importance

| Node | Degree | File |
|------|--------|------|
| `GrapheniumServer` | 30 | `src/serve/handlers.rs` |
| `Node` | 29 | `src/validate.rs` |
| `GrapheniumGraph` | 24 | `src/model/graph.rs` |
| `Manifest` | 19 | `src/main.rs` |
| `render_report` | 18 | `src/report.rs` |

These are exactly the types you'd expect to be architectural hubs:
the central server struct, the core model type, the graph engine, and
the report generator.

### Cross-community connectors identify real bridges

- **`mod`** (degree 103): module re-exports bridging 6 communities
- **`Node`** (degree 29): the core model type used by 10 communities
- **`handlers`** (degree 59): the MCP handler module bridging 5
  communities

### Shortest path accuracy

`GrapheniumServer → handlers → GrapheniumGraph` (2 hops). Correct:
the MCP server struct wraps the graph through the handlers module.

## What it missed

Without semantic extraction, edges are mostly `imports`, `contains`,
`method`, and `field_of`. What's missing:

- **`calls` edges between functions across files.** The AST extractor
  captures intra-file `calls` via tree-sitter, but cross-file call
  resolution requires the semantic pass.
- **Conceptual relationships.** Edges like `delegates_to`,
  `rationale_for`, and `implements` are only available through LLM
  inference.
- **Directional behavioural tracing.** Without `calls` edges, you can't
  trace "what calls this function?" across module boundaries; you get
  "what imports this module?" instead.

## Most useful queries

```sh
# What is the MCP server surface?
gm query "mcp server handlers"

# How does the build pipeline work?
gm query "graph build extraction"

# How is community detection implemented?
gm query "community detection"
```

## MCP-style questions

- "What is the shortest path from `GrapheniumServer` to `GrapheniumGraph`?"
  → 2 hops: `GrapheniumServer → handlers → GrapheniumGraph`

- "What are the god nodes?"
  → `GrapheniumServer` (30°), `Node` (29°), `GrapheniumGraph` (24°)

- "What community does `handlers.rs` belong to?"
  → Community 3 (the MCP server surface)

- "What symbols are extracted from `src/serve/handlers.rs`?"
  → 84 symbols: all MCP tool handlers, helpers, and test functions

## Would I use this again?

Yes. For any Rust project >500 files, the architectural map alone saves
10+ minutes of orientation per session. The graph is most useful before
reading any source. It guides you to the right files. It doesn't replace
reading them.
