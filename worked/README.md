# Worked Case Studies: Structural Architectural Containment

This directory contains **worked case studies** documenting Graphenium's performance and behavioral guardrails across real-world, production repositories. 

These case studies are designed to demonstrate Graphenium's ability to mechanically enforce engineering safety, detect AI-driven codebase erosion, and block unauthorized transitive dependencies [1.1.2, 1.2.4].

---

## 1. What a High-Value Case Study Illustrates

To maintain technical depth, each documented case study must provide an honest, objective analysis of Graphenium's capabilities, evaluating both its strengths and its limitations:

*   **Compiler-Level Truth vs. Generative Heuristics:** Demonstrating how Graphenium's local AST and Stack Graphs extraction prevents hallucinated dependencies.
*   **Active Pre-Flight Gating:** Illustrating how Graphenium's Datalog solver evaluates proposed design workspaces and blocks violations before physical code is touched [1.1.2].
*   **PR Compliance Audits:** Showing how Graphenium's post-edit verification pipeline (`gm check --plan`) detects scope creep and unplanned edits.
*   **Hotspot Analysis:** Explaining how PageRank, betweenness centrality, and Surprise Connection metrics help agents protect production hot-paths.
*   **System Limitations:** Detailing where static AST extraction has gaps (e.g., dynamic dispatch, runtime reflection) and how these were resolved.

---

## 2. Recommended Case Study Categories

We encourage the addition of new case studies across diverse project profiles to continue stress-testing Graphenium's structural compilers:

| Repository Profile | Core Evaluation Goal | Focus Metric |
|---|---|---|
| **Large C# Solution** | Visual Studio solution and compilation boundaries. | Cross-project DLL dependency gating and namespace mapping. |
| **Python Web API** | Dynamic typing and framework imports. | Evaluating local AST resolution ratios without static type declarations. |
| **TypeScript Frontend App** | Component and utility boundary coupling. | Restricting utility imports and enforcing clean UI-to-state separation. |
| **Rust Compiler Tool** | Complex type dependencies and traits. | Exhaustive trait implementations and nested module parsing. |
| **Mixed Monorepo** | Multi-language compilation boundaries. | Enforcing workspace safety across language-family boundaries. |

---

## 3. How to Document a New Case Study

To add a new case study to this directory:

1.  **Duplicate the Template:** Copy the standard template file at [`worked/TEMPLATE.md`](TEMPLATE.md) to your new target directory.
2.  **Initialize Graphenium:** Run `gm init` and compile the codebase's local index:
    ```sh
    gm run . --no-semantic --no-viz
    ```
3.  **Analyze Diagnostic Health:** Run `gm doctor --resolution` to collect import resolution ratios, symbol counts, and boundary statistics.
4.  **Configure Architectural Policies:** Define strict structural rules inside `.graphenium/policy.json` (e.g., forbidding direct database imports from controllers).
5.  **Run Gating Scenarios:** Test if Graphenium's pre-flight Datalog engine successfully blocks violations and if the post-edit compliance pipeline catches unplanned modifications.
6.  **Commit the Study:** Save your finalized case study files and submit a pull request for review.
