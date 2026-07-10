# AI Setup Playbook

This playbook is written for AI assistants helping a user install and use Graphenium.

Goal:

> Install `gm`, build a local graph for the current project, connect it to the user's AI tool when possible, and verify that the agent can query the graph.

## Step 1: Detect environment

Run:

```sh
uname -s
uname -m
which rustc
which cargo
which gm
pwd
```

Report one line to the user:

```text
OS: <os>, architecture: <arch>, Rust: <present or missing>, Graphenium: <present or missing>, project: <path>
```

## Step 2: Install Rust if missing

Skip if `rustc` and `cargo` are already available.

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
rustc --version
cargo --version
```

If verification fails, stop and report the error.

## Step 3: Install Graphenium

Clone outside the project being analyzed.

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

Verify:

```sh
which gm
gm --version
```

Optional smaller build:

```sh
cargo install --locked --path . --no-default-features --features lang-python,lang-rust
```

Available language features:

- `lang-python`
- `lang-js`
- `lang-ts`
- `lang-rust`
- `lang-go`
- `lang-java`
- `lang-c`
- `lang-cpp`
- `lang-csharp`

## Step 4: Build the first graph

Return to the user's project directory.

```sh
gm init
gm run . --no-semantic --no-viz
```

Expected output includes:

```text
Found N files
AST: N nodes, N edges
Graph: N nodes, N edges
Communities: N
Report written to graphenium-out
```

The primary graph file is:

```text
graphenium-out/graph.json
```

If the graph has 0 nodes, check:

- current working directory
- supported languages
- `.grapheniumignore`
- generated or vendored code exclusion

## Step 5: Verify the CLI

```sh
gm doctor --graph graphenium-out/graph.json
gm query "test" --budget 100
```

The query may return no strong matches, but it should not crash.

## Step 6: Detect AI tool

Ask which AI coding tool the user uses only if it is not obvious.

| Tool | Config location or setup |
|---|---|
| Grok | `~/.grok/config.toml` |
| Claude Code | `claude mcp add graphenium --scope user -- gm serve` |
| Codex | `~/.codex/config.toml` |
| Cursor | `~/.cursor/mcp.json` |
| Other or unknown | Skip MCP and use CLI fallback |

## Step 7: Configure MCP

### Recommended: `graphenium-mcp` launcher

For Grok and other tools that start MCP from the project directory, use the launcher script. It prefers a local `target/release/gm` over the globally installed binary, auto-builds only when `graph.json` is missing, and starts the server with `--watch`.

```sh
# Install once (from a Graphenium checkout)
install -m 755 scripts/graphenium-mcp ~/.local/bin/graphenium-mcp
```

**Grok** (`~/.grok/config.toml`):

```toml
[mcp_servers.graphenium]
command = "/Users/<you>/.local/bin/graphenium-mcp"
args = []
enabled = true
```

Environment variables:

| Variable | Purpose |
|---|---|
| `GM_BIN` | Force a specific `gm` binary path |
| `GROK_PROJECT_ROOT` | Project root when not inferred from git |
| `GRAPHENIUM_AUTO_REBUILD=1` | Also rebuild when source or binary is newer than the graph (off by default; keeps large-repo session starts fast) |

### Direct `gm serve` (manual setup)

Use absolute paths when configuring MCP without the launcher.

```sh
GM_PATH=$(which gm)
GRAPH_PATH=$(pwd)/graphenium-out/graph.json
```

### Claude Code

```sh
claude mcp add graphenium --scope user -- "$GM_PATH" serve --graph "$GRAPH_PATH" --watch
```

### Codex

```toml
[mcp_servers.graphenium]
command = "$GM_PATH"
args = ["serve", "--graph", "$GRAPH_PATH", "--watch"]
```

### Cursor

```json
{
  "mcpServers": {
    "graphenium": {
      "command": "$GM_PATH",
      "args": ["serve", "--graph", "$GRAPH_PATH", "--watch"]
    }
  }
}
```

Preserve existing MCP server entries. Add or update only the `graphenium` entry.

After writing config, tell the user to fully quit and relaunch the AI tool.

## Step 8: Verify MCP from the agent

Ask the AI tool:

```text
Use Graphenium. Call graph_info first and tell me which graph is loaded.
```

Successful response should mention:

- project root
- schema version
- build timestamp
- extraction mode
- languages
- nodes and edges
- graph path

If the response includes **Graph may be stale**, rebuild and hot-swap without restarting MCP:

```sh
gm run . --no-semantic --no-viz
```

Then ask the agent to call `reload_graph`.

## Optional: semantic extraction

AST-only should be the default first run. Add semantic extraction only when the user wants richer inferred relationships.

```sh
# Anthropic default
gm run . --update --api-key sk-ant-...

# OpenAI
gm run . --update --provider openai --api-key sk-...

# DeepSeek
gm run . --update --provider deepseek --api-key sk-...
```

Semantic relationships must be treated as inferred unless provenance says otherwise.

## Optional: watch mode

```sh
gm serve --graph graphenium-out/graph.json --watch
gm watch . --impact
```

## Optional: Datalog

Datalog queries automatically include the standard library (v0.19.0+). Prefer stdlib predicates over hand-written recursion.

```sh
gm query "hubs" --datalog "?- is_hub(X)."
gm query "reachability" --datalog "?- calls_transitive('main', X)."
gm query "layers" --datalog "?- bypasses_layer(X, Y, Z)."
```

Stdlib predicates: `calls_transitive`, `imports_transitive`, `depends_transitive`, `same_community`, `is_hub`, `is_orphan`, `circular_dependency`, `bypasses_layer`.

## Optional: telemetry overlay

```sh
gm import-traces otel-traces.json
gm build-overlay --hot-paths
```

Telemetry is experimental and requires explicit trace data.

## Error recovery

| Symptom | Likely cause | Action |
|---|---|---|
| `cargo install` fails | Missing build tools | Install compiler toolchain such as Xcode command line tools or build-essential |
| `gm run` finds 0 files | Wrong directory or unsupported language | Check path and supported extensions |
| MCP config path does not exist | Tool not installed | Use CLI fallback |
| MCP connection fails | Tool not restarted | Fully quit and relaunch |
| `gm query` errors | Wrong graph path | Confirm `graphenium-out/graph.json` exists |
| `graph_info` reports stale graph | Source or binary newer than graph | `gm run . --no-semantic --no-viz`, then `reload_graph` |
| `run_datalog` returns no results | Old MCP server binary | Restart AI tool or set `GM_BIN` to a current `gm` |
| Too much noise | Vendor or generated code included | Tune `.grapheniumignore` |

## Final report to user

```text
Graphenium installed: <gm path>
Graph built: <graph path>
Graph size: <nodes> nodes, <edges> edges
MCP configured: yes or skipped
Next step: ask your agent to call graph_info before editing.
```
