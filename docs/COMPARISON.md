# Graphenium Competitive Comparison

> **How Graphenium compares to existing code search, code graph, semantic search, and agent coding tools.**

---

## What Graphenium Is

Graphenium is **persistent structural memory for AI coding agents**. It builds a provenance-aware knowledge graph from source code and serves it over the Model Context Protocol (MCP). Every relationship carries confidence and source-backing metadata so agents can separate verified facts from inferred or ambiguous leads.

Graphenium does not replace source-code reading. It replaces repeated cold-start repository navigation.

---

## Competitive Landscape

The table below compares Graphenium against four categories of existing tools across the capabilities that matter for AI-agent code understanding.

| Capability | Generic Code Search | Generic Code Graph | Semantic Search | Agent Coding Tools | **Graphenium** |
|---|---|---|---|---|---|
| **Text/symbol matching** | ✅ Excellent | ✅ Good | ✅ Good | ✅ Good | ✅ Good |
| **Repository topology** | ❌ None | ✅ Yes | ❌ Limited | 🟡 Per-session | ✅ Yes |
| **Cross-file import resolution** | ❌ No | 🟡 Sometimes | ❌ No | 🟡 Per-session | ✅ Built-in |
| **Persistent across sessions** | ❌ No | 🟡 Sometimes | 🟡 Sometimes | ❌ No | ✅ Yes |
| **MCP-native delivery** | ❌ No | ❌ No | ❌ No | 🟡 Varies | ✅ First-class |
| **Provenance/confidence on edges** | ❌ No | ❌ No | ❌ No | ❌ No | ✅ Core model |
| **Separates extracted/inferred/ambiguous** | ❌ No | 🟡 Rare | ❌ No | ❌ No | ✅ Yes |
| **Change blast radius** | ❌ No | 🟡 Rare | ❌ No | 🟡 Session-limited | ✅ Built-in |
| **Diff + impact analysis** | ❌ No | 🟡 Rare | ❌ No | 🟡 Session-limited | ✅ Built-in |
| **Community detection** | ❌ No | ❌ No | ❌ No | ❌ No | ✅ Louvain-based |
| **Trust gating in CI** | ❌ No | ❌ No | ❌ No | ❌ No | ✅ `gm check` |
| **Token-optimised for LLM context** | ❌ No | ❌ No | ❌ No | 🟡 Partial | ✅ Primary goal |
| **Embedding/vector search** | ❌ No | ❌ No | ✅ Yes (requires API) | 🟡 Some | ✅ TF + Node2Vec |
| **Open source / offline** | ✅ Yes | ✅ Mostly | ❌ API-dependent | 🟡 Varies | ✅ Yes |
| **Runtime telemetry overlay** | ❌ No | ❌ No | ❌ No | ❌ No | 🟡 Experimental |

---

## Detailed Comparison

### Generic Code Search (`grep`, `ripgrep`, `ag`, `git grep`)

**Strengths**: Blazingly fast text matching across any corpus size. Zero setup. Ubiquitous.

**Limitations**: No understanding of code structure, imports, or relationships. Every query starts from scratch with no persistent model of the repository. Cannot answer "what depends on this?" or "what is the architecture of this project?".

**Verdict**: The right tool for finding strings, not for building structural memory.

### Generic Code Graph (tree-sitter, ast-grep, SCIP, Sourcegraph)

**Strengths**: Parse code into structured representations. Find references, definitions, and some call relationships. SCIP and Sourcegraph provide cross-repository code navigation.

**Limitations**:
- **No trust model**: all relationships are treated equally — no confidence/provenance metadata, no way to distinguish extracted from inferred from ambiguous connections.
- **Not MCP-first**: designed for human-facing UIs (Sourcegraph web), CLI tools (ast-grep), or IDE integration (SCIP). Agent-native delivery requires wrapping.
- **No diff analysis**: can answer "what is here?" but not "what changed between these two versions?" or "what is the blast radius of this change?".
- **No community detection**: no automatic grouping of related modules or discovery of architectural boundaries.

**Verdict**: Excellent for per-session code navigation. Does not address persistent agent memory, trust verification, or change impact.

### Semantic Search (bloop, Sourcegraph with embeddings, Cursor search)

**Strengths**: Natural language querying of codebases using LLM embeddings. Can find semantically related code even without exact keyword matches.

**Limitations**:
- **Requires API keys**: embedding models typically call external APIs; offline operation is limited or impossible.
- **No structural memory**: semantic search is query-response, not a persistent graph. The model is rebuilt per-query, not maintained across sessions.
- **No diff analysis**: cannot answer "what changed and what is affected by it?".
- **No confidence metadata**: results are scored by embedding similarity, not by source-backed provenance.
- **No topology**: no awareness of imports, containment, call graphs, or community structure.

**Verdict**: Complementary for natural-language search, but not a substitute for structural graph analysis or persistent agent memory.

### Agent Coding Tools (Claude Code, Cursor, CodeWhale, Copilot)

**Strengths**: Combine LLM code generation with repository context. Cursor and CodeWhale have some level of codebase indexing.

**Limitations**:
- **They are consumers of tools like Graphenium, not competitors.** These tools need structural memory to navigate large codebases. Graphenium is designed to be the graph backend that agent platforms query via MCP.
- **Session-scoped context**: most agent tools build context per session, with limited persistence of repository understanding across restarts.
- **No trust gating**: these tools do not verify the quality or provenance of their codebase understanding before acting on it.

**Verdict**: Graphenium integrates with these tools (via MCP) to provide the structural memory layer they currently lack.

---

## Graphenium Differentiators

### 1. Provenance and Confidence on Every Edge

Every relationship in the graph carries:
- **Confidence level**: `EXTRACTED` (1.0) / `INFERRED` (0.5) / `AMBIGUOUS` (0.2)
- **Extractor provenance**: which system produced it (`tree-sitter`, `resolver`, `llm-anthropic`, etc.)
- **Resolution status**: whether the relationship was resolved (`resolved`, `unresolved`, `ambiguous`)
- **Evidence spans**: source location with byte offsets, line ranges, and SHA256 content hashes for staleness detection

No other code graph tool surfaces this metadata to the consumer. Agents can decide to trust, verify, or ignore each fact based on its provenance.

### 2. MCP-Native Design

Graphenium speaks the Model Context Protocol (MCP) over stdio JSON-RPC as its primary delivery mechanism. This means:
- Zero network configuration — works with any MCP-compatible host.
- No HTTP server to manage — runs as a sidecar process.
- Tools (`query_graph`, `get_node`, `get_neighbors`, `shortest_path`, etc.) are designed for LLM consumption, not human UIs.
- Compatible with Claude Desktop, Cursor, CodeWhale, and any MCP client.

### 3. Diff + Impact Analysis

Built-in snapshot diff (`gm snapshot`, `gm gate`) compares two graph versions and produces:
- Added/removed nodes and edges
- Symbol inventory changes
- Community membership changes
- Downstream impact via reverse reachability (blast radius)

This enables CI gates that block PRs based on architectural impact, not just test failures.

### 4. Cross-File Resolver

The import resolver builds an export index from all extracted symbols and resolves import edges across files. Resolved imports carry `resolution_status: "resolved"`; unresolved ones are explicitly marked. This is built into the pipeline, not an external tool.

### 5. Community Detection

Louvain-based community detection automatically groups related symbols into architectural clusters. The result is a structural map of the repository with:
- Per-community cohesion scores
- Split clustering for oversized communities
- Drift detection between extracted and expected community structure

### 6. Trust Gating in CI

The `gm check` command evaluates policies against the graph and returns a structured pass/fail result:
- `MinResolution` — minimum import resolution percentage
- `MaxAmbiguous` — maximum ambiguous edges
- `MaxStale` — maximum stale evidence spans
- `MinCoherence` — minimum community coherence
- Policies are definable in TOML files

This allows teams to enforce a minimum trust bar before accepting graph-derived analysis.

### 7. Token-Optimised for LLM Context

Every design decision considers the LLM context window:
- Sparse graph representation (graph-distance queries over dense embedding stores)
- Token-efficient MCP responses (ranked results with relevance scores, not raw dumps)
- Configurable query depth and budget for traversal
- Community-aware summarisation instead of flat listings

---

## When To Choose Something Else

Graphenium is not the best tool for every code-understanding task:

- **Need raw file search?** Use `ripgrep`. It is faster and needs no setup.
- **Need IDE-grade code navigation?** Use Sourcegraph or a language server. They have deeper per-symbol resolution for interactive use.
- **Need natural-language code search?** Use bloop or Sourcegraph with embeddings. Graphenium's TF-cosine lexical search is deliberately simple.
- **Need a full static analysis platform?** Use Semgrep or CodeQL. Graphenium does not do pattern-based bug finding or dataflow analysis.
- **Need a profiler or APM?** Use your existing observability stack. Graphenium's telemetry overlay is supplementary, not a replacement.

Graphenium occupies the specific niche of **persistent, trust-aware structural memory for AI coding agents** — and within that niche, it has no direct equivalent.

---

## Summary

| Aspect | Graphenium |
|---|---|
| **Primary use case** | Persistent structural memory for AI coding agents |
| **Delivery** | MCP over stdio JSON-RPC |
| **Data model** | Nodes, edges, hyperedges, communities, provenance metadata |
| **Extraction** | Tree-sitter AST → import resolver → optional LLM semantic pass |
| **Trust** | Confidence levels (Extracted/Inferred/Ambiguous), evidence hashes, stale detection |
| **Query** | Lexical (TF-cosine), structural (graph-distance), hybrid |
| **Analysis** | God nodes, surprise edges, blast radius, community drift |
| **CI integration** | `gm check` with policy-based quality gates |
| **Status** | AST + Resolver and Semantic Pass stable; Telemetry experimental |
| **License** | Open source |
