# MCP Tool Catalog and Containment Protocol

Graphenium exposes a Model Context Protocol (MCP) interface, turning its background server into an active **pre-flight linter and structural containment gate** for AI agents. Rather than allowing agents to grep blindly or read raw codebase files, Graphenium provides precise, compiler-proven metadata to govern their write-time designs.

---

## 1. Operating Rules of Thumb for Agents

Before modifying code, agents must follow this strict tool sequence:

```text
Handshake (graph_info) ──► Pathfinding (analyse_symbol / safest_path)
                             │
                             ▼
Pre-Flight Spec        ──► Declare design (create_planning_workspace)
                             │
                             ▼
Policy Verification    ──► Prove design safety (validate_plan / evaluate_delta_gate)
                             │
                             ▼ (Passes)
Implementation         ──► Edit files locally
                             │
                             ▼
Compliance Audit       ──► Verify no scope creep occurred (agent_change_gate)
```

---

## 2. Read and Inspection Tools

These tools allow assistants to retrieve local structural metadata without executing remote API calls or bloating context windows with raw source files.

### `graph_info`
*   **Purpose:** Handshakes and returns baseline index metadata (project root, schema version, build timestamp, compilation modes, source languages, symbol counts, index file location, and active policy gates).
*   **Policy Gates Banner:** Reports which containment layers are active — explicit `.graphenium/policy.json` rules (if any) plus **Dynamic Delta Gating** (zero-config modularity protection). When no policy file is configured, delta gating still runs as the default invariant gate.
*   **Staleness Guard:** Automatically warns the agent if physical source files or Graphenium's binary are newer than the cached index. If a stale warning is returned, the agent is instructed to run `gm run` and trigger `reload_graph`.
*   **When to Use:** At the start of every chat session or after any major branch checkout.

### `graph_stats`
*   **Purpose:** Returns total symbol counts, compiler-proven (`EXTRACTED`) vs. semantic (`INFERRED`) ratios, and boundary counts.
*   **When to Use:** To evaluate the general health, scale, and resolution status of Graphenium's index.

### `query_graph`
*   **Purpose:** Executes structural queries radiating outward from matched keyword search nodes.
*   **Parameters:** `keywords`, `depth` (hop limit), `budget` (token threshold), `path_prefix` (directory scoping), `exclude_path` (noisy folder exclusion), `node_types` (file, class, method, function), `include_tests` (bool), and `ast_only_tuning` (bool).
*   **When to Use:** To locate code symbols related to a specific feature, folder, or API surface.

### `get_node`
*   **Purpose:** Resolves a single symbol precisely by its canonical ID. Returns label, file type, source file, line boundaries, and degree (coupling score).
*   **When to Use:** To disambiguate identifier collisions or identify where a class or function is physically implemented.

### `get_neighbors`
*   **Purpose:** Returns direct, AST-proven connections (incoming callers and outgoing dependencies) for a target symbol. The `extracted_only` parameter isolates compiler-proven (`EXTRACTED`) edges.
*   **When to Use:** Before proposing changes to a public API, to identify all dependents.

### `summarize_file`
*   **Purpose:** Returns all extracted symbols from a single file.
*   **When to Use:** When you need an inventory of what a file contains without reading the entire raw source.

### `references_to`
*   **Purpose:** Locates all direct imports, calls, and inheritances pointing toward a target symbol.
*   **When to Use:** To audit who currently depends on a module before modifying its public surface.

### `unresolved_references`
*   **Purpose:** Lists all imports whose targets are missing from the AST index.
*   **When to Use:** To detect missing dependencies or incorrectly configured project paths.

---

## 3. Path and Transitive Closure Tools

These tools link structural pathfinding with first-order logic proofs, enabling agents to mathematically verify dependency chains.

### `query_transitive`
*   **Purpose:** Explores a target symbol's full multi-hop dependency closure.
*   **Direction Control:** Set `direction` to `"forward"` (outgoing calls), `"reverse"` (incoming callers), or `"both"`.
*   **When to Use:** To map the full blast radius of a symbol before modifying it.

### `shortest_path`
*   **Purpose:** Finds the minimal or highest-provenance structural route between two symbols. Semantic mode prefers `calls` and `uses` relationships; strict mode uses exact hop count.
*   **When to Use:** To verify that two classes are genuinely connected through deterministic compiler-proven paths.

### `safest_path`
*   **Purpose:** Finds the path with the highest aggregate confidence profile (prioritizing `EXTRACTED` edges over `INFERRED` or `AMBIGUOUS`).
*   **When to Use:** When identifying a maximally safe execution path through your architecture.

### `module_dependencies`
*   **Purpose:** Cross-references all dependency edges between two module paths.
*   **When to Use:** To map the full dependency contract between two system domains or folders.

### `run_datalog`
*   **Purpose:** Executes a declarative, first-order logic program over Graphenium's compiled EDB.
*   **Standard Library:** Automatically includes `stdlib.dl` predicates (`calls_transitive`, `depends_transitive`, `circular_dependency`, `bypasses_layer`, etc.) without requiring manual rule definitions.
*   **When to Use:** To mathematically prove transitive boundary violations, circular dependencies, or identify orphaned nodes.

---

## 4. Verification and Gating Tools

These are Graphenium's containment and compliance enforcement tools.

### `analyse_symbol`
*   **Purpose:** A composite query returning callers, dependencies, transitive closures, domain placement, and structural risks for a single target symbol.
*   **When to Use:** As a first orientation step before modifying a symbol.

### `architecture_summary`
*   **Purpose:** Returns the top-level structural layout of the repository, including domain clusters and cross-domain connections.
*   **When to Use:** When the agent needs a global structural orientation of the system.

### `god_nodes`
*   **Purpose:** Returns the most highly coupled symbols (hubs) with the highest degree scores.
*   **When to Use:** To identify high-risk refactoring targets.

### `explain_change`
*   **Purpose:** Pre-edit orientation summary including hierarchy, community context, entry points, must-read files, and test scaffolding for a symbol.
*   **When to Use:** Before modifying any public interface.

### `verification_plan`
*   **Purpose:** Generates a risk-sorted checklist of verification steps for modified files (e.g., must-read files, dependent interfaces, and covering test files).
*   **When to Use:** Prior to committing code or opening a PR to determine what local tests must be run.

### `blast_radius`
*   **Purpose:** Measures the downstream transitive impact of modifying a symbol. It identifies affected files, compromised module domains, and the trust confidence levels of those connections.
*   **When to Use:** Before writing any modifications to estimate the risk of the change.

### `agent_change_gate`
*   **Purpose:** Executes an automated structural verification audit against current index properties.
*   **Optional Parameter `plan_id`:** When provided, Graphenium integrates a pre-flight architectural policy check alongside the index-wide resolution gates.
*   **When to Use:** In CI/CD pipelines, git pre-commit hooks, or pre-review agent workflows.

---

## 5. Planning Workspace Tools

These tools support Graphenium's write-time containment loop, allowing agents to declare their designs and verify compliance before implementing them.

### `create_planning_workspace`
*   **Purpose:** Initializes a virtual, in-memory workspace and returns a unique `plan_id`.
*   **When to Use:** When starting any multi-file task or refactoring ticket.

### `add_planned_symbol`
*   **Purpose:** Registers an intended symbol declaration or dependency edge in the virtual workspace.
*   **Automated Gating:** If a `.graphenium/policy.json` file is present, this tool automatically runs a pre-flight policy solver on the proposed design. If a violation is detected (e.g., a forbidden dependency), Graphenium returns `PRE_FLIGHT_VIOLATION` and blocks the registration.
*   **When to Use:** To declare the architectural design the agent intends to write.

### `validate_plan`
*   **Purpose:** Evaluates the virtual plan before code is implemented. Runs explicit `.graphenium/policy.json` rules when configured (forbidden dependencies, strict layering bypasses, banned symbols), then applies **Dynamic Delta Gating** as a zero-config fallback — even when no policy file exists.
*   **When to Use:** After the agent completes its virtual plan and before it edits any source files.

### `get_plan_details`
*   **Purpose:** Retrieves the virtual design spec, highlights currently implemented symbols, and flags missing declarations or scope creep.
*   **When to Use:** Prior to generating a PR or requesting human review.

---

## 6. Topological Delta Tools

These tools evaluate proposed plans against Graphenium's mathematical modularity invariants. They operate entirely in memory on the loaded index — no physical files are modified.

### `evaluate_delta_gate`

Performs an in-memory **Topological Delta Gate** on a planning workspace.

**Input Schema:**

```json
{
  "name": "evaluate_delta_gate",
  "inputSchema": {
    "type": "object",
    "properties": {
      "plan_id": {
        "type": "string",
        "description": "The unique identifier of the target planning workspace."
      },
      "modularity_tolerance": {
        "type": "number",
        "description": "Optional maximum allowed modularity drop. Defaults to -0.02."
      },
      "surprise_threshold": {
        "type": "number",
        "description": "Optional surprise threshold to flag structural violations. Defaults to 5.0."
      }
    },
    "required": ["plan_id"]
  }
}
```

**Parameters:**
*   `plan_id` (required) — Planning workspace identifier.
*   `modularity_tolerance` (optional, default: `-0.02`) — Maximum allowed modularity decay (ΔQ floor).
*   `surprise_threshold` (optional, default: `5.0`) — Minimum surprise score to flag a planned edge.

**Pass Criteria:** ΔQ ≥ `modularity_tolerance` and no planned edges exceed `surprise_threshold`.

**Response Format (Markdown):**

```markdown
### Topological Delta Report for Plan: `refactor-session-handling`

- **Status:** ❌ FAILED
- **Modularity Delta (ΔQ):** -0.0341 (Baseline: 0.3841 → Virtual: 0.3500)

#### High-Surprise Edges Proposed:
- `plan:UserView` ──► `src/db/helper` (Confidence: 6.5)
  *Reason:* cross-community, peripheral→hub

#### Structural Drift Warnings:
- **Cross-boundary edges**: cross-boundary edges increased: 42 → 45
```

**How assistants should interpret responses:**
*   **`✅ PASSED`** — The plan preserves modularity and introduces no high-surprise planned edges. Proceed to implementation.
*   **`❌ FAILED` with negative ΔQ** — The plan degrades community cohesion. Re-route dependencies through existing domain services before editing files.
*   **`❌ FAILED` with `High-Surprise Edges`** — Inspect the listed `plan_surprise_edges`. Each entry names the source, target, score, and contributing factors (`cross-community`, `peripheral→hub`, etc.). Refactor the virtual plan to decouple across community boundaries.
*   **Drift warnings** — Informational; highlight community boundary shifts triggered by the plan. Review alongside surprise edges.

**When to Use:** To iteratively refine a design before implementation, or when `validate_plan` reports a topological entropy rejection.

---

## 7. Write and Index Operations

These tools should be used sparingly and only after direct, human-verified source inspection.

### `add_node`
*   **Purpose:** Injects a logical concept, design decision rationale, or external API boundary into Graphenium's index.

### `add_edge`
*   **Purpose:** Injects a verified relationship into Graphenium's index. 
*   **Safety Rule:** Only write with `Confidence::Extracted` if the connection has been proven by direct file-level code review.

### `remove_edge`
*   **Purpose:** Removes a false positive, obsolete, or incorrect dependency from the index.

### `recluster`
*   **Purpose:** Re-calculates community cohesive domains after manual overrides have been injected.

### `reload_graph`
*   **Purpose:** Hot-swaps Graphenium's in-memory index from a local file without requiring an MCP server restart.
*   **When to Use:** After running `gm run` locally to sync the background server with physical file edits.