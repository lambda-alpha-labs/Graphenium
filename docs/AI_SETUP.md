# AI Assistant Setup Playbook

This playbook is written for AI assistants helping a user install, configure, and verify Graphenium.

Goal:
> Install `gm`, generate the first structural index for the current project, configure Graphenium's pre-flight architecture gates, connect Graphenium to the user's agentic workspace via MCP, and verify that the assistant can run pre-flight policy checks.

---

## Step 1: Detect Environment

Execute the following commands to assess the workspace environment:

```sh
uname -s
uname -m
which rustc
which cargo
which gm
pwd
```

Report exactly one line back to the user:

```text
OS: <os>, architecture: <arch>, Rust: <present or missing>, Graphenium: <present or missing>, project: <path>
```

---

## Step 2: Install Rust (If Missing)

Skip this step if both `rustc` and `cargo` are already available.

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
rustc --version
cargo --version
```

*If compilation toolchain verification fails, halt and report the environmental error.*

---

## Step 3: Install Graphenium

Clone Graphenium outside of the target repository being governed.

```sh
git clone https://github.com/lambda-alpha-labs/Graphenium "$HOME/.graphenium"
cd "$HOME/.graphenium"
cargo install --locked --path .
```

Windows PowerShell:
```powershell
git clone https://github.com/lambda-alpha-labs/Graphenium "$env:USERPROFILE\.graphenium"
cd "$env:USERPROFILE\.graphenium"
cargo install --locked --path .
```

Verify build availability:
```sh
which gm
gm --version
```

### Resource-Constrained Build Option
To build with a smaller binary footprint, you can compile with support restricted to target languages:
```sh
cargo install --locked --path . --no-default-features --features lang-python,lang-rust
```

Available language compilation flags:
*   `lang-python`
*   `lang-js`
*   `lang-ts`
*   `lang-rust`
*   `lang-go`
*   `lang-java`
*   `lang-c`
*   `lang-cpp`
*   `lang-csharp`

---

## Step 4: Generate the Structural Index

Navigate back to the root of the user's project directory. Initialize Graphenium's configuration and build the baseline structural index using fast, local AST parsing:

```sh
gm init
gm run . --no-semantic --no-viz
```

Expected output log pattern:
```text
Found N files
AST: N nodes, N edges
Graph: N nodes, N edges
Communities: N
Report written to graphenium-out
```

The compiled codebase structure is written to Graphenium's local cache:
```text
graphenium-out/graph.json
```

If 0 nodes or edges are reported, troubleshoot:
1.  Verify the current working directory is the correct project root.
2.  Confirm the files match Graphenium's supported extensions.
3.  Check if `.grapheniumignore` is over-filtering target sources.
4.  Ensure you are not analyzing exclusively vendored or generated directories.

---

## Step 5: Verify Linter and Index Health

Confirm that the local structural index was parsed correctly and diagnostics are clean:

```sh
gm doctor --graph graphenium-out/graph.json
gm query "test" --budget 100
```

*The query may return zero direct matches depending on the project, but it must complete without throwing errors or crashes.*

---

## Step 6: Identify Agent Environment

Identify which agentic coding tool the user is running to determine the correct MCP configuration:

| Agent / IDE | Configuration Target |
|---|---|
| Grok | `~/.grok/config.toml` |
| Claude Code | `claude mcp add graphenium --scope user -- gm serve` |
| Codex | `~/.codex/config.toml` |
| Cursor | `~/.cursor/mcp.json` |
| Other CLI Agents | Run local CLI commands directly |

---

## Step 7: Configure the MCP Server

### Recommended Approach: Use Graphenium's Launcher
For Grok and other tools that initialize the MCP process from within the project directory, use Graphenium's pre-flight launcher script. It automatically targets a local workspace build if present, skips redundant index generations when sources are unchanged, and activates real-time watch modes.

```sh
# Install the launcher once (run from Graphenium clone)
install -m 755 scripts/graphenium-mcp ~/.local/bin/graphenium-mcp
```

**Grok Config** (`~/.grok/config.toml`):
```toml
[mcp_servers.graphenium]
command = "/Users/<you>/.local/bin/graphenium-mcp"
args = []
enabled = true
```

#### Configurable Environment Variables:
*   `GM_BIN`: Overrides the auto-resolved Graphenium binary path.
*   `GROK_PROJECT_ROOT`: Forces a project root location when git inference is unavailable.
*   `GRAPHENIUM_AUTO_REBUILD=1`: Rebuilds the codebase index automatically on session startup if source files are newer than the cached index.

---

### Manual Path Configuration (Absolute Path Fallback)
If configuring MCP without the launcher, map the absolute paths directly:

```sh
GM_PATH=$(which gm)
INDEX_PATH=$(pwd)/graphenium-out/graph.json
```

**Claude Code:**
```sh
claude mcp add graphenium --scope user -- "$GM_PATH" serve --graph "$INDEX_PATH" --watch
```

**Codex:**
```toml
[mcp_servers.graphenium]
command = "$GM_PATH"
args = ["serve", "--graph", "$INDEX_PATH", "--watch"]
```

**Cursor:**
```json
{
  "mcpServers": {
    "graphenium": {
      "command": "$GM_PATH",
      "args": ["serve", "--graph", "$INDEX_PATH", "--watch"]
    }
  }
}
```

*Ensure existing server entries in the target tool config are preserved; only add or update the `graphenium` block.*

---

## Step 8: Verify Architectural Integrity from the Agent

Once Graphenium is configured, ask your agentic interface to run a pre-flight handshake:

```text
Use Graphenium. Call graph_info first and tell me which codebase index is loaded.
```

The agent's handshake response must successfully report:
*   Project root path.
*   Index schema version (`0.2.0`).
*   Build timestamp.
*   Languages detected.
*   Symbol and dependency counts.
*   The path of the loaded `graph.json` index.

### Stale Index Warning Mitigation
If `graph_info` warns that **"Graph may be stale"**, it means physical source files have changed since Graphenium last compiled its structural index. Rebuild and hot-swap the index locally without restarting the background MCP server process:

```sh
gm run . --no-semantic --no-viz
```
Then instruct the agent: `reload_graph` (this updates Graphenium's in-memory index immediately).

---

## Advanced Playbook Capabilities

### 1. Pre-Flight Architecture Gating (First-Order Logic)
Graphenium allows you to block invalid code design pre-flight. Define structural boundaries in `.graphenium/policy.json` at the root of the repository (e.g., controllers can never call database entities directly).

To test if a proposed change violates policy before writing any code:
1.  Call `create_planning_workspace` to initialize a draft workspace.
2.  Call `add_planned_symbol` to register the proposed class, method, or dependency. This automatically runs Graphenium's embedded Datalog solver.
3.  If Graphenium returns `PRE_FLIGHT_VIOLATION`, block the agent from editing the files, and inspect the violations.

### 2. Datalog Transitive Closure Queries
Graphenium's Datalog engine automatically includes a standard library of structural predicates (`stdlib.dl`). Instruct the agent to run declarative transitive queries rather than writing recursive prompts:

```sh
# Detect transitive dependency paths
gm query "reach" --datalog "?- depends_transitive('api_controller', X)."

# Identify circular module dependencies
gm query "cycles" --datalog "?- circular_dependency(X, Y)."

# Check if an agent's design bypasses a layer
gm query "bypass" --datalog "?- bypasses_layer('auth_controller', 'auth_service', 'db_helper')."
```

---

## Error Recovery Guide

| Problem | Root Cause | Resolution |
|---|---|---|
| `cargo install` compilation fails | C compiler toolchain missing | Install Xcode Command Line Tools (`xcode-select --install`) on macOS or `build-essential` on Linux. |
| `gm run` reports 0 files | Unsupported language or directory mismatch | Verify Graphenium is run from the target project root, and check Graphenium's supported extensions. |
| MCP tool initialization fails | Target tool needs restart | Fully quit and relaunch the agent's parent IDE or terminal session. |
| `graph_info` reports stale index | Code modifications occurred post-build | Execute `gm run . --no-semantic --no-viz` and instruct the agent to run `reload_graph`. |
| Datalog queries return no results | Old server binary is running | Verify that `GM_BIN` points to the updated compilation of `gm` and restart the agent workspace. |
| Too much architectural noise | Build artifacts or library vendors included | Restructure `.grapheniumignore` to exclude build, target, or third-party folders. |

---

## Final Playbook Report

Once the setup is complete, provide the user with a concise architectural summary:

```text
Graphenium Pre-Flight Gate Active: <gm path>
Base Codebase Index Generated: <index path>
Current Index Scale: <nodes> symbols, <edges> boundaries
Pre-Flight Policies Configured: <yes / no / skipped>
Workspace Guardrails Active: Yes (agent must query codebase structure before editing)
```
