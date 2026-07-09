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
| Claude Code | `claude mcp add graphenium --scope user -- gm serve` |
| Codex | `~/.codex/config.toml` |
| Cursor | `~/.cursor/mcp.json` |
| Other or unknown | Skip MCP and use CLI fallback |

## Step 7: Configure MCP

Use absolute paths.

```sh
GM_PATH=$(which gm)
GRAPH_PATH=$(pwd)/graphenium-out/graph.json
```

### Claude Code

```sh
claude mcp add graphenium --scope user -- "$GM_PATH" serve --graph "$GRAPH_PATH"
```

### Codex

```toml
[mcp_servers.graphenium]
command = "$GM_PATH"
args = ["serve", "--graph", "$GRAPH_PATH"]
```

### Cursor

```json
{
  "mcpServers": {
    "graphenium": {
      "command": "$GM_PATH",
      "args": ["serve", "--graph", "$GRAPH_PATH"]
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

```sh
gm query --datalog "?- node(X, 'AuthSvc', _, _, _)."
gm query --datalog "?- calls(X, Y, _)."
```

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
| Too much noise | Vendor or generated code included | Tune `.grapheniumignore` |

## Final report to user

```text
Graphenium installed: <gm path>
Graph built: <graph path>
Graph size: <nodes> nodes, <edges> edges
MCP configured: yes or skipped
Next step: ask your agent to call graph_info before editing.
```
