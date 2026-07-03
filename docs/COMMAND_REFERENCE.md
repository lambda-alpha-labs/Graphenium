# Command Reference

The `gm` binary serves as both a CLI and an MCP server. This reference covers all commands and flags.

## `gm init`

Initialize a Graphenium workspace. Creates `.grapheniumignore` with sensible defaults.

```
gm init [path]
```

| Flag | Type | Description |
|------|------|-------------|
| `path` | PathBuf | Project root (default: current dir) |

## `gm run`

Run the analysis pipeline: detect files, extract AST, resolve imports, cluster, analyze.

```
gm run <path> [flags]
```

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| path | PathBuf |: | Project root directory |
| `--mode` | String | `"ast"` | Extraction mode: `ast`, `semantic` |
| `--no-semantic` | bool | false | Skip semantic LLM pass |
| `--no-viz` | bool | false | Skip HTML visualization |
| `--no-report` | bool | false | Skip GRAPH_REPORT.md generation |
| `--exclude-dirs` | String |: | Comma-separated directory patterns to skip |
| `--plan` | bool | false | Dry-run: scan and report file stats only |
| `--update` | bool | false | Incremental update (re-extract changed files) |

## `gm query`

Query the knowledge graph with keywords or Datalog programs.

```
gm query "<keywords>" [flags]
```

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| keywords | String |: | Search query or Datalog program |
| `--mode` | String | `lexical` | Retrieval mode: `lexical`, `structural`, `hybrid` |
| `--datalog` | String |: | Run a Datalog query instead of keyword search |
| `--budget` | Int | 2000 | Output token budget |
| `--depth` | Int | 3 | Traversal depth (1-6) |
| `--datalog` | String |: | Run a Datalog query instead of keyword search |
| `--dfs` | bool | false | Use DFS instead of BFS |
| `--safe` | bool | false | Confidence-aware traversal |
| `--graph` | PathBuf |: | Path to graph.json |
| `--json` | bool | false | Output as JSON array |
| `--path-prefix` | String |: | Include only matching paths |
| `--exclude-path` | String |: | Exclude matching paths |
| `--generated-code` | String |: | Generated code filter: `include`, `exclude`, `only` |
| `--ast-only-tuning` | bool |: | Tune for AST-only graphs |
| `--include-tests` | bool | false | Include test nodes in results |

## `gm serve`

Start the MCP server.

```
gm serve [--graph <path>] [--watch]
```

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--graph` | PathBuf |: | Path to graph.json |
| `--watch` | bool | false | Auto-reload graph on file change |

## `gm watch`

Watch mode: re-extract on file changes.

```
gm watch <path> [flags]
```

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| path | PathBuf |: | Directory to watch |
| `--graph` | PathBuf |: | Output graph path |
| `--debounce` | Float | 3.0 | Debounce seconds |
| `--incremental` | bool | true | Incremental re-extraction |
| `--impact` | bool | false | Show blast radius on changes |

## `gm doctor`

Diagnostic checks for the graph and environment.

```
gm doctor [flags]
```

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--graph` | PathBuf |: | Path to graph.json |
| `--json` | bool | false | Structured JSON output |
| `--schema` | bool | false | Show graph schema version |
| `--resolution` | bool | false | Show resolution quality |
| `--repository` | bool | false | Show repository metadata |

## `gm check`

Trust quality gates for CI.

```
gm check [flags]
```

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--graph` | PathBuf |: | Path to graph.json |
| `--min-resolution` | Float | 80.0 | Minimum import resolution % |
| `--max-ambiguous` | Int | 10 | Maximum allowed ambiguous edges |
| `--strict` | bool | false | Also fail on warnings |
| `--plan` | String |: | Verify a planning workspace ID |

## `gm diff`

Compare graph snapshots.

```
gm diff [flags]
```

| Flag | Type | Description |
|------|------|-------------|
| `--before` | PathBuf | Before graph snapshot |
| `--after` | PathBuf | After graph snapshot |
| `--impact` | bool | Show downstream impact analysis |
| `--review-plan` | bool | Generate verification plan |
| `--json` | bool | Output as JSON |

## `gm explain`

Pre-edit architectural orientation for a symbol.

```
gm explain <symbol> [--graph <path>]
```

| Flag | Type | Description |
|------|------|-------------|
| symbol | String | Node ID or label to explain |
| `--graph` | PathBuf | Path to graph.json |

## `gm setup`

Generate MCP configuration for common tools.

```
gm setup <target> [--gm-path <path>] [--graph <path>]
```

| Target | Description |
|--------|-------------|
| `claude-desktop` | Claude Desktop (macOS) |
| `claude-code` | Claude Code (CLI) |
| `cursor` | Cursor editor |
| `codewhale` | CodeWhale |

## `gm graph`

Graph metadata subcommands.

```
gm graph <subcommand> [args]
```

| Subcommand | Description |
|------------|-------------|
| `migrate` | Migrate graph.json schema version |
| `schema` | Show graph schema version |
| `build-map` | Show build dependency map |
| `test-map` | Show test-to-source map |

## `gm snapshot`

Snapshot management.

```
gm snapshot <subcommand> [args]
```

| Subcommand | Args | Description |
|------------|------|-------------|
| `create` | `<name>` | Create a named snapshot |
| `list` |: | List all snapshots with sizes |

## `gm gate`

Quality gate subcommands.

```
gm gate --diff <before> <after>
```

## Common Workflows

```
# Build and query
gm run . --no-semantic
gm query "authentication flow"

# CI gate
gm check --graph graphenium-out/graph.json

# Incremental development
gm watch src/ --impact --graph graphenium-out/graph.json
