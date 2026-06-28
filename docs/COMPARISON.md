# Comparison and Positioning

Graphenium belongs in the ecosystem of repository maps, codebase-to-graph tools, MCP graph servers, semantic search, and enterprise code intelligence systems.

Its lane is deliberately narrow:

> **Trust-aware codebase memory and change-safety infrastructure for AI coding agents.**

Graphenium is not trying to be the deepest static analyzer, the largest code graph, or the broadest search engine. It is the map an agent uses before reading and changing code.

---

## Category comparisons

### Grep, ripgrep, and exact search

Exact search is excellent for locating strings and symbols. It is weak for topology, dependency paths, architectural hubs, blast radius, and cross-session memory.

Graphenium does not replace exact search. It tells an agent where to search and which files are likely to matter.

### Vector search and semantic search

Semantic search is useful when naming is inconsistent or an agent needs concept-level retrieval. It is weaker for source-backed graph relationships, directed impact analysis, per-hop confidence, and change-safety gates.

Graphenium is structural-first. Hybrid retrieval can combine lexical and topology signals, but the primary value is the repository graph with provenance.

### Repo maps

Repo maps are useful for compact agent context. They usually identify important symbols and files under a token budget.

Graphenium extends the repo-map idea into a persistent, MCP-accessible graph with explicit confidence, provenance, blast-radius analysis, graph diffing, and CI trust gates.

### Generic MCP code graphs

Generic code graph MCP servers expose structural repository data to agents. Many optimize for fast indexing, broad language support, or graph retrieval.

Graphenium differentiates on trust and change safety:

- `EXTRACTED`, `INFERRED`, and `AMBIGUOUS` confidence;
- provenance and resolution metadata;
- safe traversal;
- safest source-backed paths;
- symbol-level diffing;
- downstream impact analysis;
- policy-based quality gates.

### Enterprise semantic graphs and indexers

Compiler-backed code intelligence platforms can offer very high precision in controlled environments, especially with full build context.

Graphenium is lighter-weight and easier to bootstrap. It is not designed to be compiler-perfect. It is designed to give AI agents useful, inspectable, low-friction structural memory across diverse repositories.

---

## Summary matrix

| Dimension | Enterprise indexers | Repo maps | Generic MCP code graphs | Graphenium |
|---|---|---|---|---|
| Primary goal | Compiler-precise code intelligence | Compact agent context | GraphRAG / code graph access | Trust-aware agent memory and gating |
| Setup overhead | High | Low | Low to moderate | Low |
| Interface | Custom APIs / CLI / IDE | Tool-specific | MCP / CLI | MCP / CLI |
| Persistent graph memory | Yes | Tool-specific | Usually | Yes |
| Confidence/provenance | Usually compiler-derived, less agent-facing | No | Rare | Core model |
| Extracted/inferred/ambiguous separation | Not usually agent-facing | No | Rare | Yes |
| Token-budgeted traversal | Not primary | Yes | Sometimes | Yes |
| Safest source-backed path | Custom | No | Rare | Built in |
| Blast-radius analysis | Possible with custom tooling | No | Sometimes | Built in |
| CI trust gates | Custom | No | No | Built in |
| Best fit | Large orgs with heavy indexing infrastructure | CLI coding agents | Agent graph retrieval | Multi-agent workspaces and large-repo AI workflows |

---

## When Graphenium is a strong fit

Graphenium is a strong fit when:

- agents repeatedly work in the same large repository;
- navigation tokens are crowding out reasoning and implementation context;
- reviewers want dependency paths and blast-radius summaries for agent patches;
- the team wants confidence and provenance surfaced to the agent;
- CI should enforce graph-quality thresholds;
- the repo spans multiple languages or build systems and needs a low-friction map.

---

## When Graphenium is not the right tool

Graphenium is not the best primary tool when:

- the repository is small enough for an agent to read directly;
- the task is pure exact-text lookup;
- a compiler-perfect call graph is mandatory;
- runtime behavior depends heavily on dynamic dispatch, reflection, code generation, or framework conventions and no semantic/manual/telemetry layer is configured;
- the team will treat graph output as a substitute for source inspection.

---

## Positioning sentences

Short:

> Provenance-aware repo memory for AI coding agents.

Practical:

> Before your agent edits code, give it a map it can trust.

Detailed:

> Graphenium is a trust-aware repository graph that lets AI coding agents plan changes, trace impact, and choose the right files to read before spending context on source code.

Category:

> Graphenium is the context-budget, trust, and change-safety layer for AI coding agents working in large repositories.
