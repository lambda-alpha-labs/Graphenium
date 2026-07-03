# Graphenium: Competitive Comparison

> **Question:** When should a team choose Graphenium over existing code-analysis tools?
>
> **Short answer:** When the primary consumer is an AI coding agent that needs to navigate,
> plan, and verify changes in a large repository — and when trust metadata per
> relationship matters more than raw search speed or parse depth.

---

## In detail

### Grep / ripgrep

**What they are:** The fastest way to find literal text or regex patterns across files.

**Where they fall short for agents:** Zero understanding of what a match means — no
knowledge that `authenticate()` in file A calls `validate_token()` in file B, no concept
of dependency direction, no persistent memory between queries. Every session starts
from scratch. An agent using grep alone must chain ad-hoc searches and infer structure
manually.

**Graphenium's advantage:** The graph answers "what depends on X?" and "what calls X?"
in one query, with source-backed evidence for every relationship. Grep is still useful
inside Graphenium's keyword-search layer; the two are complementary.

**Limitation:** Graphenium does not replace grep for ad-hoc text search — it is not a
dedicated search engine, and its keyword matching (TF-cosine) is less precise than
ripgrep for raw string lookup.

### tree-sitter / ast-grep

**What they are:** Tools that parse source code into syntax trees and match patterns
against those trees.

**Where they fall short for agents:** They understand structure within a single file
but have no concept of cross-file relationships, import resolution, or dependency
direction. They output matches as text spans, not as a persistent graph. There is
no trust model, no diff analysis, and no MCP server.

**Graphenium's advantage:** Graphenium uses tree-sitter for initial AST extraction,
then builds a cross-file graph on top with import resolution, community detection,
and trust metadata. The graph persists across sessions and serves through MCP.

**Limitation:** Graphenium's AST extraction is less configurable than a standalone
tree-sitter query for one-off pattern matching. If you need to write ad-hoc AST
patterns, ast-grep is more direct.

### Sourcegraph

**What they are:** Web-based code search with cross-repository navigation, commit
history, and code intelligence.

**Where they fall short for agents:** Sourcegraph is designed for humans browsing
code in a web browser. It has limited MCP support (experimental), no trust model
on individual relationships, no token-budgeted output, and no CI trust gating.
Every query requires a network round-trip to a server; there is no local
persistent graph for the agent to query offline.

**Graphenium's advantage:** Fully local, MCP-native, with trust metadata on every
edge. The graph persists on disk and loads in milliseconds. No network dependencies.

**Limitation:** Sourcegraph can index entire organizations across thousands of repos.
Graphenium works within a single repository at a time. For org-wide search,
Sourcegraph complements Graphenium.

### Claude Code / Cursor / CodeWhale

**What they are:** AI coding agents and IDEs that edit code and run commands.

**Where they fall short for agents:** They do not build persistent code graphs.
Every session starts with a cold repository navigation problem. Without Graphenium,
these tools grep their way to understanding.

**Graphenium's advantage:** Graphenium provides the structural memory these agents
lack — a pre-built, trusted graph they can query via MCP before touching files.

**Limitation:** Graphenium does not replace the coding agent. It makes the agent
more effective by providing structural memory.

### Symbol indexers (ctags, SCIP, Kythe)

**What they are:** Tools that extract flat lists or indices of symbols — definitions,
references, types.

**Where they fall short for agents:** Flat indexes answer "where is X defined?" but
not "what depends on X?" or "what is the path between X and Y?" They have no edge
confidence, no community structure, no blast-radius analysis, and no token-aware
traversal.

**Graphenium's advantage:** The graph model is relational, not flat. Edges carry
provenance and confidence. The resolver resolves cross-file imports into directed
dependency edges. Community detection (Louvain) groups symbols into architectural
clusters.

**Limitation:** Graphenium's extraction is less precise than Kythe's
compiler-backed index for languages with complex build systems.

---

## Core Differentiators

| Differentiator | What it means | Why it matters for agents |
|---|---|---|
| **Provenance/confidence per edge** | Every relationship is labeled EXTRACTED, INFERRED, or AMBIGUOUS with the source method | Agent knows which facts are safe to plan against vs. which need verification |
| **MCP-native** | Graph is served through MCP tools (`gm serve`), not a web UI or REST API | Agent uses the same protocol for graph queries as for file reads and edits |
| **Diff + impact analysis** | Symbol-level diff between graph snapshots with blast-radius computation | Reviewers get a risk-sorted delta instead of hunting through changed files |
| **Cross-file import resolver** | Import resolution directed edges across file boundaries | Agent gets verified edges, not guessed strings |
| **Community detection (Louvain)** | Graph is clustered into architectural communities | Agent sees the high-level shape of the codebase in one query |
| **CI trust gating** | `gm check` enforces min-resolution and max-ambiguous thresholds | PRs can be blocked when the graph is too unreliable to plan changes |
| **Token-optimized for LLM context** | Token-budgeted traversal, leaf-symbol omission, compact output format | Agents spend less context on navigation and more on reasoning |
| **Design-then-verify planning workspaces** | Agents declare intended symbols virtually before writing code; `verify_plan` audits compliance | Formal verification loop: intent, implementation, audit. Reduces review time for multi-file changes |
| **Topological anomaly detection** | Multi-variable surprise scoring without ML: cross-boundary edges, peripheral-to-hub jumps, cross-community links | Agents locate architectural erosion and leaky abstractions without writing custom rules |
| **Multi-language guardrails** | Language-family classification prevents cross-language false positives in mixed-stack monorepos | Enterprise-safe resolution in repos spanning C#, C++, Python, and JS/TS simultaneously |

---

## When to use Graphenium

- AI agents repeatedly work in the same large repository
- Navigation tokens are crowding out reasoning and implementation
- Reviewers want dependency paths and blast-radius summaries for agent patches
- The team wants confidence and provenance surfaced to the agent
- CI should enforce graph-quality thresholds before agent changes are accepted
- The repo spans multiple languages and needs a unified map

## When NOT to use Graphenium

- The repository is small enough for an agent to understand by reading directly
- The task is pure exact-text lookup (use ripgrep)
- A compiler-perfect call graph is mandatory (use Kythe or a language-specific indexer)
- The codebase relies heavily on dynamic dispatch, reflection, code generation
- The team will treat graph output as a substitute for source inspection (it is a map, not the territory)

---

## Positioning one-liner

> Graphenium is the trust-aware repository-graph layer that makes AI coding agents
> effective in large, multi-language codebases — without replacing grep, tree-sitter,
> Sourcegraph, or your coding agent of choice.
