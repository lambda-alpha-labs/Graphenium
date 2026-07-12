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

Graphenium distinguishes between two independent policy layers in your governance stack:

| Policy Layer | Primary Config | Enforcement Target | Metric Evaluated |
|---|---|---|---|
| **Trust Quality** | `gm check` options | Index-Wide Health | Import resolution ratio, maximum allowed ambiguity, and evidence freshness. |
| **Architecture** | `.graphenium/policy.json` | Agent Design Spec | Forbidden dependencies, strict layering domains, and banned symbols. |

Use **Trust Quality** policies to ensure Graphenium's index is complete and healthy enough to plan against. Use **Architecture** policies to block agents from committing bad designs.

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