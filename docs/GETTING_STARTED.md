# Getting Started with Graphenium

## Prerequisites

- Rust 1.81+
- A codebase to analyze (any mix of Rust, Python, Go, JavaScript, TypeScript, Java, C, C++, C#)

## Installation

```sh
# From source
cargo install --locked --path .

# With minimal languages
cargo install --locked --path . --no-default-features --features lang-python,lang-rust
```

## First Run

```sh
# 1. Initialize workspace
gm init

# 2. Build the graph (AST-only, fast)
gm run . --no-semantic --no-viz

# 3. Inspect the graph
gm doctor
gm query "my feature" --budget 1000

# 4. Start the MCP server (for agent use)
gm serve --graph graphenium-out/graph.json
```

## Workflows

### Understand a module before editing

```sh
gm query "module_name" --depth 2 --budget 2000
gm query "module_name function_name" --safe --budget 1500
gm doctor --resolution
```

### Find architectural hotspots

```sh
gm query "god node" --mode hybrid --budget 3000
gm query "architecture summary" --budget 2000
```

### CI quality gate

```sh
gm check --min-resolution 80 --max-ambiguous 10
```

### Design-then-verify compliance check

```sh
# Create a planning workspace, declare intended symbols, implement, then verify:
gm plan create --name "my-change"
gm plan add-symbol --plan "my-change" --symbol "new_utils" --kind module
# ... write the code ...
gm verify-plan --plan "my-change"
```

This workflow lets AI agents formally verify that their implemented code matches the planned design before requesting review.

### Incremental development

```sh
# Terminal 1: watch for changes
gm watch . --impact

# Terminal 2: edit code, see live blast radius
```

## MCP Integration

### Claude Desktop
Add the graphemium MCP server to your `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "graphenium": {
      "command": "gm",
      "args": ["serve", "--graph", "/path/to/graphenium-out/graph.json"]
    }
  }
}
```

### Cursor
Add to `~/.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "graphenium": {
      "command": "gm",
      "args": ["serve", "--graph", "/path/to/graphenium-out/graph.json"]
    }
  }
}
```

### CodeWhale
Add to `~/.codewhale/mcp.json`:

```json
{
  "graphenium": {
    "command": "gm",
    "args": ["serve", "--graph", "/path/to/graphenium-out/graph.json"]
  }
}
```

## First Query

Once the MCP server is running, ask your agent:

```text
Use Graphenium to understand the architecture before we make changes.
Call graph_info first to confirm which graph is loaded.
```

The agent should respond with communities, hubs, and relevant files.

## Key MCP Tools

| Tool | Use Case |
|------|----------|
| `graph_info` | Confirm which graph is loaded |
| `architecture_summary` | High-level codebase shape |
| `query_graph` | Keyword + traversal search |
| `summarize_file` | What's in a file (without reading it) |
| `get_neighbors` | What connects to a symbol |
| `shortest_path` | Dependency chain between two symbols |
| `safest_path` | Highest-confidence dependency chain |
| `resolution_report` | How trustworthy is this graph |
| `next_files_to_read` | Prioritized reading list for changes |
| `blast_radius` | Impact analysis for proposed changes |
| `run_datalog` | Declarative graph queries with rules |

## Creating a `grapheniumignore`

Create `.grapheniumignore` in your project root:

```
target/
node_modules/
dist/
build/
vendor/
*.generated.*
```

Or use `gm init` to create one with sensible defaults.
