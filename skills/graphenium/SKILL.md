---
name: graphenium
description: Use when analyzing codebase structure, verifying pre-flight policy compliance, tracing transitive dependency closures, or auditing post-edit scope creep.
---

# Graphenium Agentic Skill

Graphenium is a local, external architecture gate and pre-flight linter. It compiles your workspace into an AST-proven structural index and exposes boundary constraints over an MCP server interface.

---

## 1. Handshake Rule

**You must call `graph_info` as your first action during any session.**

Verify the following index properties before planning edits:
1.  **Project Root:** Confirm you are operating in the correct repository root.
2.  **Schema Version:** Must be `0.2.0`.
3.  **Source Languages:** Confirm Graphenium has compiled symbol configurations for your target languages.
4.  **Freshness Status:** If Graphenium warns that the **"Graph may be stale"**, physical source files have been modified since the last index compilation. You must run `gm run . --no-semantic --no-viz` locally, then invoke `reload_graph` to hot-swap the server state before proceeding.

---

## 2. MCP Boundary and Verification Tools

| Engineering Goal | Target MCP Tool | Behavioral Expectation |
|---|---|---|
| **Handshake & Freshness** | `graph_info` | Verify index integrity and check for stale warning flags. |
| **Codebase Orientation** | `architecture_summary` | Review top-level folder domains and cohesive module boundaries. |
| **Verify Design Pre-Flight** | `validate_plan` | Mathematically prove design safety against `.graphenium/policy.json` and dynamic delta gating. |
| **Topological Delta Gate** | `evaluate_delta_gate` | In-memory modularity delta check and surprise analysis on a proposed plan. |
| **Trace Transitive Paths** | `run_datalog` | Evaluate multi-hop dependency chains using compiled stdlib rules. |
| **Single Symbol Audit** | `analyse_symbol` | Retrieve AST-proven callers, dependencies, and identifier collisions. |
| **Check Direct Callers** | `get_neighbors` | View incoming callers and outgoing targets (set `extracted_only = true`). |
| **Identify Hotspots** | `god_nodes` | Identify highly coupled bottlenecks to target for refactoring risk. |
| **Disambiguate Collisions** | `get_node` | Disambiguate identically named classes or methods. |
| **Audit Post-Edit Compliance** | `agent_change_gate` | Run PR gates (optional `plan_id` executes pre-flight + resolution audits). |
| **PR Verification Plan** | `verification_plan` | Generate a risk-sorted test and review checklist for changed files. |
| **Index Hot-Swap** | `reload_graph` | Sync the server immediately after running a physical re-indexing pass. |

---

## 3. Datalog Queries (Transitive Path Proving)

Graphenium contains a pre-compiled Datalog standard library (`stdlib.dl`). **Never write manual recursive rules** to trace dependencies; instead, invoke Graphenium's pre-loaded standard library predicates via `run_datalog`:

*   **Audit Layer Bypassing:**
    `?- bypasses_layer('auth_controller', 'auth_service', 'db_helper').`
*   **Identify Circular Import Cycles:**
    `?- circular_dependency(X, Y).`
*   **Trace Multi-Hop Call Closures:**
    `?- calls_transitive('api_router', X).`
*   **Trace Generic Transitive Dependencies:**
    `?- depends_transitive('auth_helper', X).`

### Available EDB Relations:
*   `node(Id, Label, Type, File, Community)`
*   `calls(Source, Target, Confidence)`
*   `imports(Source, Target, Confidence)`
*   `inherits(Source, Target, Confidence)`
*   `implements(Source, Target, Confidence)`
*   `degree(NodeId, Count)`
*   `hub(NodeId)`

---

## 4. Trust-Aware Design Policies

You must explicitly separate AST-proven facts from semantic guesswork when planning edits:

*   `EXTRACTED` **(AST-Proven):** Compiler-backed facts (imports, class boundaries, method calls). Use as the **planning backbone**.
*   `INFERRED` **(Heuristics):** Semantic similarity or naming guesses. Treat as a **hypothesis**—you are strictly blocked from editing these targets until you read their source files directly.
*   `AMBIGUOUS` **(Collisions):** Symbol name collisions. **Risk gated**—stop and run `get_node` to resolve the collision before proceeding.

---

## 5. Standard Operating Workflows

### The "Design-then-Verify" Workspace Loop
For all multi-file refactoring tasks, you must execute Graphenium's structural contract:
1.  **Initialize Planning Workspace:** Call `create_planning_workspace` to establish a virtual design.
2.  **Declare Design Intent:** Call `add_planned_symbol` for every class, method, or dependency you intend to write. This automatically evaluates your design against `.graphenium/policy.json`.
3.  **Pre-Flight Solve:** Call `validate_plan` to mathematically verify design compliance before writing code.
4.  **Write Code:** Implement edits inside your local editor.
5.  **Audit Scope Creep:** Re-compile the index and run `agent_change_gate` (passing your `plan_id`). If Graphenium detects unplanned file modifications or unapproved dependencies, abort your PR and resolve the scope creep.

### Write-Back Protocol (Manual Overrides)
Only inject manual index overrides after direct, human-verified file-level code reviews:
*   Use `add_node` to define verified architectural concepts or external system boundaries.
*   Use `add_edge` with `EXTRACTED` confidence only when a dependency is proven by source code.
*   Run `recluster` after manual edits to update folder domain boundaries.

---

## 6. CLI Fallback Syntax (Local Terminal)

When background MCP tools are unavailable, run local CLI commands directly:

```sh
# Execute combined structural search
gm query "auth service" --mode hybrid --budget 2000

# Trace AST-proven dependencies only
gm query "validate_token" --safe --budget 1500

# Execute Datalog rules
gm query "hubs" --datalog "?- is_hub(X)."
```

---

## 7. Standard Handshake Response Pattern

After analyzing Graphenium's pre-flight and diagnostic gates, format your planning response exactly as follows:

```text
Structural Index Loaded: [loaded index path]
Handshake Status: [fresh / stale - compile needed]
Target Symbol Resolved: [canonical symbol ID]
AST-Proven Boundaries (Extracted): [count of compiler facts]
Semantic Hypotheses (Inferred): [count of semantic guesses]
Identifier Collisions (Ambiguous): [list of collisions needing manual check]
Pre-Flight Policy Status: [PASS / FAILED - list violations]
Must-Read Implementation Files: [list of source files to read before editing]
Post-Edit Verification Plan: [list of covering tests to run after editing]
```

---

## 8. Resolving Topological Delta Failures

If you run `evaluate_delta_gate` (or `validate_plan`) and receive an entropy rejection:

1. **Understand the Warning:** Graphenium did not fail because of a regex; it failed because your proposed dependencies mathematically degrade the modularity of the system.
2. **Review High-Surprise Edges:** Locate which planned symbols triggered `cross-community` or `peripheral_to_hub` spikes. These represent architectural shortcuts.
3. **Re-Plan decoupling:**
   - Instead of connecting a new view directly to a database, modify your planning workspace to route the request through the existing intermediate domain services.
   - If the connection is functionally required, expose a generic Interface/Trait within the targets' community, rather than coupling the concrete classes directly.
4. **Re-Evaluate:** Call `evaluate_delta_gate` again. Once modularity stabilizes (ΔQ ≥ -0.02) and surprise scores are below threshold, proceed with implementation.