# Structural Codebase Audit: GRAPH_REPORT.md

Whenever Graphenium parses and indexes your repository, it automatically compiles a local structural diagnostic report:
```text
graphenium-out/GRAPH_REPORT.md
```

This report acts as a **comprehensive architectural audit and compiler diagnostic summary** of your codebase. It compresses Graphenium's structural analysis—module boundaries, coupled hotspots, dependency anomalies, and resolution gaps—into a single, reviewable document for human architects and AI assistants.

---

## 1. Typical Report Sections and Diagnostics

The compiled `GRAPH_REPORT.md` consists of nine automated diagnostic sections:

| Diagnostic Section | Purpose | System Insight |
|---|---|---|
| **1. Corpus Warnings** | Flags parser or walk anomalies. | Highlights unindexed, unreadable, or missing files. |
| **2. Summary Table** | Structural scale metrics. | Lists total compiled symbols, dependencies, and our `EXTRACTED` vs. `INFERRED` trust ratios. |
| **3. God Nodes** | Highly coupled hotspots. | Highlights structural bottlenecks and risk areas. |
| **4. Surprising Connections** | Boundary anomalies. | Identifies unexpected dependencies crossing Top-Level directories or folder domains. |
| **5. Hyperedges** | Multi-symbol groupings. | Shows N-ary relationships (3+ symbols participating in a shared design concept). |
| **6. Communities** | Cohesive folder domains. | Lists Louvain-partitioned module clusters, members, and internal cohesion scores. |
| **7. Ambiguous Edges** | Identifier collisions. | Highlights name collisions that require physical disambiguation or manual review. |
| **8. Knowledge Gaps** | Isolated symbols. | Lists symbols with zero compiled connections (possible dead code or unindexed modules). |
| **9. Suggested Questions** | Automated review prompts. | Generates structural review questions based on modular coupling and bottleneck analysis. |

---

## 2. Hand-off Guidelines: How Agents and Humans Use the Report

### How AI Coding Agents Must Use the Report
Agents must treat the report as a high-level architectural map for **orientation, not implementation truth**. 
*   **Do:** Use the report to identify high-risk hub symbols before proposing edits.
*   **Do:** Check for `AMBIGUOUS` symbol collisions to avoid writing incorrect class or method references.
*   **Do:** Analyze the `Suggested Questions` block to generate safety checklists for their design plans.
*   **Do Not:** Substitute reading Graphenium's report for direct, file-level source code inspection.
*   **Do Not:** Assume `INFERRED` semantic connections are compiler-proven dependencies without reading the corresponding implementation files.

### How Human Architects and Reviewers Use the Report
*   **Monitor Architectural Erosion:** Review the *Surprising Connections* section during PR reviews to instantly catch if an agent snuck in a dependency that couples unrelated folder domains.
*   **Assess AI Code Bloat:** Use the *Knowledge Gaps* section to detect if an agent's modifications introduced completely isolated, unused classes or duplicate helper methods (dead code).
*   **Audit Refactoring Risk:** Check the *God Nodes* table. If an agent-generated PR proposes edits to a highly coupled hotspot, require strict pre-flight planning and extra unit-test coverage.

---

## 3. Regenerating and Disabling the Audit

Graphenium regenerates the report automatically on every full codebase compilation:
```sh
gm run . --no-semantic --no-viz
```

### Disabling Report Writes
If you are running Graphenium's index compiler in a resource-constrained CI container or background daemon where only raw JSON is required, disable Markdown compilation to save disk operations:
```sh
gm run . --no-semantic --no-viz --no-report
```