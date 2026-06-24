# AI Setup Playbook for Graphenium

> **For AI assistants.** Read this entire file, then execute each step.
> Report progress to the user as you go. If a step fails, explain the
> error and offer the user a choice: retry, skip, or stop.

## Goal

Install Graphenium (`gm`), build a knowledge graph for the current
project, and connect it to the AI tool the user is running. When this
playbook finishes, the user can ask structural questions about their
codebase: "what calls this?", "what community is this module in?",
"what are the hub nodes?" and get answers immediately.

## Step 1: Detect the environment

Check and record these facts before proceeding:

```sh
uname -s        # Darwin, Linux, or MINGW*
uname -m        # arm64, x86_64
which rustc     # empty if Rust is not installed
which cargo     # empty if Cargo is not installed
which gm        # empty if Graphenium is not installed
pwd             # the project directory to analyze
```

Report the findings to the user in one line: OS, architecture, Rust
status, Graphenium status.

## Step 2: Install Rust (if missing)

If `rustc` is already present, skip to Step 3.

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
```

After installation, source the environment:

```sh
source "$HOME/.cargo/env"
```

Verify:

```sh
rustc --version   # should print "rustc 1.75.0" or later
cargo --version   # should print "cargo 1.75.0" or later
```

If verification fails, stop and report the error. Do not proceed.

## Step 3: Clone and build Graphenium

```sh
# Clone to a stable location (not inside the project being analyzed)
git clone https://github.com/lambda-alpha-labs/Graphenium "$HOME/.graphenium"
cd "$HOME/.graphenium"

# Build and install. This compiles tree-sitter grammars for 9 languages;
# it may take 2-5 minutes on the first build.
cargo install --path .
```

Verify:

```sh
which gm            # should print a path
gm --version        # should print a version number
```

If the user wants a smaller binary (only specific languages), rebuild:

```sh
cargo install --path . --no-default-features --features lang-python,lang-rust
```

Available language features: `lang-python`, `lang-js`, `lang-ts`,
`lang-rust`, `lang-go`, `lang-java`, `lang-c`, `lang-cpp`, `lang-csharp`.

## Step 4: Run the first scan

Return to the project directory and run the AST-only pipeline. This
requires no API key; it uses tree-sitter to extract structure from
source files.

```sh
cd <project-directory>    # the directory from Step 1
gm run . --no-semantic --no-viz
```

Expected output includes lines like:

```
[graphenium] Found N file(s)
[graphenium] AST: N nodes, N edges
[graphenium] Graph: N nodes, N edges
[graphenium] Communities: N
[graphenium] Report written to ...
```

The output is in `graphenium-out/graph.json`.

**If the graph is empty** (0 nodes), the project may not contain
supported source files. Graphenium supports Python, JavaScript,
TypeScript, Rust, Go, Java, C, C++, and C#. If the project uses an
unsupported language, tell the user and stop.

**Optional: add semantic extraction later.** The user can run a
semantic pass to add LLM-inferred relationships for richer cross-file
connectivity. Graphenium supports multiple AI providers:

```sh
# Anthropic (default)
gm run . --update --api-key sk-ant-...

# OpenAI
gm run . --update --provider openai --api-key sk-...

# DeepSeek
gm run . --update --provider deepseek --api-key sk-...

# Or set the provider-specific env var:
export OPENAI_API_KEY=sk-...
gm run . --update --provider openai
```

Make a note of this but do not require it now.

## Step 5: Detect which AI tool to configure

Ask the user: "Which AI coding tool are you using?"

| Option | Tool | Config location (macOS / Linux) |
|--------|------|--------------------------------|
| 1 | CodeWhale | `~/.codewhale/mcp.json` |
| 2 | Claude Desktop | `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS) or `~/.config/Claude/claude_desktop_config.json` (Linux) |
| 3 | Cursor | `~/.cursor/mcp.json` |
| 4 | Other / not sure | Skip MCP config; the `gm query` CLI and the Skill work without it |

If the user picks option 4, skip to Step 7.

## Step 6: Configure MCP

Use the **absolute path** to `gm` from Step 3 and the **absolute path**
to the graph from Step 4. Replace `$GM_PATH` and `$GRAPH_PATH` below
with the actual values.

### CodeWhale

CodeWhale loads MCP servers from `~/.codewhale/mcp.json` (JSON format).
If the file doesn't exist, create it. If it already has other servers,
merge the `graphenium` entry under `servers`:

```json
{
  "servers": {
    "graphenium": {
      "command": "$GM_PATH",
      "args": ["serve", "--graph", "$GRAPH_PATH"],
      "env": {}
    }
  }
}
```

If Rust was installed to a custom location (e.g. inside the workspace
due to sandbox restrictions), include `CARGO_HOME` and `RUSTUP_HOME`
in the `env` object:

### Claude Desktop

Read the config file. Navigate to `mcpServers`. If `mcpServers` does
not exist, create it as an empty object `{}`. Add or update the
`graphenium` key:

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

If the file contains other `mcpServers` entries, preserve them. Only
add or update the `graphenium` key.

### Cursor

Read `~/.cursor/mcp.json` if it exists. Add or update:

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

### After writing config

Tell the user: "The MCP config has been written. **Quit and relaunch your
AI tool completely** (Cmd+Q on macOS, not just close the window) for the
Graphenium server connection to take effect. MCP server definitions are
only loaded at process startup."

## Step 7: Verify

```sh
gm query "test" --budget 100
```

This should return a Markdown summary (even if no nodes match, it
should not crash). If `gm query` runs without errors, Graphenium is
working.

## Step 8: Done. Tell the user

Report a summary:

- Graphenium installed: `$GM_PATH`
- Graph built: `$GRAPH_PATH` (N nodes, N edges)
- MCP configured: yes / skipped
- Skill location: the repo ships `skills/graphenium/SKILL.md`. It is
  auto-discovered when the project is opened as a workspace. It teaches
  the AI which MCP tool to use for which question and how to interpret
  the confidence model.

**What the user can do now:**

- Ask structural questions: "what calls X?", "what are the hub nodes?",
  "what community is this module in?"
- Run `gm run . --update` periodically to keep the graph current
- Run `gm run . --api-key sk-ant-...` to add LLM-inferred edges
- Run `gm watch .` in a terminal for automatic rebuilds on file changes

## Error recovery

| Symptom | Likely cause | Action |
|---------|-------------|--------|
| `cargo install` fails | Missing build tools (cc, cmake) | `xcode-select --install` (macOS) or `apt install build-essential` (Linux) |
| `gm run` finds 0 files | Unsupported language or empty project | Check file extensions; suggest `--no-semantic` if API key is the issue |
| MCP config path doesn't exist | Tool not installed | Skip MCP; the CLI fallback still works |
| `gm query` errors | Graph file path wrong | Verify `graphenium-out/graph.json` exists from Step 4 |
