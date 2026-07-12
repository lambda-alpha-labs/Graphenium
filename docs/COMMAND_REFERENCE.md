# CLI Command Reference

The Graphenium command-line interface (`gm`) serves as both an interactive analysis tool and an automated gatekeeper. It compiles codebases into AST-proven structural indexes, runs Datalog policy solvers, and executes pre-flight containment checks.

---

## Command Map

| Command | Primary Purpose |
|---|---|
| [`gm init`](#gm-init) | Initialize a workspace and configure `.grapheniumignore`. |
| [`gm run`](#gm-run) | Compile the codebase's AST symbols, cross-file imports, and namespaces. |
| [`gm query`](#gm-query) | Execute structural searches, path traces, or declarative Datalog constraints. |
| [`gm check`](#gm-check) | Enforce pre-flight architectural policies and post-facto scope audits. |
| [`gm serve`](#gm-serve) | Initialize the background MCP server on stdio. |
| [`gm watch`](#gm-watch) | Re-compile the index in real-time as physical file modifications occur. |
| [`gm doctor`](#gm-doctor) | Diagnose index integrity, schema compliance, and resolution health. |
| [`gm diff`](#gm-diff) | Compare structural snapshots to identify additions, removals, and drift. |
| [`gm explain`](#gm-explain) | Generate a pre-edit structural orientation summary for a target symbol. |
| [`gm setup`](#gm-setup) | Generate workspace MCP configurations for IDEs and assistants. |
| [`gm graph`](#gm-graph) | Subcommands to inspect compiled metadata, build targets, and test mappings. |
| [`gm snapshot`](#gm-snapshot) | Create and list indexed codebase snapshots. |
| [`gm gate`](#gm-gate) | Execute automated containment gates over index differences. |

---

## `gm init`
Initializes Graphenium's configuration inside the target directory and generates a default `.grapheniumignore` file to filter out build artifacts and external vendor code.

```sh
gm init [path]
```

### Arguments:
*   `path` (string, optional, default: `.`) — The path to the repository root directory.

---

## `gm run`
Parses codebase source files, extracts declarations (classes, structs, functions, methods, interfaces, types), resolves cross-file import paths and calls, partitions cohesive domains (communities), and writes the structural index.

```sh
gm run [path] [flags]
```

### Arguments:
*   `path` (string, optional, default: `.`) — The repository directory to analyze.

### Flags:
*   `--mode <mode>` (string, default: `standard`) — Extraction depth: `standard` for fast AST + Stack Graphs or `deep` for aggressive inference.
*   `--no-semantic` (bool) — Explicitly skips remote LLM passes, guaranteeing 100% offline, local execution.
*   `--no-viz` (bool) — Disables the generation of the local `graph.html` visualization file.
*   `--no-report` (bool) — Disables compilation of the local `GRAPH_REPORT.md` summary.
*   `--exclude-dirs <dirs>` (string) — Comma-separated folder patterns to skip during analysis.
*   `--plan` (bool) — Executes a dry-run analysis, reporting scanned file boundaries without writing the index.
*   `--update` (bool) — Activates incremental patching, compiling only changed or affected source files.
*   `--provider <provider>` (string, default: `anthropic`) — AI provider: `anthropic`, `openai`, `openrouter`, `deepseek`, or `openai-compatible`.
*   `--api-key <key>` (string) — API key (overrides the provider-specific env var).
*   `--api-base <url>` (string) — API base URL for `openai-compatible` provider.
*   `--model <model>` (string) — Model to use (defaults to provider-specific default).

---

## `gm query`
Executes structural queries against Graphenium's local index. Supports lexical searches, topological path searches, and first-order Datalog queries.

```sh
gm query "<query>" [flags]
```

### Arguments:
*   `query` (string, required) — The keyword query or declarative Datalog program.

### Flags:
*   `--mode <mode>` (string, default: `lexical`) — Search strategy:
    *   `lexical`: Relevance matching against symbol names, qualified paths, and source files.
    *   `structural`: Topological distance sorting radiating outward from matched seed nodes.
    *   `hybrid`: Combined score weighting lexical relevance (60%) and structural distance (40%).
*   `--datalog <program>` (string) — Executes a declarative Datalog program against Graphenium's relational EDB. Standard library rules (`stdlib.dl`) are automatically merged pre-flight [1.1.2].
*   `--budget <tokens>` (integer, default: `2000`) — Limits Graphenium's printed output to remain within the specified context-token limit.
*   `--dfs` (bool) — Uses Depth-First Search for structural tracing (default: BFS).
*   `--safe` (bool) — Restricts query-tracing strictly to AST-proven (`EXTRACTED`) dependencies.
*   `--graph <path>` (string, default: `graphenium-out/graph.json`) — Overrides the source index file location.
*   `--json` (bool) — Outputs query results as a raw JSON array instead of a formatted Markdown report.
*   `--path-prefix <path>` (string) — Restricts results to symbols located inside the specified directory path.
*   `--exclude-path <path>` (string) — Filters out results located inside the specified directory path.
*   `--generated-code-mode <mode>` (string, default: `include`) — How to handle files classified as generated: `include`, `exclude`, or `only`.
*   `--ast-only-tuning <mode>` (string, default: `auto`) — AST-only tuning: `auto`, `on`, or `off`.

---

## `gm check`
Enforces global resolution thresholds and validates planning workspaces. This is Graphenium's primary CI/CD and pre-commit containment hook.

```sh
gm check [flags]
```

### Flags:
*   `--graph <path>` (string, default: `graphenium-out/graph.json`) — Path to the codebase index file.
*   `--min-resolution <pct>` (float, default: `80.0`) — Fails the check if Graphenium's AST import resolution ratio drops below the target percentage.
*   `--max-ambiguous <count>` (integer, default: `10`) — Fails the check if Graphenium detects more than `<count>` unresolved name collisions.
*   `--strict` (bool) — Fails the check on any index, parser, or configuration warnings.
*   `--plan <id>` (string) — Executes a double-gate verification for a planning workspace: runs pre-flight policy validation, followed by a post-facto scope-creep audit.

---

## `gm serve`
Starts Graphenium's Model Context Protocol (MCP) server, allowing AI coding assistants to invoke codebase tools natively via JSON-RPC.

```sh
gm serve [flags]
```

### Flags:
*   `--graph <path>` (string, default: `graphenium-out/graph.json`) — Path to Graphenium's compiled codebase index.
*   `--watch` (bool) — Monitors the compiled index file and hot-reloads the server's state in-memory as modifications occur.

---

## `gm watch`
Watches filesystem sources in real-time, executing incremental AST patching and boundary re-compilation as code edits occur.

```sh
gm watch <path> [flags]
```

### Arguments:
*   `path` (string, required) — The directory path to monitor.

### Flags:
*   `--debounce <secs>` (float, default: `3.0`) — Wait time in seconds after the last file modification before compiling changes.
*   `--incremental` (bool, default: `true`) — Restricts re-extraction to modified files and their direct importers, skipping full repository re-scans.
*   `--impact` (bool) — Prints the structural blast radius (affected callers, dependent domains) immediately after compiling an update.

---

## `gm doctor`
Diagnoses the health of Graphenium's environment, compilation boundaries, index schema compliance, and resolution ratios.

```sh
gm doctor [flags]
```

### Flags:
*   `--graph <path>` (string, default: `graphenium-out/graph.json`) — Path of Graphenium's compiled codebase index.
*   `--schema` (bool) — Displays graph.json schema version and compiler versions.
*   `--resolution` (bool) — Outputs a detailed resolution report (imports, calls, methods, and evidence freshness status).
*   `--repository` (bool) — Show repository info from graph metadata.

---

## `gm diff`
Compares two index files to audit structural modifications (additions, removals, and domain drifts).

```sh
gm diff --before <path> --after <path> [flags]
```

### Flags:
*   `--before <path>` (string, required) — Baseline index snapshot.
*   `--after <path>` (string, required) — Post-change index snapshot.
*   `--impact` (bool) — Evaluates the downstream transitive impact of all added and removed symbols.
*   `--review-plan` (bool) — Generates a risk-sorted review plan for PR reviews.

---

## `gm explain`
Provides a structural orientation summary for a target symbol. Explains where the symbol lives, what is compiler-proven to call it, and what dependencies it imports.

```sh
gm explain <symbol> [flags]
```

### Arguments:
*   `symbol` (string, required) — The class, method, or function name to explain.

### Flags:
*   `--graph <path>` (string, default: `graphenium-out/graph.json`) — Path of Graphenium's codebase index.

---

## `gm setup`
Generates MCP server configuration snippets for target developer tools.

```sh
gm setup <target> [flags]
```

### Arguments:
*   `target` (string, required) — The environment to configure: `claude`, `cursor`, `codewhale`.

### Flags:
*   `--gm-path <path>` (string) — Explicitly overrides the absolute path to Graphenium's compiled CLI binary.
*   `--graph <path>` (string) — Explicitly overrides the target index file path.

---

## `gm graph`
Subcommand suite to inspect index metadata, compilation mappings, and build targets.

```sh
gm graph <subcommand>
```

### Subcommands:
*   `schema` — Displays the index schema version.
*   `migrate` — Migrates older index schemas to the current version.
*   `build-map` — Prints the extracted build-to-source mapping.
*   `test-map` — Prints the extracted test-to-source validation mapping.

---

## `gm snapshot`
Manages index snapshots, facilitating change gating and manual structural reviews.

```sh
gm snapshot <subcommand>
```

### Subcommands:
*   `create <name>` — Generates a named snapshot of Graphenium's current index under `graphenium-snapshots/`.
*   `list` — Lists all available snapshots, timestamps, and compiled sizes.

---

## `gm gate`
Executes automated PR containment gates over index differences.

```sh
gm gate --diff <before> <after>
```

### Flags:
*   `--diff <before> <after>` (strings, required) — The baseline and post-change index files to gate.
