# Package Manifest

This manifest indexes all core documentation and playbook assets maintained in Graphenium's repository. These files define our positioning as an external structural linter and containment gate, protecting codebases from AI-driven architectural drift.

---

## Root Directory
*   `README.md` — Graphenium's primary positioning, capabilities overview, and fast pre-flight workflow guide.
*   `AI_SETUP.md` — Environmental playbook for assistants installing and verifying local linter controls.
*   `CHANGELOG.md` — Graphenium release and milestone history.
*   `CODE_OF_CONDUCT.md` — Project code of conduct and community enforcement policies.
*   `CONTRIBUTING.md` — Developer contribution guidelines, package module index, and extractor templates.
*   `LICENSE` — Legal MIT license text.
*   `SECURITY.md` — Security policy, local-first data guarantees, and secret file exclusion rules.

---

## Core Documentation Set (`docs/`)
*   `docs/DOCUMENTATION_MAP.md` — Documentation directory restructure guide.
*   `docs/GETTING_STARTED.md` — Workspace initialization, index generation, and baseline workspace setups.
*   `docs/POSITIONING.md` — Analysis of competitive differentiation, target audiences, and messaging core.
*   `docs/AGENT_WORKFLOWS.md` — Step-by-step operating playbooks for AI agents (pre-flight, in-edit planning, post-facto verification).
*   `docs/CI_AND_GOVERNANCE.md` — CI pipeline gates, git hooks, PR comment templates, and declarative policy schemas.
*   `docs/COMMAND_REFERENCE.md` — Syntax and flag definitions for Graphenium's CLI commands (`gm run`, `gm check`, `gm query`, etc.).
*   `docs/MCP_TOOLS.md` — Tool descriptions and behavioral guidelines for agents connected via MCP.
*   `docs/ARCHITECTURE.md` — Deep technical dive into Graphenium's AST parsing pipelines, C# project bounds, and Datalog solvers.
*   `docs/TRUST_MODEL.md` — Technical reference defining AST-proven vs. semantic provenance.
*   `docs/BENCHMARKING.md` — Performance testing methodologies: latencies, token budgets, and scaling limits.
*   `docs/COMPARISON.md` — Context comparison matrix mapping Graphenium against standard developer tools (grep, ast-grep, RAG indexes).
*   `docs/HARNESS_ADAPTER.md` — Reference guide for programmatically embedding Graphenium's containment engine as a library.
*   `docs/GRAPH_REPORT.md` — Diagnostic guidelines for interpreting generated codebase report summaries.
*   `docs/LICENSE.md` — Legal MIT license text *(.md copy)*.
*   `docs/AI_SETUP.md` — Assistant setup playbook *(.md copy)*.
*   `docs/CHANGELOG.md` — Release history *(.md copy)*.
*   `docs/CODE_OF_CONDUCT.md` — Community guidelines *(.md copy)*.
*   `docs/CONTRIBUTING.md` — Developer guidelines *(.md copy)*.
*   `docs/SECURITY.md` — Security policies *(.md copy)*.

---

## Integration Adapters (`contrib/`)
*   `contrib/harness-adapter/README.md` — Integration notes for embedding Graphenium's engine inside third-party agent harnesses.

---

## Agentic Interface Skills (`skills/`)
*   `skills/graphenium/SKILL.md` — Standard instructions and rules injected into Graphenium-aware agent workspaces.

---

## Worked Base Case Studies (`worked/`)
*   `worked/README.md` — Introduction to structural case studies.
*   `worked/TEMPLATE.md` — Standard template for documenting new codebase verification studies.
*   `worked/graphenium-self-analysis/README.md` — Self-analysis verification study applying Graphenium to its own Rust repository.
*   `worked/graphenium-self-analysis/GRAPH_REPORT.md` — Automated codebase report output for Graphenium's repository.
*   `worked/graphenium-self-analysis/sample-queries.md` — Practical query examples illustrating AST-proven cross-file resolution.
