# Agentic Containment Workflows

Graphenium compiles your workspace into a provenance-tagged relational index and establishes strict, write-time containment boundaries for AI coding agents. Use the index first to replace blind grep/file-read loops, then use the containment loop to govern multi-file writes. The core rule governing all agent interactions is simple:

> **Verify boundaries pre-flight, read source files before implementing, and audit scope creep post-edit.**

This guide outlines seven automated workflows to integrate Graphenium's structural gates directly into active engineering sessions.

---

## Workflow 1: Pre-Edit Structural Gating
Use this workflow whenever an agent is tasked with modifying a public interface, database helper, endpoint controller, or module boundary.

### Core Premise
AI agents often choose the path of least resistance (e.g., importing a module that bypasses your repository layers). Graphenium forces the agent to analyze the structural neighborhood of a target symbol before writing code.

### Recommended Tool Sequence
1.  `get_graph_info` - Verify index integrity and freshness.
2.  `inspect_symbol` with `include_analysis = true` - Retrieve the target symbol's metadata, callers, and current dependency profile.
3.  `inspect_symbol` with `include_neighbors = true` and `extracted_only = true` - Identify direct callers and upstream dependencies.
4.  `trace_path` with `mode = "transitive_forward"` - Inspect the transitive closure of the symbol to evaluate the blast radius across multi-hop paths.
5.  `verify_changes` with `target_symbols` - Identify the primary implementation files that must be read to understand the structural context.
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
1.  `get_graph_info` - handshake and verify index size.
2.  `get_architecture_summary` - Extract the top-level structural highlights, including folder domains and hotspots.
3.  `get_architecture_summary` with `include_god_nodes = true` - Identify highly connected hub nodes and structural bottlenecks.
4.  `get_architecture_summary` with `community_id` - Inspect member lists of specific structural domains.

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
1.  `trace_path` with `mode = "shortest"` and `path_mode = "semantic"` - Find the high-provenance route between two symbols.
2.  `trace_path` with `mode = "safest"` - Find the path with the highest aggregate confidence score.
3.  For any segment marked `INFERRED` or `AMBIGUOUS`, the agent must execute `inspect_symbol` to retrieve the exact source file.
4.  If `AMBIGUOUS` collisions are detected, the agent must flag them and request human disambiguation before continuing.

---

## Workflow 4: Pre-Flight Design Check (Interactive Policy Gate)
Use this workflow whenever the user asks the agent to make a multi-file, cross-boundary, or architectural change.

### Core Premise
Agents should not blindly edit files and hope they respect your architecture. Graphenium forces the agent to declare its design intent and mathematically validate compliance against your `.graphenium/policy.json` before it writes a single line of code.

### Step-by-Step Execution:
1.  **Initialize Workspace:** Execute `create_planning_workspace` with a descriptive ID for the task (`refactor-session-handling`).
2.  **Declare Design spec:** Execute `add_planned_symbol` for every new or modified class, struct, function, interface, or dependency edge the agent plans to introduce.
3.  **Pre-Flight Policy Solver:** Execute `validate_plan`. Graphenium checks the virtual plan against:
    *   `forbidden_dependency` rules (direct banned import paths)
    *   `strict_layering` rules (transitive layer bypasses)
    *   `banned_symbol` rules (proposed accesses to disallowed modules)
    *   **Dynamic Delta Gating** (zero-config fallback): modularity delta (ΔQ) and surprise edge profiling
4.  **Iterative Delta Refinement (optional):** If the plan fails on topological entropy, call `validate_plan` with `skip_policy_rules = true` to inspect Delta Q, high-surprise edges, and drift events. Re-plan through intermediate services or interfaces, then re-evaluate.
5.  **Block on Failure:** If Graphenium returns `PRE_FLIGHT_VIOLATION` or a delta gate rejection, stop immediately and report the violating dependency path and recommended structural fix.
6.  **Implement:** If pre-flight passes, authorize file edits.

---

## Workflow 5: Post-Edit Scope-Creep Audit
Use this workflow after the agent implements a multi-file change to confirm physical compliance with the approved plan.

### Core Premise
Scope creep is one of the most dangerous failure modes of AI coding agents. An agent tasked with fixing a parser may quietly modify a routing file. Graphenium's post-edit audit catches this.

### Step-by-Step Execution:
1.  **Re-Compile the Index:** Run `gm run . --no-semantic --no-viz` locally to generate an updated physical codebase index.
2.  **Hot-Swap Server State:** Call `reload_graph` to sync the running MCP server without restarting it.
3.  **Executive Audit:** Call `verify_changes` with the same `plan_id` and the changed `target_symbols` used during pre-flight. Graphenium evaluates:
    *   **Resolution Health:** Are AST import and call ratios still above threshold?
    *   **Pre-Flight Compliance:** Is the physical code still compliant with `.graphenium/policy.json`?
    *   **Scope-Creep Audit:** Did the agent modify any files not declared in the approved plan? Graphenium compares git-status modified files (or an explicit snapshot diff) against the plan's declared files, so untouched baseline files are not falsely flagged.
    *   **Planned-Edge Audit:** Were all declared dependency edges realized in the physical index?
4.  **Generate Review Plan:** `verify_changes` returns a risk-sorted list of targeted tests and must-read files.
5.  **Evaluate Downstream Impact:** `verify_changes` also returns the blast radius so you can verify that changed symbols did not accidentally break dependent callers.

When this workflow completes, the agent must report:
*   **Unplanned Edits (Scope Creep):** If the agent modified files outside the declared plan, Graphenium fails the build and flags the unapproved edits.
*   **Missing Implementations:** If the agent failed to write a class it declared in the plan, Graphenium flags it.
*   **Missing Planned Edges:** If the agent declared a dependency edge but the physical index contains no corresponding edge, Graphenium reports it as a missing planned edge.

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

### Topological Delta Gate (Zero-Config Modularity Protection)
Fail the build if a proposed plan degrades community modularity or introduces high-surprise architectural shortcuts:
```sh
gm check --graph graphenium-out/graph.json --delta --plan "refactor-session-handling"
```

### PR Diff Gate
Compare a pre-change snapshot with the current index and fail on critical architectural drift:
```sh
gm gate --diff graphenium-snapshots/before-change.json graphenium-out/graph.json
```

### Read-Only Freshness Gate
Fail when the committed index is older than the checked-out source, without mutating the workspace:
```sh
gm check --graph graphenium-out/graph.json --verify-fresh
```

### Lean 4 Trust Gate
For formal-verification repositories, bake in an externally generated `#print axioms` report and audit inherited trust boundaries:
```sh
gm run . --no-semantic --no-viz --lean-axioms axioms.json
gm trust --theorem MyProject.main_theorem --forbid-native-decide --json
```

`gm trust` classifies each theorem as `UNCONDITIONAL`, `CONDITIONAL`, or `TAINTED`. It exits non-zero on `sorryAx`, a non-allowlisted external axiom, a forbidden `Lean.ofReduceBool` / `Lean.trustCompiler`, or an unapproved binder hypothesis. Binder hypotheses (including multi-line signatures, active `variable` declarations, cross-file certificate types, and elaborated `forallE` binders) are detected automatically; `[CommRing R]` typeclass constraints and `(n : Nat)` parameters are ignored. Kernel labels are canonicalized and `--json` keeps stdout machine-readable with clean stderr. Pass `--allow-hypothesis <name>` (or configure `trust.allow_hypotheses` in `.graphenium/policy.json`) only after reviewing the structure/certificate parameter.

---

## Workflow 7: Manual Index Corrections & Overrides
Use this workflow when compiler parsing cannot automatically detect a critical runtime dependency (e.g., dynamic dependency injection or framework-based naming conventions).

### Core Premise
Do not let agents guess or invent relationships. If a dependency is verified by documentation or tests, write a manual, source-backed override to keep the index accurate.

### Recommended Tool Sequence
1.  `mutate_graph` with `action = "add_node"` - Add the logical concept, external API, or framework endpoint with explicit documentation-backed reasons.
2.  `mutate_graph` with `action = "add_edge"` - Add a verified relationship with `EXTRACTED` confidence.
3.  `mutate_graph` with `action = "recluster"` - Re-run community domain partitioning to update structural boundaries.
4.  `verify_changes` - Run compliance gates to verify that the manual override resolved the boundary gap.

---

## Strict Agentic Behavior Rules

An agent running in a Graphenium-governed workspace must adhere to the following rules:

*   **Handshake First:** Always call `get_graph_info` before performing any codebase edits to verify index freshness.
*   **Check Policies Pre-Flight:** Never implement code for a multi-file task without initializing a planning workspace and running `validate_plan`.
*   **Prefer AST Truth:** Build change paths on `EXTRACTED` (AST-proven) dependencies. Treat `INFERRED` paths as hypotheses, and stop to inspect code when a path hits `AMBIGUOUS` collisions.
*   **No Redundant Reads:** Use Graphenium's structural summaries to target files precisely, avoiding context-bloating folder sweeps.
*   **Inspect Scope Post-Edit:** Always run a post-edit audit with `verify_changes` before declaring a task complete.

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
1. Call get_graph_info to verify index freshness. If stale, run 'gm run . --no-semantic --no-viz' first.
2. Resolve your target symbol, identify direct callers, and trace downstream transitive dependencies.
3. If this is a multi-file task, initialize a planning workspace via create_planning_workspace, declare your intended classes/files, and run validate_plan. If rejected on topological entropy, call validate_plan with skip_policy_rules = true, re-plan through existing domain services, and re-evaluate. Do not edit source files if pre-flight fails.
4. Read only the implementation files recommended by Graphenium. Do not sweep directories blindly.

After editing files:
1. Re-run 'gm run . --no-semantic --no-viz' to update the local index.
2. Run verify_changes with target_symbols, then generate a structural PR audit.
```