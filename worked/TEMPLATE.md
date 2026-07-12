# Worked Case Study Template: <Repository Name>

## 1. Repository Profile

| Property | Value |
|---|---|
| **Repository Name** |  |
| **Source URL** |  |
| **Programming Language(s)** |  |
| **Graphenium Version** |  |
| **Compilation Mode** | AST-Only / Semantic |
| **Date Analyzed** |  |

---

## 2. Codebase Index Summary

Graphenium's local compiler output statistics:

| Metric | Compiled Count / Ratio |
|---|---:|
| **AST Symbols Compiled (Nodes)** |  |
| **Structural Boundaries Resolved (Edges)** |  |
| **Cohesive Domains Identified (Communities)** |  |
| **AST-Proven (`EXTRACTED`) Boundaries** |  |
| **Heuristic (`INFERRED`) Connections** |  |
| **Identifier Collisions (`AMBIGUOUS`)** |  |
| **AST Import Resolution Ratio** |  |

---

## 3. Setup and Indexing Commands

```sh
# Workspace initialization
gm init

# Local baseline index compilation (AST-proven)
gm run . --no-semantic --no-viz

# Resolution ratio audit
gm doctor --graph graphenium-out/graph.json --resolution
```

---

## 4. Primary Gating and Linter Queries

```sh
# Hybrid structural domain query
gm query "<feature_or_symbol>" --mode hybrid --budget 3000

# Strict AST-proven transitive boundary query
gm query "<symbol>" --safe --budget 1500

# Multi-hop layering violation proof via Datalog
gm query "layer-bypass" --datalog "?- bypasses_layer('<src>', '<layer>', '<target>')."

# Pre-edit blast radius calculation
gm check --graph graphenium-out/graph.json --plan <id>
```

---

## 5. Pre-Flight Policy & Linter Successes

*   *Detail how Graphenium's local AST index successfully mapped the codebase's boundaries.*
*   *Explain which declarative policies in `.graphenium/policy.json` were triggered.*
*   *Describe how Graphenium's pre-flight Datalog engine successfully proved structural violations before code was written.*

---

## 6. Unresolved Compiler Gaps

*   *Identify where Graphenium's static Tree-sitter parser or Stack Graphs resolver had gaps (e.g., dynamic dependency injection, reflection, or un-annotated types).*
*   *Document any un-resolved references or false-positive connections that required manual index overrides.*

---

## 7. Provenance & Hand-off Observations

*   *What percentage of the dependencies were AST-proven (`EXTRACTED`) versus heuristic guesses (`INFERRED`)?*
*   *Which boundaries required direct, file-level source inspection by the agent?*
*   *How many identifier collisions (`AMBIGUOUS`) were flagged, and how were they resolved?*

---

## 8. Agent Containment Workflow Impact

*   *Describe how Graphenium's pre-flight design gating changed the agent's task execution.*
*   *Did the post-edit compliance audit successfully detect any scope creep (unplanned file modifications)?*
*   *How many unnecessary file reads (context bloat) did Graphenium prevent the agent from executing?*

---

## 9. Engineering Assessment: Would I deploy this gate again?

*   *Answer objectively. Detail if Graphenium's containment guardrails were structurally rigorous and robust enough to prevent software erosion in this project.*

---

## 10. Notes for Maintainers

*   *List any grammar improvements, parser edge cases, or standard library Datalog rule additions revealed during this codebase study.*
