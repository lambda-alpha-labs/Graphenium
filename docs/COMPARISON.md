# Competitive Comparison

Graphenium is best understood by what it is optimized for:

> AI coding agents that need to plan, modify, verify, and gate structural changes in real repositories.

It is not just search. It is not a replacement for tests. It is not a compiler-perfect index. It is a trust-aware architecture map for agentic code changes.

## Quick comparison

| Tool type | Best at | Where Graphenium is different |
|---|---|---|
| grep and ripgrep | exact text search | Graphenium understands relationships and dependency direction |
| tree-sitter and ast-grep | syntax-aware matching | Graphenium persists a cross-file graph with trust metadata |
| Sourcegraph | human code search and navigation | Graphenium is local-first and MCP-native for agents |
| Claude, Cursor, CodeWhale | writing and editing code | Graphenium gives these agents structural memory and verification workflows |
| ctags, SCIP, Kythe | symbol indexing and compiler-backed references | Graphenium adds confidence, community structure, blast radius, and agent workflows |
| generic RAG | semantic text retrieval | Graphenium models code as relationships, not just chunks |

## Grep and ripgrep

Grep is excellent for literal and regex search.

Where it falls short for agents:

- no dependency direction
- no persistent structural memory
- no trust model
- no blast radius
- no verification plan

Graphenium advantage:

- answers relationship questions such as what calls X and what depends on X
- carries provenance on edges
- gives an agent a compact path through the codebase

Use both together. Grep is still the fastest exact-text tool.

## tree-sitter and ast-grep

AST tools understand syntax within files and can run powerful structural searches.

Where they fall short for agents:

- usually not a persistent cross-file graph
- no agent workflow layer
- no MCP-native graph operations
- no confidence model
- no post-edit verification loop

Graphenium advantage:

- uses tree-sitter as a foundation
- adds cross-file resolution, communities, trust metadata, impact analysis, and MCP tools

## Sourcegraph

Sourcegraph is strong for human code search across large organizations.

Where it falls short for this specific lane:

- primarily optimized for human browsing
- network/server orientation
- no per-edge trust model designed for agent action
- no design-then-verify planning workspace
- no local-first architecture graph for an agent to query offline

Graphenium advantage:

- local-first
- MCP-native
- trust-aware
- workflow-oriented for agents

Sourcegraph can complement Graphenium for organization-wide search.

## AI coding tools

Claude, Cursor, CodeWhale, and similar tools are agents or agentic IDEs.

Where they fall short without Graphenium:

- repeated cold-start repo exploration
- grep-driven navigation
- hidden assumptions about dependencies
- no standalone architecture trust layer
- no graph-quality gate before review

Graphenium advantage:

- gives agents a shared structural memory
- teaches agents what to trust
- supports blast-radius checks and verification plans

Graphenium does not replace the coding agent. It makes the coding agent safer and more effective.

## Symbol indexers

ctags, SCIP, and Kythe are valuable for definitions, references, and compiler-backed indexing.

Where they fall short for agent workflows:

- flat or compiler-centric indexes are not enough for change planning
- confidence and provenance are often implicit
- limited planning and verification workflows
- no token-budgeted traversal designed for LLMs

Graphenium advantage:

- relational graph model
- explicit confidence tiers
- communities, hubs, chokepoints, and blast radius
- CI gates and planning workspaces

Kythe and compiler-backed systems may be more precise for certain language reference graphs. Graphenium optimizes for practical multi-language agent workflows.

## Feature matrix

| Capability | Graphenium | grep | ast-grep | Sourcegraph | coding agents | symbol indexers |
|---|---|---|---|---|---|---|
| Local architecture graph | Yes | No | No | Server-oriented | No | Partial |
| MCP-native agent interface | Yes | No | No | Limited or external | Varies | No |
| Per-edge confidence | Yes | No | No | No | No | Usually no |
| Provenance on relationships | Yes | No | No | Partial | No | Partial |
| Cross-file resolution | Yes | No | Limited | Yes | Varies | Yes |
| Blast radius | Yes | No | No | Limited | Varies | No |
| Planning workspace | Yes | No | No | No | No | No |
| CI trust gates | Yes | No | No | No | No | No |
| Token-budgeted output | Yes | No | No | No | Varies | No |
| Runtime telemetry overlay | Experimental | No | No | No | No | No |
| Multi-language repository map | Yes | No | Limited | Yes | Varies | Varies |

## When to use Graphenium

Use Graphenium when:

- AI agents repeatedly work in the same repository
- the repository is too large to read directly
- navigation tokens crowd out reasoning
- reviewers need blast radius and dependency paths
- agents need to distinguish facts from guesses
- CI should enforce graph trust thresholds
- the codebase spans multiple languages
- agent-generated changes need governance

## When not to use Graphenium

Do not use Graphenium as the primary tool when:

- the repository is small enough for direct reading
- the task is pure exact-text lookup
- compiler-perfect call graph precision is mandatory
- the codebase relies heavily on reflection or dynamic dispatch without traces
- the team plans to treat graph output as a substitute for source reading

## Core differentiation

Graphenium is the only tool in this comparison built around this full loop:

```text
Plan -> Read -> Edit -> Diff -> Blast radius -> Verify -> Gate
```

That is the lane to own.
