# Agentic Containment Workflows

Graphenium establishes strict, write-time containment boundaries for AI coding agents. The core rule governing all agent interactions is simple:

> **Verify boundaries pre-flight, read source files before implementing, and audit scope creep post-edit.**

This guide outlines seven automated workflows to integrate Graphenium's structural gates directly into active engineering sessions.

---

## Workflow 1: Pre-Edit Structural Gating
Use this workflow whenever an agent is tasked with modifying a public interface, database helper, endpoint controller, or module boundary.

### Core Premise
AI agents often choose the path of least resistance (e.g., importing a module that bypasses your repository layers). Graphenium forces the agent to analyze the structural neighborhood of a target symbol before writing code.

### Recommended Tool Sequence
1.  `graph_info` — Verify index integrity and freshness.
2.  `analyse_symbol` — Retrieve the target symbol's metadata, callers, and current dependency profile.
3.  `get_neighbors` (with `extracted_only` active) — Identify direct callers and upstream dependencies.
4.  `query_transitive` — Inspect the transitive closure of the symbol to evaluate the blast radius across multi-hop paths.
5.  `next_files_to_read` — Identify the primary implementation files that must be read to understand the structural context.
6.  **Human/Agent hand-off:** The agent reads the implementation files directly before writing code.

### Expected Agent Output Shape
```text
Target Symbol Resolved: auth.validate_token
AST-Proven Provenance: 32 EXTRACTED, 4 INFERRED, 0 AMBIGUOUS
Direct Dependent Callers: routes.account, middleware.require_session
Must-Read Implementation Files: src/auth/session.py, src/middleware/session.py
Pre-Flight Boundary Status: Checked. No strict layering rules violated.
Proposed Change Plan: [Brief, structural summary of file edits]
```

---

## Workflow 2: Module Boundary Orientation
Use this workflow when an agent initializes a session in a large, unfamiliar, or high-change-rate repository.

### Core Premise
AI agents waste context budget by reading raw files or grepping blindly to understand a system's structure. Graphenium provides an automated, local summary of the system's modules and folder domains.

### Recommended Tool Sequence
1.  `graph_info` — handshake and verify index size.
2.  `architecture_summary` — Extract the top-level structural highlights, including folder domains and hotspots.
3.  `god_nodes` — Identify highly connected hub nodes and structural bottlenecks.
4.  `get_community` — Inspect member lists of specific structural domains.

### Expected Agent Behavior
The agent must clearly report what is compiler-proven and what still requires physical file reads, avoiding assumptions about implementation:
```text
I have parsed Graphenium's local index. The system contains three primary domains:
- API Domain (Community 1): Bounded by src/api/. Contains routing and handler logic.
- Core Business Domain (Community 2): Bounded by src/core/. Contains service objects and domain logic.
- Data Access Domain (Community 3): Bounded by src/db/. Contains repository and data-layer modules.

I will now read the domain boundary files to confirm the structural layout before proposing any multi-file changes.
```

---

## Workflow 3: Dependency Path Auditing
Use this workflow to trace the exact structural path between two symbols and validate every connection's provenance.

### Core Premise
AI agents can easily hallucinate dependency chains that do not physically exist. Graphenium forces agents to trace deterministic, AST-proven paths and halt on gaps that require manual inspection.

### Recommended Tool Sequence
1.  `shortest_path` (semantic mode) — Find the high-provenance route between two symbols.
2.  `safest_path` — Find the path with the highest aggregate confidence score.
3.  For any segment marked `INFERRED` or `AMBIGUOUS`, the agent must execute `get_node` to retrieve the exact source file.
4.  If `AMBIGUOUS` collisions are detected, the agent must flag them and request human disambiguation before continuing.

---

## Workflow 4: Pre-Flight Design Check (Interactive Policy Gate)
Use this workflow whenever the user asks the agent to make a multi-file, cross-boundary, or architectural change.

### Core Premise
Agents should not blindly edit files and hope they respect your architecture. Graphenium forces the agent to declare its design intent and mathematically validate compliance against your `.graphenium/policy.json` before it writes a single line of code.

### Step-by-Step Execution:
1.  **Initialize Workspace:** Execute `create_planning_workspace` with a descriptive ID for the task (`refactor-session-handling`).
2.  **Declare Design spec:** Execute `add_planned_symbol` for every new or modified class, struct, function, interface, or dependency edge the agent plans to introduce.
3.  **Pre-Flight Policy Solver:** Execute `validate_plan`. Graphenium runs its embedded Datalog solver to check the virtual plan against:
    *   `forbidden_dependency` rules (Direct banned import paths)
    *   `strict_layering` rules (Transitive layer bypasses)
    *   `banned_symbol` rules (proposed accesses to disallowed modules)
4.  **Block on Failure:** If Graphenium returns `PRE_FLIGHT_VIOLATION`, stop immediately and report to the user which rule was violated, the exact violating dependency path, and the recommended structural fix.
5.  **Implement:** If pre-flight passes, authorize file edits.

---

## Workflow 5: Post-Edit Scope-Creep Audit
Use this workflow after the agent implements a multi-file change to confirm physical compliance with the approved plan.

### Core Premise
Scope creep is one of the most dangerous failure modes of AI coding agents. An agent tasked with fixing a parser may quietly modify a routing file. Graphenium's post-edit audit catches this.

### Step-by-Step Execution:
1.  **Re-Compile the Index:** Run `gm run . --no-semantic --no-viz` locally to generate an updated physical codebase index.
2.  **Hot-Swap Server State:** Call `reload_graph` to sync the running MCP server without restarting it.
3.  **Executive Audit:** Call `agent_change_gate` and pass the same `plan_id` used during pre-flight. Graphenium evaluates:
    *   **Resolution Health:** Are AST import and call ratios still above threshold?
    *   **Pre-Flight Compliance:** Is the physical code still compliant with `.graphenium/policy.json`?
    *   **Scope-Creep Audit:** Did the agent modify any files not declared in the approved plan?
4.  **Generate Review Plan:** Call `verification_plan` to retrieve a risk-sorted list of targeted tests.
5.  **Evaluate Downstream Impact:** Call `blast_radius` to verify that changed symbols did not accidentally break dependent callers.

When this workflow completes, the agent must report:
*   **Unplanned Edits (Scope Creep):** If the agent modified files outside the declared plan, Graphenium fails the build and flags the unapproved edits.
*   **Missing Implementations:** If the agent failed to write a class it declared in the plan, Graphenium flags it.

---

## Workflow 6: CI/CD Architecture Gates
Enforce Graphenium's structural rules automatically in your GitHub Actions, GitLab CI, or pre-commit hooks to block bad AI code before review.

### Baseline Gate
Verify that the codebase index meets minimum import and call resolution targets, and contains no high-risk ambiguity:
```sh
gm check --graph graphenium-out/graph.json --min-resolution 80 --max-ambiguous 10
```

### Strict Policy Gate (Combining Pre-Flight & Post-Facto)
Fail the build if the agent's implemented code deviates from its approved planning workspace or violates `.graphenium/policy.json` boundaries:
```sh
gm check --graph graphenium-out/graph.json --plan "refactor-session-handling" --strict
```

---

## Workflow 7: Manual Index Corrections & Overrides
Use this workflow when compiler parsing cannot automatically detect a critical runtime dependency (e.g., dynamic dependency injection or framework-based naming conventions).

### Core Premise
Do not let agents guess or invent relationships. If a dependency is verified by documentation or tests, write a manual, source-backed override to keep the index accurate.

### Recommended Tool Sequence
1.  `add_node` — Add the logical concept, external API, or framework endpoint with explicit documentation-backed reasons.
2.  `add_edge` — Add a verified relationship with `EXTRACTED` confidence.
3.  `recluster` — Re-run community domain partitioning to update structural boundaries.
4.  `agent_change_gate` — Run compliance gates to verify that the manual override resolved the boundary gap.

---

## Strict Agentic Behavior Rules

An agent running in a Graphenium-governed workspace must adhere to the following rules:

*   **Handshake First:** Always call `graph_info` before performing any codebase edits to verify index freshness.
*   **Check Policies Pre-Flight:** Never implement code for a multi-file task without initializing a planning workspace and running `validate_plan`.
*   **Prefer AST Truth:** Build change paths on `EXTRACTED` (AST-proven) dependencies. Treat `INFERRED` paths as hypotheses, and stop to inspect code when a path hits `AMBIGUOUS` collisions.
*   **No Redundant Reads:** Use Graphenium's structural summaries to target files precisely, avoiding context-bloating folder sweeps.
*   **Inspect Scope Post-Edit:** Always run a post-edit audit (`verification_plan` and `blast_radius`) before declaring a task complete.

### Forbidden Behaviors:
*   *Do not* bypass Graphenium's pre-flight check by writing code directly.
*   *Do not* write manually-verified edges into Graphenium unless you have read the source and confirmed the dependency.
*   *Do not* ignore `AMBIGUOUS` warnings or unresolved references during planning.

---

## One Prompt to Rule Them All
Copy and paste this instruction block into your agent's system instructions, `CLAUDE.md`, or initial session prompt:

```text
You are operating in a Graphenium-contained workspace. 

Before editing any files:
1. Call graph_info to verify index freshness. If stale, run 'gm run . --no-semantic --no-viz' first.
2. Resolve your target symbol, identify direct callers, and trace downstream transitive dependencies.
3. If this is a multi-file task, initialize a planning workspace via create_planning_workspace, declare your intended classes/files, and run validate_plan. Do not edit source files if pre-flight fails.
4. Read only the implementation files recommended by Graphenium. Do not sweep directories blindly.

After editing files:
1. Re-run 'gm run . --no-semantic --no-viz' to update the local index.
2. Run verification_plan and blast_radius, then generate a structural PR audit.
```