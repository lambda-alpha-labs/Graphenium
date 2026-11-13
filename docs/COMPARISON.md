# Competitive Analysis: Active Gating vs. Passive Search

AI developer tooling is currently saturated with passive codebase search indexes, codebase visualization tools, and "GraphRAG" databases. These tools help agents *read* or *search* repositories. Graphenium is a **relational codebase compiler** that both compresses that reading and enforces a far more critical engineering role: **active write-time containment and spec-driven pre-flight gating**.

This document outlines how Graphenium compares to alternative categories of developer tooling, including traditional static architecture linters. The short version: Graphenium matches their boundary intent, but adds Virtual AST pre-flight workspaces, zero-config Louvain ΔQ relative modularity gating, Datalog fixed-point reasoning with stratified negation, and Lean 4 proof-gap plus hypothesis-aware semantic axiom auditing (`gm trust`) — none of which classic import-rule linters provide.

---

## 1. High-Level Comparison Matrix

| Tool Category | Core Strength | Where Graphenium is Distinct |
|---|---|---|
| **Text Search** (`grep`, `ripgrep`) | High-speed exact string/regex matches. | Graphenium compiles symbol relationships, traces transitive calls, and runs logical policy checks. |
| **Syntactic AST Matchers** (`ast-grep`) | Single-file pattern-matching and syntax rewrites. | Graphenium maintains a persistent, cross-file structural index and enforces pre-flight design specs. |
| **Enterprise Search** (Sourcegraph) | Federated human search across thousands of repositories. | Graphenium is local-first, MCP-native, and designed as a machine-level write-gate for agents. |
| **Symbol Indexers** (SCIP, LSIF, Kythe) | Compiler-perfect reference indices for IDE definitions. | Graphenium incorporates provenance confidence tracking, transitive solvers, and design planning workspaces. |
| **Static Architecture Linters** (ArchUnit, import-linter, dependency-cruiser) | Declared import/folder rules checked against source. | Graphenium adds Virtual AST pre-flight plans, zero-config Louvain ΔQ relative modularity gating, Datalog fixed-point proofs, and Lean 4 proof-gap plus hypothesis-aware semantic axiom auditing. |
| **Codebase RAG / Vector Indexes** | Probabilistic semantic retrieval from text chunks. | Graphenium is AST-proven and deterministic. It uses a local Datalog engine instead of neural "guesses." |
| **AI Coding Agents** (Cursor, Claude Code) | Generating and editing file contents. | Graphenium acts as an **external containment gate**, blocking these agents from violating architecture rules. |

---

## 2. In-Depth Tooling Analyses

### Grep and `ripgrep`
Grep is the fastest tool in the world for finding literal text. However, it lacks structural awareness.
*   **The Gap for Agents:** Grep cannot trace dependency direction, calculate transitive closures, evaluate top-degree hubs, or estimate the blast radius of a change.
*   **The Graphenium Edge:** Graphenium uses Tree-sitter to turn text into compiler-proven structures. It answers structural questions (e.g., *"What is transitively affected if I modify this struct?"*) with deterministic accuracy. 
*   *Workflow Guidance: Graphenium does not replace grep. Graphenium determines which files are structurally relevant, and the agent uses grep to inspect or modify exact lines within those files.*

### `ast-grep` and Tree-sitter Utilities
`ast-grep` is highly effective at syntax-aware, single-file pattern matching and localized refactoring.
*   **The Gap for Agents:** It operates on individual files and lacks a persistent, cross-file index. It cannot resolve cross-file import boundaries, track structural domains (communities), or run multi-file policy gates.
*   **The Graphenium Edge:** Graphenium uses Tree-sitter as a parser but builds a persistent, cross-file structural index. It introduces **virtual planning workspaces** where agents can model multi-file designs pre-flight before any code is physically refactored.

### Symbol Indexers (SCIP, LSIF, Kythe, ctags)
These indexers provide compiler-perfect reference tracking for IDE navigations (definitions, references, hover tips).
*   **The Gap for Agents:** They are built for human-oriented read pathways. They are flat, passive reference structures that do not include provenance confidence mapping, Datalog-powered layering constraints, or change-impact verifications.
*   **The Graphenium Edge:** Graphenium is built for the **agentic write lifecycle**. It includes an embedded Datalog solver to prove layer-bypassing violations, separates compiler truth (`EXTRACTED`) from semantic hypotheses (`INFERRED`), and runs post-edit scope audits to detect unplanned file edits.

### Traditional Static Architecture Linters (ArchUnit, import-linter, dependency-cruiser)
Classic architecture linters encode human-written import or folder rules and fail builds when a direct dependency violates them. That is useful, but it stops at the exact point where AI agents create the most risk.
*   **The Gap for Agents:** They evaluate physical code after the agent has already edited it. They generally require hand-maintained glob/layer configuration, cannot model a proposed design before files change, do not reason over multi-hop fixed points beyond simple transitive imports, and have no notion of provenance tiers, relative modularity decay, or formal proof gaps.
*   **The Graphenium Edge:** Graphenium compiles a provenance-tagged relational index, lets the agent declare a **Virtual AST** in memory, and validates that future topology before disk mutation. It runs embedded semi-naive Datalog with stratified negation for transitive/aliasing/unreachability proofs, applies zero-config Louvain ΔQ gating that accepts legacy complexity as baseline while rejecting *new* entropy, audits Lean 4 `sorry`/`admit` taint plus semantic `#print axioms` trust boundaries and binder hypotheses, and then verifies realized nodes, realized edges, and file-scope compliance after the edit.

### Codebase RAG & Semantic Vector Databases
Vector databases split code files into text chunks and use embedding models to perform nearest-neighbor searches.
*   **The Gap for Agents:** Vector search is inherently probabilistic and "fuzzy." It is highly prone to hallucinating connections, cannot trace dependency chains, and suffers from context bloat because it returns large chunks of irrelevant text.
*   **The Graphenium Edge:** Graphenium is local-first, zero-cost, and completely offline. It establishes **AST-proven compiler truth first** using tree-sitter. It only introduces semantic heuristics under an explicit `INFERRED` provenance tag, ensuring the agent never confuses a vector similarity "guess" with a physical code import.

---

## 3. Structural Capabilities Matrix

| Feature | Graphenium | Arch Linters | `ripgrep` | `ast-grep` | Kythe / SCIP | RAG / Vector |
|---|---|---|---|---|---|---|
| **Local-First Parsing** | **Yes** | Yes | Yes | Yes | Yes | No |
| **Offline Import Resolution** | **Yes** | Partial | No | No | Yes | No |
| **Provenance Tracking** | **Yes** | No | No | No | Partial | No |
| **Confidence Tiering** | **Yes** | No | No | No | No | No |
| **Transitive Closure Proofs** | **Yes (Datalog)** | Partial | No | No | No | No |
| **Virtual AST Pre-Flight Specs** | **Yes (`plan_id`)** | No | No | No | No | No |
| **Zero-Config Louvain ΔQ Gating** | **Yes** | No | No | No | No | No |
| **Lean 4 Proof-Gap & Axiom Trust** | **Yes** | No | No | No | No | No |
| **Scope-Creep Audits** | **Yes** | No | No | No | No | No |
| **Declarative CI Policy Gates** | **Yes** | Yes | No | No | No | No |
| **Telemetry Context Overlays** | **Yes (Experimental)** | No | No | No | No | No |

---

## 4. When to Deploy Graphenium

### Deploy Graphenium if:
1.  **AI agents are executing multi-file edits** in your repository and you want to prevent them from introducing architectural drift.
2.  Your codebase has strict design patterns (such as layered architectures, DDD, or clean architecture) that agents must comply with.
3.  You want to **fail the build in CI** whenever an agent-generated PR violates a module boundary or modifies files outside its declared task scope.
4.  Your codebase is too large to fit entirely in the agent's context window, and you need a highly compressed, token-optimized model of the system boundaries.
5.  You operate in a regulated, secure environment and require **100% offline, zero-network code parsing**.

### Do not deploy Graphenium if:
1.  Your repository is small enough to fit completely inside the agent's native context window.
2.  Your target task is pure, exact-text keyword lookup (use `ripgrep`).
3.  Your architecture relies heavily on dynamic reflection or runtime dependency injection that cannot be mapped statically.