# Graphenium CLI Command Reference

**Graphenium** (`gm`) is the elemental knowledge graph engine for your codebase. It
discovers source files, extracts structural and semantic relationships, builds a
rich dependency graph, and surfaces insights through queries, visualizations, and
CI-friendly quality gates.

All commands use the `gm` binary and follow a standard `gm <command> [options]`
pattern. Most `graph.json` paths default to `graphenium-out/graph.json` in the
current or target directory.

---

## Table of Contents

- [`gm init`](#gm-init) — Initialize workspace
- [`gm run`](#gm-run) — Run analysis pipeline
- [`gm query`](#gm-query) — Query knowledge graph
- [`gm serve`](#gm-serve) — MCP server
- [`gm watch`](#gm-watch) — Watch mode
- [`gm doctor`](#gm-doctor) — Diagnostics
- [`gm check`](#gm-check) — Trust quality gates
- [`gm diff`](#gm-diff) — Snapshot comparison
- [`gm setup`](#gm-setup) — MCP config generation
- [`gm graph`](#gm-graph) — Graph subcommands
- [`gm snapshot`](#gm-snapshot) — Snapshot subcommands
- [`gm gate`](#gm-gate) — Gate subcommands
- [Common workflows](#common-workflows)

---

## `gm init`

Initialize a Graphenium workspace with default configuration files.

**Usage**

```
gm init [path]
```

**Options**

| Argument  | Description                                    | Default |
|-----------|------------------------------------------------|---------|
| `path`    | Directory to initialize (current dir if empty) | `.`     |

The command creates a `.grapheniumignore` file in the target directory. It is
idempotent — running it again on an already-initialized workspace is safe.

**Examples**

```bash
# Initialize the current directory
gm init

# Initialize a specific project directory
gm init ~/projects/my-repo
```

---

## `gm run`

Run the full analysis pipeline on a directory: detect files, extract symbols and
relationships, run semantic analysis, cluster communities, rank nodes, and
optionally generate a report and HTML visualization.

**Usage**

```
gm run [path] [options]
```

**Options**

| Flag                    | Description                                                                    | Default      |
|-------------------------|--------------------------------------------------------------------------------|--------------|
| `path`                  | Directory to analyze                                                           | `.`          |
| `--mode <mode>`         | Extraction mode: `deep` for aggressive inference, otherwise standard           | `standard`   |
| `--update`              | Re-extract only new or modified files (uses mtime manifest)                    | `false`      |
| `--no-semantic`         | Skip LLM semantic extraction; use AST-only results                             | `false`      |
| `--no-viz`              | Skip HTML visualization generation                                             | `false`      |
| `--no-report`           | Skip `GRAPH_REPORT.md` generation                                              | `false`      |
| `--provider <name>`     | AI provider: `anthropic`, `openai`, `openrouter`, `deepseek`, or compatible    | `anthropic`  |
| `--api-base <url>`      | API base URL for `openai-compatible` provider                                  | —            |
| `--model <name>`        | Model override (provider-specific default if omitted)                          | —            |
| `--api-key <key>`       | API key (overrides the provider-specific environment variable)                 | —            |
| `--exclude-dirs <list>` | Comma-separated list of directories to exclude (e.g. `target,node_modules`)    | —            |

**Examples**

```bash
# Run the full pipeline on the current directory
gm run

# Run with deep extraction and a custom AI provider
gm run ~/projects/my-repo --mode deep --provider openai --model gpt-4o

# Incremental update (re-extract only changed files)
gm run --update

# AST-only run (no semantic/LLM pass)
gm run --no-semantic
```

---

## `gm query`

Query the knowledge graph with keywords to find related symbols, files, and
communities.

**Usage**

```
gm query [options] <question>
```

**Options**

| Flag                         | Description                                                  | Default                    |
|------------------------------|--------------------------------------------------------------|----------------------------|
| `question`                   | Keywords or question to match against the graph              | *(required)*               |
| `--graph <path>`             | Path to `graph.json`                                         | `graphenium-out/graph.json`|
| `--dfs`                      | Use depth-first search instead of default BFS                | `false`                    |
| `--safe`                     | Use structural query mode for safer results                  | `false`                    |
| `--budget <n>`               | Maximum output token budget (rough estimate)                 | `2000`                     |
| `--mode <mode>`              | Query mode: `lexical`, `structural`, or `hybrid`             | `lexical`                  |
| `--path-prefix <fragment>`   | Restrict to nodes whose source path contains this fragment   | —                          |
| `--exclude-path <fragment>`  | Exclude nodes whose source path contains this fragment       | —                          |
| `--generated-code-mode <m>`  | Generated-like code filter: `include`, `exclude`, or `only`  | `include`                  |
| `--ast-only-tuning <mode>`   | AST-only tuning: `auto`, `on`, or `off`                      | `auto`                     |

**Examples**

```bash
# Simple keyword query
gm query "database connection pooling"

# Restrict results to a specific module
gm query "authentication middleware" --path-prefix src/auth

# Structural mode with DFS for deeper relationship discovery
gm query "error handling" --mode structural --dfs

# Exclude generated code from results
gm query "core types" --generated-code-mode exclude
```

---

## `gm serve`

Start the MCP (Model Context Protocol) server over stdio JSON-RPC for agent/tool
integration. This allows AI assistants such as Claude Desktop, Cursor, or
CodeWhale to query the knowledge graph interactively.

**Usage**

```
gm serve [options]
```

**Options**

| Flag                  | Description                                                  | Default                    |
|-----------------------|--------------------------------------------------------------|----------------------------|
| `--graph <path>`      | Path to `graph.json`                                         | `graphenium-out/graph.json`|
| `--watch`             | Watch the graph file for changes and auto-reload             | `false`                    |

**Examples**

```bash
# Start the MCP server with the current graph
gm serve

# Start with live-reload when the graph file changes
gm serve --watch

# Serve a specific graph snapshot
gm serve --graph graphenium-out/snapshots/v1.0.json
```

---

## `gm watch`

Watch a directory for file changes and automatically rebuild the knowledge graph.

**Usage**

```
gm watch [path] [options]
```

**Options**

| Flag                  | Description                                                    | Default |
|-----------------------|----------------------------------------------------------------|---------|
| `path`                | Directory to watch                                             | `.`     |
| `--debounce <secs>`   | Debounce interval in seconds before triggering a rebuild       | `3.0`   |
| `--incremental`       | Enable incremental patching: only re-extract changed files     | `true`  |
| `--impact`            | Show blast radius impact analysis after each rebuild           | `false` |

**Examples**

```bash
# Watch the current directory with defaults
gm watch

# Watch with a shorter debounce and impact reporting
gm watch --debounce 1.0 --impact

# Watch a specific project, disabling incremental mode
gm watch ~/projects/my-repo --incremental false
```

---

## `gm doctor`

Run diagnostic checks on the Graphenium installation, loaded graph, tree-sitter
language support, and API key configuration.

**Usage**

```
gm doctor [options]
```

**Options**

| Flag                | Description                                                       | Default      |
|---------------------|-------------------------------------------------------------------|--------------|
| `--graph <path>`    | Optional path to a specific `graph.json`                          | —            |
| `--schema`          | Show graph schema information (nodes, edges, communities)         | `false`      |
| `--resolution`      | Show resolution quality report (extracted/inferred/ambiguous)     | `false`      |
| `--repository`      | Show repository metadata from the graph                           | `false`      |
| `--json`            | Output diagnostics as structured JSON (programmatic consumption)  | `false`      |

Without any flag, `gm doctor` runs a full health check covering:

- Binary on PATH
- Graph file existence and validity
- Schema version, extraction modes, languages
- Tree-sitter grammar availability
- API key environment variables
- Graph quality (extracted vs. inferred vs. ambiguous edge ratios)

**Examples**

```bash
# Full health check
gm doctor

# Show graph schema information
gm doctor --schema

# Show resolution quality report
gm doctor --resolution

# Show repository metadata
gm doctor --repository

# JSON output for programmatic consumption
gm doctor --json
```

---

## `gm check`

Run trust quality checks and enforce gates for CI. Validates that the knowledge
graph meets minimum quality thresholds.

**Usage**

```
gm check [options]
```

**Options**

| Flag                       | Description                                                | Default                    |
|----------------------------|------------------------------------------------------------|----------------------------|
| `--graph <path>`           | Path to `graph.json`                                       | `graphenium-out/graph.json`|
| `--min-resolution <pct>`   | Minimum resolution percentage (0–100)                      | `80.0`                     |
| `--max-ambiguous <n>`      | Maximum number of ambiguous edges allowed                  | `10`                       |
| `--strict`                 | Exit with non-zero status if any check fails               | `false`                    |

**Examples**

```bash
# Run checks with default thresholds
gm check

# Strict CI gate: fail if resolution < 90% or ambiguous > 5
gm check --min-resolution 90 --max-ambiguous 5 --strict

# Check a specific graph file
gm check --graph graphenium-out/snapshots/release-2.0.json
```

---

## `gm diff`

Diff two graph snapshots and show symbol-level changes. Useful for reviewing
what changed between runs or releases.

**Usage**

```
gm diff [options]
```

**Options**

| Flag                  | Description                                                      | Default                    |
|-----------------------|------------------------------------------------------------------|----------------------------|
| `--before <path>`     | Path to the old (before) `graph.json`                            | Last snapshot or empty     |
| `--after <path>`      | Path to the new (after) `graph.json`                             | `graphenium-out/graph.json`|
| `--impact`            | Show detailed downstream impact/ blast-radius analysis           | `false`                    |
| `--review-plan`       | Show a prioritized review plan with verification steps           | `false`                    |

**Examples**

```bash
# Diff the latest graph against the last snapshot
gm diff

# Compare two specific snapshots
gm diff --before graphenium-out/snapshots/baseline.json \
        --after  graphenium-out/snapshots/after-refactor.json

# Show impact analysis
gm diff --before old.json --after new.json --impact

# Generate a review plan from the diff
gm diff --before old.json --after new.json --review-plan
```

---

## `gm setup`

Print MCP setup instructions for an AI assistant. Generates the JSON
configuration snippet needed to register Graphenium as an MCP tool server.

**Usage**

```
gm setup <target> [options]
```

**Options**

| Flag                | Description                                                  | Default                    |
|---------------------|--------------------------------------------------------------|----------------------------|
| `target`            | Target assistant: `claude`, `cursor`, or `codewhale`         | *(required)*               |
| `--gm-path <path>`  | Path to the `gm` binary (auto-detected if omitted)           | —                          |
| `--graph <path>`    | Path to `graph.json`                                         | `graphenium-out/graph.json`|

**Examples**

```bash
# Setup for Claude Desktop
gm setup claude

# Setup for Cursor with a custom binary path
gm setup cursor --gm-path /usr/local/bin/gm

# Setup for CodeWhale
gm setup codewhale
```

---

## `gm graph`

Inspect and manage the knowledge graph with subcommands.

**Usage**

```
gm graph <subcommand> [options]
```

### `gm graph schema`

Load and print graph metadata and schema information (nodes, edges, communities,
extraction modes, languages).

```
gm graph schema [path]
```

| Argument | Description                                          | Default                    |
|----------|------------------------------------------------------|----------------------------|
| `path`   | Path to `graph.json`                                 | `graphenium-out/graph.json`|

### `gm graph migrate`

Migrate a graph from an older schema version.

```
gm graph migrate <path>
```

| Argument | Description                        | Default |
|----------|------------------------------------|---------|
| `path`   | Path to the `graph.json` file      | *(required)* |

### `gm graph build-map`

Print build targets discovered during CI extraction.

```
gm graph build-map [path]
```

| Argument | Description                                          | Default                    |
|----------|------------------------------------------------------|----------------------------|
| `path`   | Path to `graph.json`                                 | `graphenium-out/graph.json`|

### `gm graph test-map`

Print test targets discovered during CI extraction.

```
gm graph test-map [path]
```

| Argument | Description                                          | Default                    |
|----------|------------------------------------------------------|----------------------------|
| `path`   | Path to `graph.json`                                 | `graphenium-out/graph.json`|

**Examples**

```bash
# Inspect graph schema
gm graph schema

# Show build targets from CI extraction
gm graph build-map

# Show test targets
gm graph test-map

# Migrate an older graph to the current schema
gm graph migrate old-graph.json
```

---

## `gm snapshot`

Create and manage graph snapshots — point-in-time copies of the knowledge graph
that can be compared with `gm diff`.

**Usage**

```
gm snapshot <subcommand> [options]
```

### `gm snapshot create`

Create a new snapshot of the current graph.

```
gm snapshot create --name <name>
```

| Flag       | Description               | Default |
|------------|---------------------------|---------|
| `--name`   | Name for the snapshot     | *(required)* |

Snapshots are stored as `graphenium-out/snapshots/<name>.json`.

### `gm snapshot list`

List all available snapshots with file sizes.

```
gm snapshot list
```

Takes no arguments or flags.

**Examples**

```bash
# Create a named snapshot
gm snapshot create --name pre-refactor

# List all snapshots
gm snapshot list

# Diff two snapshots
gm diff --before graphenium-out/snapshots/baseline.json \
        --after  graphenium-out/snapshots/post-refactor.json \
        --impact
```

---

## `gm gate`

Run gate checks and quality gates for CI. Currently supports diff-based gates.

**Usage**

```
gm gate [options]
```

**Options**

| Flag                              | Description                                        | Default |
|-----------------------------------|----------------------------------------------------|---------|
| `--diff <before> <after>`         | Diff two graph snapshots as a gate check           | —       |

The `--diff` option compares two `graph.json` files and applies quality policy
gates. If either before or after fails the configured thresholds, the gate
reports a failure.

**Examples**

```bash
# Gate check between two snapshots
gm gate --diff graphenium-out/snapshots/baseline.json \
               graphenium-out/snapshots/current.json
```

---

## Common workflows

### Full analysis with report

```bash
# Initialize, run complete pipeline, and produce a report
cd ~/projects/my-repo
gm init
gm run --mode deep --provider anthropic
```

The pipeline creates `graphenium-out/graph.json`,
`graphenium-out/GRAPH_REPORT.md`, and (unless `--no-viz` is passed) an HTML
visualization.

### CI quality gate

```bash
# Run the pipeline, check quality, and enforce strict thresholds
gm run
gm check --min-resolution 90 --max-ambiguous 5 --strict

# Create a snapshot for later comparison
gm snapshot create --name ci-build-1234
```

If any threshold is exceeded, `gm check --strict` exits non-zero, failing the
CI step.

### Interactive MCP integration

```bash
# Build the graph and start the MCP server in watch mode
gm run
gm serve --watch

# In another terminal, register with your AI assistant
gm setup claude

# The assistant can now query with: "what does the authentication module depend on?"
```

### Incremental development loop

```bash
# Initial full run
gm run

# Watch mode with impact analysis for live feedback
gm watch --debounce 1.0 --impact

# After making changes, create a snapshot and compare
gm snapshot create --name after-changes
gm diff --impact --review-plan
```

The `--impact` flag shows downstream blast radius after each rebuild, helping
you understand the ripple effects of your changes.
