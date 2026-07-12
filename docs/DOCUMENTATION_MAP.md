# Graphenium Documentation Ecosystem Map

This documentation set is structured entirely around Graphenium's core mission: **providing an external, pre-flight linter and mechanical containment gate for AI coding agents**. 

The documents have been systematically rewritten to move away from passive "graph visualization" or "GraphRAG" concepts, focusing instead on write-safety, deterministic AST-level boundaries, and Datalog policy solving.

---

## 1. Documentation Taxonomy and Placement

The Graphenium documentation suite is distributed across the following key locations in the repository:

```text
README.md                       # The main technical entry point & quick start
AI_SETUP.md                     # Assistant setup playbook (compiled into workspace)
CHANGELOG.md                    # Release history & structural milestones
CONTRIBUTING.md                 # Contributor guide & compiler module reference
SECURITY.md                     # Local-first security & sensitive file rules

docs/
  ├── DOCUMENTATION_MAP.md      # This file: Guide to Graphenium's documentation
  ├── GETTING_STARTED.md        # Initialization, AST indexing, & MCP handshake
  ├── POSITIONING.md            # Market positioning & competitive differentiation
  ├── AGENT_WORKFLOWS.md        # Step-by-step pre-flight and verify loops
  ├── CI_AND_GOVERNANCE.md      # CI/CD gates, git hooks, & policy.json schemas
  ├── COMMAND_REFERENCE.md      # Complete CLI flags, commands, & outputs
  ├── MCP_TOOLS.md              # MCP tool schemas & behavioral protocols
  ├── ARCHITECTURE.md           # Tree-sitter pipelines, C# bounds, & Datalog solving
  ├── TRUST_MODEL.md            # AST-proven vs. semantic provenance
  ├── BENCHMARKING.md           # Payload budgets, latency metrics, & TTVP
  ├── COMPARISON.md             # Graphenium vs. grep, ast-grep, & vector indexes
  ├── HARNESS_ADAPTER.md        # Programmatic Rust integration reference
  └── GRAPH_REPORT.md           # Diagnostic guidelines for codebase reports

skills/
  └── graphenium/
        └── SKILL.md            # In-context agent rules & response pattern templates

worked/
  ├── README.md                 # Overview of structural containment case studies
  ├── TEMPLATE.md               # Standard report template for new codebase studies
  └── graphenium-self-analysis/  # Base study: Graphenium analyzed by its own engine
```

---

## 2. Refactoring Graphenium's Message Architecture

Our documentation refactoring enforces a strict paradigm shift in terminology and concepts to bypass "graph fatigue" and connect directly with enterprise developers:

| Old Concept (Passive & Saturated) | New Concept (Active & High-Value) |
| :--- | :--- |
| **Durable Structural Memory** | **AST-Proven Codebase Index / Pre-Flight Gate** |
| Helping agents search and navigate the codebase. | **Stopping agents from violating boundaries and creating code bloat [1.2.4].** |
| Passive visualization diagrams (HTML outputs). | **Automated pre-commit hooks, Datalog proofs, and CI gates [1.1.2, 1.1.6].** |
| Fictional "knowledge graph RAG" similarity guesses. | **Compiler-backed facts (`EXTRACTED`) vs. exploratory hypotheses (`INFERRED`).** |
| AI prompts (`AGENTS.md`, `CLAUDE.md`) | **External, mechanical constraints (`policy.json` / `verify_plan`) [1.1.6].** |

---

## 3. Primary Documentation Audiences

Graphenium's documentation set is written to address three distinct engineering profiles:

1.  **Platform & DevEx Engineers:** Targeted by `docs/CI_AND_GOVERNANCE.md`, `docs/GETTING_STARTED.md`, and `AI_SETUP.md`. They need to know how to install `gm`, construct automated pre-commit gates, and standardize agent containment across development teams.
2.  **Software Architects & Tech Leads:** Targeted by `docs/POSITIONING.md`, `docs/ARCHITECTURE.md`, `docs/TRUST_MODEL.md`, and `.graphenium/policy.json` schemas. They need to know how Graphenium uses Datalog to mathematically prove boundary safety and how to declare strict module layers [1.1.2].
3.  **AI Assistant Engines:** Targeted by `skills/graphenium/SKILL.md` and `docs/MCP_TOOLS.md`. These files provide structured rules and handshake protocols that the agent parses to govern its own change planning.

---

## 4. Fundamental Architectural Assertion
Every document in Graphenium's repository reinforces a single engineering assertion:

> **AI coding agents cannot self-police. Enforcing codebase integrity at generative scale requires an external, mechanical structural compiler.**
