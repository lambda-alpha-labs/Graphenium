# Epistemic Trust and Provenance in AI-Assisted Engineering

Graphenium is built on a fundamental principle of engineering safety: **AI coding agents must operate under a strict, mathematically verifiable distinction between compiler-proven facts, heuristic hypotheses, and identifier collisions.**

Relying on "vibe-coding" or probabilistic vector search leads to structural decay because AI agents treat assumed dependencies as ground truth. Graphenium enforces **epistemic trust boundaries**, mapping your codebase's dependencies with strict, source-backed provenance.

---

## 1. The Provenance Confidence Tiers

Every symbol declaration and dependency edge in Graphenium's structural index carries an explicit confidence classification:

| Confidence | Technical Meaning | Agent Design Policy |
|---|---|---|
| `EXTRACTED` **(Facts)** | AST-proven and compiler-backed. Discovered directly in code by Tree-sitter, Stack Graphs, or Visual Studio solution parsers. | **Planning Backbone:** Safe to plan and build against. (Agent must still read the targeted source files before writing edits). |
| `INFERRED` **(Hypotheses)** | Generatively or heuristically inferred. Discovered via Graphenium's optional semantic pass, naming convention heuristics, or cross-file fuzzy resolution. | **Exploratory Leads:** Treat as a hypothesis. The agent is strictly blocked from editing until it inspects at least one source file to prove the dependency. |
| `AMBIGUOUS` **(Collisions)** | Identifier or namespace collision (e.g., matching a symbol that exists in multiple directories). | **Risk Gated:** Rejects automated edits. The agent is forced to halt, flag the collision, and request human review or execute targeted file-level disambiguation. |

---

## 2. Symbol Resolution States

Graphenium's resolver (`src/resolver.rs`) audits every import, call, and inheritance boundary in the index, assigning a resolution state:

*   **`resolved`:** The target symbol exists as a physical node in the AST index. This is the highest-trust state.
*   **`unresolved`:** The target symbol was referenced but not found in Graphenium's static scan. This indicates a dependency on an external system, standard library, or unindexed vendor package.
*   **`heuristic`:** The connection was linked via heuristic matching rather than explicit AST resolution. It requires source validation before modification.

---

## 3. Structural Extractor Provenance

The `extractor` metadata field discloses exactly which module compiled the relationship into Graphenium's index. This provides complete auditability for human reviewers:

| Extractor | Source Mechanism | Default Trust Profile |
|---|---|---|
| `tree-sitter` | Local syntax parsing | High trust (`EXTRACTED`). Compiler-proven. |
| `resolver` | Local import matching | High trust (`EXTRACTED`). Compiler-proven. |
| `tree-sitter-stack-graphs` | Local cross-file call resolution | High trust (`EXTRACTED` / `INFERRED` unique bindings). |
| `csproj-parser` | Visual Studio project boundaries | High trust (`EXTRACTED`). Solution-proven. |
| `llm` | Remote semantic inference | Medium trust (`INFERRED`). Heuristic guess. |
| `manual-mcp-write` | Programmatic agent injection | Auditable. Only accepted if the agent provides explicit proof of source inspection. |

---

## 4. Enforcing a Strict Trust-Aware Design Contract

Graphenium translates these trust tiers into a strict, programmatic contract for agent execution:

```text
1. Compile virtual planning workspace (Virtual AST).
2. If any planned connection depends on an AMBIGUOUS path:
   └── Fail pre-flight. Force agent to disambiguate.
3. If any planned connection depends on an INFERRED path:
   └── Require agent to read target implementation files first.
4. If plan relies exclusively on resolved, EXTRACTED boundaries:
   └── Approve pre-flight and authorize file edits.
```

---

## 5. Trust Quality Policies vs. Structural Architecture Policies

Graphenium distinguishes between three complementary policy layers in your governance stack:

| Policy Layer | Primary Config | Enforcement Target | Metric Evaluated |
|---|---|---|---|
| **Trust Quality** | `gm check` options | Index-Wide Health | Import resolution ratio, maximum allowed ambiguity, and evidence freshness. |
| **Architecture** | `.graphenium/policy.json` | Agent Design Spec | Forbidden dependencies, strict layering domains, and banned symbols. |
| **Topological Entropy (Zero-Drift)** | Zero-config (always on) | Agent Design Spec | Louvain modularity delta (ΔQ), surprise edge scores, community drift. |

Use **Trust Quality** policies to ensure Graphenium's index is complete and healthy enough to plan against. Use **Architecture** policies to block agents from committing designs that violate declared boundaries. **Topological Entropy** gating runs automatically as a zero-config fallback — even without `.graphenium/policy.json` — rejecting plans that mathematically degrade modularity or introduce structurally anomalous dependencies (`cross-community`, `peripheral→hub`).

### Topological Entropy Boundaries

Unlike human-declared glob patterns in `policy.json`, topological entropy boundaries are **compiler-proven mathematical invariants** derived from the index itself:

| Signal | Source Module | Trust Property |
|---|---|---|
| **Louvain communities** | `src/cluster/louvain.rs` | Compiler-extracted graph structure; deterministic given the same index and seed. |
| **Modularity score (Q)** | `src/cluster/louvain.rs` | Quantifies how well edges cluster within communities. Not a heuristic guess. |
| **Modularity delta (ΔQ)** | `src/analyze/delta.rs` | Relative change between baseline and virtual plan. Evaluates *new* decay, not absolute shape. |
| **Surprise edge score** | `src/analyze/surprise.rs` | Flags structurally anomalous connections (`cross-community`, `peripheral→hub`) with explicit factor breakdown. |

These signals improve epistemic trust because they are computed from AST-proven edges and deterministic graph algorithms — not from natural-language prompts or naming assumptions.

### The Existing Drift Paradox

Legacy codebases often contain pre-existing architectural debt: tangled modules, cross-boundary shortcuts, and uneven community cohesion. A naive linter that enforces idealized shapes (e.g., strict MVC folder layouts) would reject every agent plan on contact with real-world complexity.

Graphenium resolves this paradox through **relative evaluation**:

```text
G_baseline  = physical-only subgraph (current reality)
G_virtual   = physical + proposed plan overlay
ΔQ          = Q(G_virtual) - Q(G_baseline)

Pass if:  ΔQ ≥ tolerance  AND  no high-surprise planned edges
```

The baseline accepts your existing legacy complexity as ground truth. The gate blocks only *new* topological decay introduced by the agent's proposed edges — preserving modularity without forcing a wholesale architectural rewrite.

### Trust Quality Policy Examples:
```sh
# Permissive (Initial repo setup)
gm check --min-resolution 50 --max-ambiguous 50

# Moderate (Standard team workflow)
gm check --min-resolution 70 --max-ambiguous 20

# Strict (High-safety enterprise repository)
gm check --min-resolution 85 --max-ambiguous 5 --strict
```

---

## 6. Security and Trust Anti-Patterns

To prevent generative software erosion, avoid the following operational anti-patterns:

*   **Treating All Dependencies as Equal:** Never let an agent treat a semantic, nearest-neighbor vector "similarity" connection (`INFERRED`) with the same safety profile as a compiler-proven import (`EXTRACTED`).
*   **Allowing Generative Edge Injection:** Do not allow agents to write manual edges into Graphenium's index based on file proximity or naming assumptions alone. Manual writes must be reserved for documented, human-verified design decisions.
*   **Operating on Stale Indexes:** If `graph_info` warns that **"Graph may be stale"**, the loaded index predates recent source edits. Operating on a stale index means the agent is planning against obsolete structural context. Always execute `gm run . --no-semantic --no-viz` and `reload_graph` to hot-swap the server state before planning.