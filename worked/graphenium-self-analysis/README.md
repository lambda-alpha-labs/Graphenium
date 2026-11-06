# Case Study Report: Graphenium Self-Analysis

This case study documents Graphenium's automated, local self-analysis of its own Rust codebase. It evaluates Graphenium's capacity to maintain architectural boundaries, resolve cross-file dependencies offline, and mechanically enforce system decoupling.

---

## 1. Codebase Profile

| Property | Value |
|---|---|
| **Target Repository** | [lambda-alpha-labs/Graphenium](https://github.com/lambda-alpha-labs/Graphenium) |
| **Primary Language** | Rust |
| **Graphenium Compiler Version** | v0.24.0 |
| **Indexing Mode** | AST + Stack Graphs Resolver (Offline, Tier 1) |
| **Index Schema Version** | `0.2.0` |
| **AST Symbols Compiled (Nodes)** | 1,410 |
| **Structural Boundaries Resolved (Edges)** | 3,606 |
| **Cohesive Folder Domains (Communities)** | 19 |
| **Analysis Date** | November 6, 2026 |

---

## 2. Pre-Flight Policy & Containment Successes

### Multi-Hop Cross-File Call Resolution
Graphenium's Stack Graphs resolver mapped **1,030 cross-file references** locally and offline. Graphenium established an AST-proven baseline containing:
*   **100% Import Resolution** (330 of 330 physical imports resolved to concrete files).
*   **36% Cross-File Call Resolution** (742 of 2,044 call edges resolved to unique, AST-proven source declarations).
*   **Zero Ambiguity:** Graphenium identified 0 unresolved name collisions across the core module domains.

The overall index confidence profile breaks down as:
*   **43% AST-Proven (`EXTRACTED`):** 1,562 boundaries compiler-proven via tree-sitter or Stack Graphs.
*   **57% Heuristic (`INFERRED`):** 2,044 boundaries inferred via structural call-graph analysis.
*   **0% Ambiguous:** No garbage or un-resolvable collisions in the baseline.

### Domain Partitioning Matches Folder Layout
Graphenium's Louvain clustering partitioned the codebase's 1,410 symbols into 19 folder-focused domains:
*   **Community 0** (221 symbols): Build, detection, classification, and configuration surfaces.
*   **Community 1** (170 symbols): Cache management, language configs, semantic client, and freshness tracking.
*   **Community 3** (152 symbols): MCP server handlers, graph accessors, and tool backends.

### Policy Pre-Flight and Structural Proof
We define a strict layering rule in `.graphenium/policy.json` forbidding the core model (`src/model/**`) from depending on the background MCP server (`src/serve/**`). Graphenium enforces this in two places:
*   **Pre-flight:** `validate_plan` applies the explicit policy rules plus the Louvain ΔQ and surprise-edge gates before code is written.
*   **Post-edit:** `verify_plan` re-checks physical edges originating from approved plan files, closing the loophole where a forbidden edge could be added inside an otherwise-approved file.

The Datalog engine remains available for ad-hoc reachability questions over calls, imports, inheritance, and containment:
```prolog
?- calls_transitive(X, Y), node(X, _, _, "src/model/graph.rs", _), node(Y, _, _, "src/serve/handlers.rs", _).
```
The self-index deliberately includes test-module and inferred heuristic edges, so production-only layer audits should be scoped to the relevant source paths rather than reading the raw closure as a zero-result proof.

---

## 3. Unresolved Compiler Gaps

*   **Dynamic Dispatch Gaps:** Graphenium's static Stack Graphs resolver cannot automatically trace method invocations routed through Rust trait objects (such as `Box<dyn Watcher>`). These boundaries remain un-resolved or require manual overrides.
*   **Unresolved Call Overhead (64%):** While Graphenium successfully bound 36% of calls (742 of 2,044), the remaining 1,302 were left unresolved to preserve Graphenium's strict, AST-proven confidence thresholds, avoiding the injection of unverified guesses.

---

## 4. Bottleneck & Hotspot Analysis

Graphenium's centrality analyzer identified the top five highly coupled symbols in the repository:

| Hotspot Symbol | Total Degree (Coupling) | Source File |
|---|---|---|
| `GrapheniumGraph::upsert_node` | 92 | `src/model/graph.rs` |
| `Edge::extracted` | 74 | `src/model/edge.rs` |
| `GrapheniumServer` | 72 | `src/serve/handlers.rs` |
| `GrapheniumGraph::node_data` | 54 | `src/model/graph.rs` |
| `GrapheniumServer::new` | 51 | `src/serve/handlers.rs` |

This analysis shows coupling concentrated in the graph model's mutation/read API and the MCP server state. The earlier `Manifest::len` artifact is gone: bare `.len()` calls no longer bind across files, so cache implementation details no longer dominate the centrality ranking. The nine `mod.rs` files also now have distinct root-relative IDs instead of collapsing into one `mod` node.

---

## 5. Most Useful Diagnostic Queries

```sh
# Summarize the serve module surface
gm query "serve module handlers mcp" --budget 800

# Audit the build pipeline's dependencies
gm query "graph build extraction" --budget 800

# Trace the transitive blast radius of GrapheniumGraph edits
gm query --datalog "?- depends_transitive('graphenium_graph', X)."
```

---

## 6. Engineering Assessment: Would we deploy this gate again?

**Yes.** Enforcing Graphenium's structural gates on Graphenium itself has prevented several occurrences of architectural drift during development. Forcing AI assistants to prove their designs pre-flight via Datalog before they touch core modules like `src/model/graph.rs` prevents the codebase from decaying into spaghetti code.
