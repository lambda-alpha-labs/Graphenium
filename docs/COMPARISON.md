# Comparison and Positioning

Graphenium belongs in the ecosystem of repository maps, codebase-to-graph tools, MCP graph servers, and enterprise code intelligence systems.

Its lane is deliberately narrow:

> Trust-aware codebase memory and change-safety infrastructure for AI coding agents.

## Categories

### Grep, ripgrep, and exact search

Excellent for exact text lookup. Weak for topology, dependency paths, architectural hubs, blast radius, and cross-session memory.

Graphenium does not replace exact search. It tells an agent where to search and which files are likely to matter.

### Vector search and semantic search

Useful when naming is inconsistent or the agent needs concept-level retrieval. Weak for source-backed graph relationships, directed impact analysis, and confidence-labeled dependency paths.

Graphenium is structural-first. Hybrid retrieval can combine lexical and topology signals, but the primary value is the repository graph.

### Repo maps

Repo maps are highly useful for compact agent context. They typically identify important symbols and files, often under a token budget.

Graphenium extends the idea into a persistent, MCP-accessible graph with explicit confidence, provenance, blast-radius analysis, graph diffing, and CI trust gates.

### Generic MCP code graphs

Generic code graph MCP servers expose structural repository data to agents. Many are optimized for fast indexing, broad language support, or graph retrieval.

Graphenium differentiates on trust and change safety:

- `EXTRACTED` / `INFERRED` / `AMBIGUOUS` confidence
- provenance and resolution metadata
- safe traversal
- symbol-level diffing
- downstream impact analysis
- policy-based quality gates

### Enterprise semantic graphs and indexers

Systems such as compiler-backed code intelligence platforms can offer very high precision in controlled environments, especially when full build context is available.

Graphenium is lighter-weight and easier to bootstrap. It is not designed to be compiler-perfect. It is designed to give AI agents useful, inspectable, low-friction structural memory across diverse repositories.

## Summary matrix

| Dimension | Enterprise indexers | Repo maps | Generic MCP code graphs | Graphenium |
|---|---|---|---|---|
| Primary goal | Compiler-precise code intelligence | Compact agent context | GraphRAG / code graph access | Trust-aware agent memory and gating |
| Setup overhead | High | Low | Low to moderate | Low |
| Interface | Custom APIs / CLI / IDE | Tool-specific | MCP / CLI | MCP / CLI |
| Persistent graph memory | Yes | Tool-specific | Usually | Yes |
| Confidence/provenance | Usually compiler-derived, less agent-facing | No | Rare | Core model |
| Extracted/inferred/ambiguous separation | No / not primary | No | Rare | Yes |
| Token-budgeted traversal | Not primary | Yes | Sometimes | Yes |
| Blast-radius analysis | Possible with custom tooling | No | Sometimes | Built in |
| CI trust gates | Custom | No | No | Built in |
| Best fit | Large orgs with heavy indexing infrastructure | CLI coding agents | Agent graph retrieval | Multi-agent workspaces and large-repo AI workflows |

## Positioning sentence

Graphenium is not the biggest code graph or the deepest static analyzer. It is the trust-aware memory layer an AI coding agent uses before reading or changing code.

