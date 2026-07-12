# Getting Started: Establishing Workspace Containment

This guide walks you from a clean repository to an active, write-gated developer workspace governed by Graphenium.

---

## 1. Prerequisites

Before installing Graphenium, ensure your environment meets the following requirements:
*   **Compilation Toolchain:** Rust 1.81 or later (`rustc` and `cargo` installed).
*   **Repository Languages:** One or more supported languages:
    *   *Rust, Python, Go, JavaScript, TypeScript, Java, C, C++, and C#.*
*   **AI Agent Workspace:** An MCP-compatible coding agent (Claude Code, Cursor, Grok, or Codex).

---

## 2. Installation

To compile Graphenium locally, clone outside your target repository and execute a locked build:

```sh
# Clone Graphenium
git clone https://github.com/lambda-alpha-labs/Graphenium "$HOME/.graphenium"
cd "$HOME/.graphenium"

# Build and install the CLI binary (gm)
cargo install --locked --path .
```

*For resource-constrained environments, refer to [`docs/AI_SETUP.md`](AI_SETUP.md) for custom language-compilation flags.*

---

## 3. Initialize Your Workspace

Navigate to the root directory of the repository you want to govern, and initialize Graphenium's configuration:

```sh
cd /path/to/your-project
gm init
```

This generates a default `.grapheniumignore` file at your repository root to exclude compiled artifacts, dependencies, and generated code:

```gitignore
# Exclude build and package artifacts
target/
node_modules/
bin/
obj/
graphenium-out/
```

---

## 4. Generate the Baseline Structural Index

Compile your first codebase index. We recommend starting with a local AST-only run. This runs 100% offline, requires no API keys, and establishes your baseline compiler-proven truth:

```sh
gm run . --no-semantic --no-viz
```

Expected output logs:
```text
Found N files
AST: N nodes, N edges
Graph: N nodes, N edges
Communities: N
Report written to graphenium-out
```

The compiled structural index is written locally to:
```text
graphenium-out/graph.json
```

---

## 5. Verify Index Integrity

Execute Graphenium's diagnostic checks to ensure the AST and Stack Graphs extraction resolved your codebase's physical import boundaries:

```sh
# Run general environment and index diagnostic
gm doctor --graph graphenium-out/graph.json

# Audit import resolution ratios
gm doctor --graph graphenium-out/graph.json --resolution
```

A healthy index should report:
*   A non-zero count of compiled symbols (nodes).
*   An import resolution ratio over 80%.
*   Zero corpus warnings. If warnings are present, adjust your `.grapheniumignore` file.

---

## 6. Execute Your First Structural Queries

Verify that Graphenium can run local path traces and Datalog logic solving on your index:

```sh
# Trace direct and transitive calls for a target symbol
gm query "validate_token" --safe --budget 1500

# Probe for circular module dependencies using Datalog
gm query "cycles" --datalog "?- circular_dependency(X, Y)."

# Prove if a component bypasses an intermediary layer
gm query "layer-check" --datalog "?- bypasses_layer('auth_ctrl', 'auth_service', 'db_helper')."
```

---

## 7. Connect Graphenium to Your AI Agent (MCP)

To intercept and govern agentic actions, start Graphenium's background MCP server. It listens on standard I/O and hot-reloads its index automatically:

```sh
gm serve --graph graphenium-out/graph.json --watch
```

*For Grok and other project-local tools, we recommend using Graphenium's pre-flight launcher script (`scripts/graphenium-mcp`). Refer to [`docs/AI_SETUP.md`](AI_SETUP.md) for tool-specific configuration blocks.*

### The Verification Handshake:
Once MCP is active, instruct your agent to run an initial hand-off check:
```text
Use Graphenium. Call graph_info first and tell me which codebase index is loaded.
```

If Graphenium warns that the **"Graph may be stale"**, the compiled index is older than your physical files. Recompile the index (`gm run . --no-semantic --no-viz`) and instruct the agent to run `reload_graph`.

---

## 8. Establish Write-Time Policy Gating

Protect your repository from architectural drift by defining structural boundaries. Write a `.graphenium/policy.json` at your repository root to forbid direct database imports from your API controllers:

```json
{
  "rules": [
    {
      "type": "forbidden_dependency",
      "from_pattern": "src/controllers/**",
      "to_pattern": "src/db/**",
      "reason": "Controllers must use services, not access DB directly"
    }
  ]
}
```

Now, instruct your agent to execute a **Design-then-Verify** loop:
1.  **Initialize Planning Workspace:** The agent calls `create_planning_workspace` to establish a virtual design.
2.  **Declare Intent:** The agent registers its planned class and import additions (`add_planned_symbol`). Graphenium's Datalog engine automatically evaluates these additions.
3.  **Validate Pre-Flight:** If the agent tries to import a database module directly into a controller, Graphenium returns `PRE_FLIGHT_VIOLATION` and blocks the plan.
4.  **Audit Scope Creep:** After implementing the code, run `gm check --plan <id> --strict` in CI. If the agent modified files outside the declared plan, the build fails.

---

## 9. Troubleshooting and Gating Recovery

| Problem | Root Cause | Action |
|---|---|---|
| **0 symbols compiled** | Unsupported files, wrong directory, or ignore-rule mismatch. | Run `pwd` to verify the root path. Check Graphenium's language support, and audit `.grapheniumignore`. |
| **MCP server connection fails** | IDE or CLI tool did not reload config. | Fully quit and relaunch your IDE or terminal workspace to force MCP initialization. |
| **High ambiguity counts** | Identically named classes across folders. | Use Graphenium's fully qualified labels (`qualified_label`) to target symbols uniquely during searches. |
| **Index-build is slow** | Third-party packages or build folders are being scanned. | Exclude dependencies (e.g., `target/`, `node_modules/`, `.git/`) inside `.grapheniumignore`. |

---

## 10. Success Criteria

You have successfully established workspace containment when:
1.  `gm doctor` reports a clean codebase index with high resolution ratios.
2.  Your AI coding assistant calls `graph_info` successfully during handshake runs.
3.  The assistant explicitly separates AST-proven dependencies (`EXTRACTED`) from hypotheses (`INFERRED`).
4.  Any agent design plan violating your `.graphenium/policy.json` is successfully blocked pre-flight.
