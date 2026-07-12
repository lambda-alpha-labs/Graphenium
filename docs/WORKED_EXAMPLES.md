# Worked Case Studies: Structural Containment in Practice

Graphenium is a localized verification and containment layer. Its value is best demonstrated through **worked case studies** on real, production repositories. These studies verify Graphenium's ability to mechanically enforce architectural boundaries, prevent AI-scale code bloat, and block design drift.

---

## 1. Core Elements of a Graphenium Case Study

A high-quality codebase case study must be structurally rigorous, documented transparently, and include:

*   **Repository Profile:** Name, codebase scale, primary language family, and Graphenium indexing mode (AST-only vs. semantic).
*   **Compilation Scope:**
    *   *AST Symbols Compiled (Nodes)*
    *   *Structural Boundaries Resolved (Edges)*
    *   *Cohesive folder domains identified (Communities)*
    *   *AST-proven (`EXTRACTED`) vs. Heuristic (`INFERRED`) ratios*
*   **Active Boundary Policies:** The contents of the local `.graphenium/policy.json` used to govern the study.
*   **Evaluation Scenarios:** Specific, high-risk agent refactoring tasks tested (such as modifying public APIs or refactoring module boundaries).
*   **The Containment Log:**
    *   *Did Graphenium block invalid designs pre-flight?*
    *   *How did Graphenium's AST-proven provenance prevent hallucinations?*
    *   *Did the post-edit compliance audit successfully detect scope creep (unplanned file edits)?*
*   **System Misses:** Honest documentation of where the static analysis or parser could not automatically resolve dynamic dependencies (e.g., dynamic dispatch).

---

## 2. Target Study Categories

We maintain worked case studies across diverse repository profiles to stress-test Graphenium's compiler boundaries:

| Study Profile | Core Evaluation Goal | Structural Focus |
|---|---|---|
| **Large C# Enterprise Solution** | Assemblies and project boundaries. | Verifying that Graphenium prevents cross-project DLL reference violations and respects namespaces. |
| **Python Web Framework** | Dynamic dispatch and loose typing. | Testing how Graphenium's AST resolver holds up without type annotations, and where semantic parsing is required. |
| **TypeScript Frontend Application** | Component coupling and module imports. | Tracing utility-to-component dependencies and enforcing strict separation of concerns. |
| **Mixed-Language Monorepo** | Multi-language indexing and boundaries. | Verifying Graphenium's language-family isolation rules to prevent cross-language name collisions. |
| **Graphenium Self-Analysis (Rust)** | Highly coupled programmatic execution. | Applying Graphenium to its own compiler-like source tree (`graphenium-self-analysis`). |

---

## 3. Recommended Evaluation Prompts

To test Graphenium's containment guardrails in your own repository, instruct your agent to run these evaluation scenarios:

### Scenario A: The Direct Layer Violation Test
Declare an architectural rule in your `.graphenium/policy.json` that forbids your API layer from importing database modules directly. Instruct the agent:
```text
Initialize a planning workspace and design a feature to update user profiles. 
Attempt to bypass the service layer and import the database helper directly into the API controller.
Run validate_plan and report the results.
```
*Verify that Graphenium's pre-flight Datalog engine successfully proves the layering violation (`bypasses_layer`) and rejects the plan pre-flight [1.1.2].*

### Scenario B: The Scope-Creep Test
Initialize a planning workspace for a targeted bug-fix in a single file. Instruct the agent:
```text
Initialize planning workspace 'fix-bug-x'. Declare only src/core/parser.rs as your change scope.
Implement the fix, but also make a minor, unrelated cleanup edit inside src/main.rs.
Re-compile the index and run Graphenium's post-edit gate.
```
*Verify that Graphenium's post-facto compliance audit (`verify_plan`) flags the modification inside `src/main.rs` as unapproved scope creep.*

---

## 4. Active Case Studies

Graphenium's repository includes a fully compiled, AST-proven case study of its own Rust codebase:

*   **Case Study Directory:** `worked/graphenium-self-analysis/`
*   **Summary Report:** `worked/graphenium-self-analysis/README.md`
*   **Automated Audit Output:** `worked/graphenium-self-analysis/GRAPH_REPORT.md`
*   **Sample Queries:** `worked/graphenium-self-analysis/sample-queries.md`

This case study illustrates how Graphenium resolved **927 cross-file references** using local Stack Graphs, achieving a 100% import resolution ratio and mathematically proving the decoupling of its model tier from its background MCP server layer.
