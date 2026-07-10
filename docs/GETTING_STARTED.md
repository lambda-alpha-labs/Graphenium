# Getting Started

This guide takes you from an empty workspace to a Graphenium-powered AI coding workflow.

## Prerequisites

- Rust 1.81 or later
- A repository written in one or more supported languages
- Optional: an AI coding tool that supports MCP

Supported languages:

| Language family | Support |
|---|---|
| Rust | AST extraction and graph relationships |
| Python | AST extraction and graph relationships |
| Go | AST extraction and graph relationships |
| JavaScript | AST extraction and graph relationships |
| TypeScript | AST extraction and graph relationships |
| Java | AST extraction and graph relationships |
| C | AST extraction and graph relationships |
| C++ | AST extraction and graph relationships |
| C# | AST extraction plus `.sln` and `.csproj` boundary parsing |

## Install

From a local checkout:

```sh
cargo install --locked --path .
```

Minimal language build example:

```sh
cargo install --locked --path . --no-default-features --features lang-python,lang-rust
```

## Initialize a workspace

Run this at the repository root:

```sh
gm init
```

This creates `.grapheniumignore` with sensible defaults for build artifacts, dependencies, generated files, and vendor directories.

## Build the graph

Start with the fast AST-only path. It requires no API key.

```sh
gm run . --no-semantic --no-viz
```

Expected output includes file counts, node counts, edge counts, communities, and output paths.

The main output is:

```text
graphenium-out/graph.json
```

## Inspect graph health

```sh
gm doctor --graph graphenium-out/graph.json
```

For trust quality details:

```sh
gm doctor --graph graphenium-out/graph.json --resolution
```

Check:

- node count is not zero
- edge count is plausible
- languages match the repository
- ambiguous and unresolved counts are understandable
- sensitive files and generated files are excluded as expected

## Run your first query

```sh
gm query "authentication flow" --budget 2000
```

Try these patterns:

```sh
# Orient on the repository.
gm query "architecture summary" --budget 2000

# Explore a feature.
gm query "billing service" --mode hybrid --budget 3000

# Follow a symbol neighborhood.
gm query "validate_token" --depth 2 --safe --budget 1500

# Declarative query.
gm query --datalog "?- node(X, _, _, _, _)."
```

## Start MCP for an AI agent

```sh
gm serve --graph graphenium-out/graph.json --watch
```

Or use the launcher script (recommended for Grok): install `scripts/graphenium-mcp` to `~/.local/bin/` and point your MCP config at it. The launcher auto-builds only when `graph.json` is missing; stale graphs are served immediately with a warning in `graph_info`.

Then configure your AI tool to connect to the server. See `docs/AI_SETUP.md` for per-tool config, including Grok.

### Claude Code

```sh
claude mcp add graphenium --scope user -- gm serve --graph /path/to/graphenium-out/graph.json --watch
```

### Grok

```toml
[mcp_servers.graphenium]
command = "/Users/<you>/.local/bin/graphenium-mcp"
args = []
```

### Codex

Add this to `~/.codex/config.toml`:

```toml
[mcp_servers.graphenium]
command = "gm"
args = ["serve", "--graph", "/path/to/graphenium-out/graph.json", "--watch"]
```

### Cursor

Add this to `~/.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "graphenium": {
      "command": "gm",
      "args": ["serve", "--graph", "/path/to/graphenium-out/graph.json", "--watch"]
    }
  }
}
```

After editing config, fully restart the AI tool so it reloads MCP servers.

## First agent prompt

Use this prompt after MCP is connected:

```text
Use Graphenium before editing. Call graph_info first to confirm which graph is loaded. Then summarize the relevant architecture, identify source-backed relationships, list ambiguous relationships, and recommend the first files to read before making changes.
```

Expected behavior:

- The agent confirms the loaded graph.
- The agent uses graph tools before reading many files.
- The agent distinguishes `EXTRACTED`, `INFERRED`, and `AMBIGUOUS` relationships.
- The agent reads source files before finalizing a patch.

## Create a planning workspace

Use this for multi-file changes.

```sh
gm plan create --name "refactor-auth"
gm plan add-symbol --plan "refactor-auth" --symbol "new_auth_service" --kind function
# Write the code.
gm run . --update --no-semantic --no-viz
gm plan get --plan "refactor-auth"
```

Through MCP, use:

- `create_planning_workspace`
- `add_planned_symbol`
- `get_plan_details`
- `verification_plan`
- `blast_radius`
- `agent_change_gate`

## Live development

Keep the graph fresh during edits with watch mode or MCP hot-reload.

**CLI watch** (rebuilds on file changes):

```sh
gm watch . --impact
```

A typical two-terminal flow:

```text
Terminal 1: gm watch . --impact
Terminal 2: edit code, run tests, ask the agent to inspect impact
```

**MCP hot-reload** (when using `gm serve --watch` or `graphenium-mcp`):

1. Edit code.
2. Rebuild: `gm run . --no-semantic --no-viz` (or `gm run . --update --no-semantic --no-viz` for incremental extraction).
3. Ask the agent to call `reload_graph`, or let the file watcher pick up the new `graph.json` automatically.

Call `graph_info` first. If it reports **Graph may be stale**, rebuild before trusting structural queries.

## Keep the graph focused

Use `.grapheniumignore` to reduce noise.

```gitignore
# Dependencies
node_modules/
vendor/

# Build artifacts
target/
dist/
build/
obj/

# Generated code
*.generated.*
*.Designer.cs
*.g.cs
*.gen.go
*.pb.go
```

## Add semantic extraction later

AST-only is the recommended first run. Semantic extraction can add richer inferred relationships.

```sh
# Anthropic style key
gm run . --update --api-key sk-ant-...

# OpenAI
gm run . --update --provider openai --api-key sk-...

# DeepSeek
gm run . --update --provider deepseek --api-key sk-...
```

Semantic edges must be treated as inferred unless provenance says otherwise.

## Troubleshooting

| Symptom | Likely cause | Action |
|---|---|---|
| Graph has 0 nodes | Unsupported language, ignored files, or wrong directory | Check path and `.grapheniumignore` |
| MCP tool cannot connect | AI tool did not reload config | Fully quit and relaunch the tool |
| Query output is noisy | Vendored or generated files included | Tighten `.grapheniumignore` |
| Too many ambiguous edges | Name collisions or dynamic code | Use `resolution_report` and inspect manually |
| Query misses behavior | Relationship is runtime-only or convention-based | Use semantic extraction or verified manual edges |

## Success criteria

You are ready to use Graphenium when:

- `gm doctor` reports a valid graph
- `gm query` returns relevant symbols and relationships
- Your AI tool can call `graph_info`
- The agent can explain which facts are source-backed
- The agent can produce a pre-edit plan before changing files
