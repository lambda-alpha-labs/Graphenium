# Command Reference

The `gm` binary is both a CLI and an MCP server. The CLI is useful for local setup, graph builds, diagnostics, CI gates, snapshots, and direct graph queries.

## Command map

| Command | Purpose |
|---|---|
| `gm init` | Initialize a Graphenium workspace |
| `gm run` | Build or update the graph |
| `gm query` | Query the graph by keyword, traversal, or Datalog |
| `gm serve` | Start the MCP server |
| `gm watch` | Rebuild on file changes |
| `gm doctor` | Diagnose environment and graph health |
| `gm check` | Run trust quality gates |
| `gm diff` | Compare graph snapshots |
| `gm explain` | Explain a symbol before editing |
| `gm setup` | Generate MCP setup snippets |
| `gm graph` | Inspect or migrate graph metadata |
| `gm snapshot` | Manage snapshots |
| `gm gate` | Run gate workflows over diffs |

## `gm init`

Initialize a workspace and create `.grapheniumignore`.

```sh
gm init [path]
```

| Argument | Type | Default | Description |
|---|---|---|---|
| `path` | path | current directory | Project root |

Use this before the first graph build.

## `gm run`

Run the analysis pipeline: detect files, parse ASTs, resolve imports and symbols, cluster communities, analyze topology, and export artifacts.

```sh
gm run <path> [flags]
```

| Flag | Type | Default | Description |
|---|---|---|---|
| `path` | path | required | Project root directory |
| `--mode` | string | `ast` | Extraction mode, such as `ast` or `semantic` |
| `--no-semantic` | bool | false | Skip semantic LLM pass |
| `--no-viz` | bool | false | Skip HTML visualization |
| `--no-report` | bool | false | Skip report generation |
| `--exclude-dirs` | string | none | Comma-separated directory patterns to skip |
| `--plan` | bool | false | Dry run: scan and report file stats only |
| `--update` | bool | false | Incremental update |
| `--provider` | string | provider default | Semantic provider, when semantic extraction is enabled |
| `--api-key` | string | none | Provider API key for semantic extraction |

Common use:

```sh
gm run . --no-semantic --no-viz
```

## `gm query`

Query the graph with keywords or Datalog.

```sh
gm query "<keywords>" [flags]
gm query --datalog "<program>"
```

| Flag | Type | Default | Description |
|---|---|---|---|
| `keywords` | string | required for keyword mode | Search query |
| `--mode` | string | `lexical` | `lexical`, `structural`, or `hybrid` |
| `--datalog` | string | none | Run a Datalog query |
| `--budget` | integer | 2000 | Output token budget |
| `--depth` | integer | 3 | Traversal depth, usually 1 to 6 |
| `--dfs` | bool | false | Use DFS instead of BFS |
| `--safe` | bool | false | Prefer confidence-aware traversal |
| `--graph` | path | default output graph | Path to `graph.json` |
| `--json` | bool | false | Output JSON array |
| `--path-prefix` | string | none | Include only matching paths |
| `--exclude-path` | string | none | Exclude matching paths |
| `--generated-code` | string | default behavior | `include`, `exclude`, or `only` |
| `--ast-only-tuning` | bool | auto | Tune output for AST-only graphs |
| `--include-tests` | bool | false | Include test nodes |

Datalog queries automatically merge the standard library (v0.19.0+). Pre-loaded predicates include `calls_transitive/2`, `imports_transitive/2`, `depends_transitive/2`, `same_community/2`, `is_hub/1`, `is_orphan/1`, `circular_dependency/2`, and `bypasses_layer/3`. Base EDB relations: `calls/3`, `imports/3`, `contains/3`, `inherits/3`, `implements/3`, `degree/2`, `hub/1`.

Examples:

```sh
gm query "authentication flow" --mode hybrid --budget 3000
gm query "Parser.parse_file" --safe --depth 2 --budget 1500
gm query "hubs" --datalog "?- is_hub(X)."
gm query "reach" --datalog "?- calls_transitive('main', X)."
```

## `gm serve`

Start the MCP server.

```sh
gm serve [--graph <path>] [--watch]
```

| Flag | Type | Default | Description |
|---|---|---|---|
| `--graph` | path | default output graph | Path to `graph.json` |
| `--watch` | bool | false | Auto-reload graph when it changes |

Use this for Claude Desktop, Cursor, CodeWhale, Claude Code, or custom MCP clients.

## `gm watch`

Watch files and re-extract on changes.

```sh
gm watch <path> [flags]
```

| Flag | Type | Default | Description |
|---|---|---|---|
| `path` | path | required | Directory to watch |
| `--graph` | path | output graph | Graph output path |
| `--debounce` | float | 3.0 | Debounce in seconds |
| `--incremental` | bool | true | Use incremental extraction |
| `--impact` | bool | false | Show blast radius on changes |

## `gm doctor`

Run diagnostics for environment, graph metadata, schema, and quality.

```sh
gm doctor [flags]
```

| Flag | Type | Default | Description |
|---|---|---|---|
| `--graph` | path | default output graph | Path to `graph.json` |
| `--json` | bool | false | Structured JSON output |
| `--schema` | bool | false | Show graph schema version |
| `--resolution` | bool | false | Show resolution quality |
| `--repository` | bool | false | Show repository metadata |

## `gm check`

Run trust quality gates for CI.

```sh
gm check [flags]
```

| Flag | Type | Default | Description |
|---|---|---|---|
| `--graph` | path | default output graph | Path to `graph.json` |
| `--min-resolution` | float | 80.0 | Minimum import or call resolution percentage |
| `--max-ambiguous` | integer | 10 | Maximum allowed ambiguous edges |
| `--strict` | bool | false | Fail on warnings |
| `--plan` | string | none | Verify a planning workspace |

Example:

```sh
gm check --graph graphenium-out/graph.json --min-resolution 80 --max-ambiguous 10
```

## `gm diff`

Compare graph snapshots.

```sh
gm diff --before <path> --after <path> [flags]
```

| Flag | Type | Description |
|---|---|---|
| `--before` | path | Before graph snapshot |
| `--after` | path | After graph snapshot |
| `--impact` | bool | Show downstream impact |
| `--review-plan` | bool | Generate verification plan |
| `--json` | bool | Output JSON |

## `gm explain`

Generate pre-edit architectural orientation for a symbol.

```sh
gm explain <symbol> [--graph <path>]
```

Use this when an agent needs context for a target symbol before making changes.

## `gm setup`

Generate MCP configuration for common tools.

```sh
gm setup <target> [--gm-path <path>] [--graph <path>]
```

| Target | Description |
|---|---|
| `claude-desktop` | Claude Desktop config |
| `claude-code` | Claude Code CLI setup |
| `cursor` | Cursor MCP config |
| `codewhale` | CodeWhale MCP config |

## `gm graph`

Graph metadata subcommands.

```sh
gm graph <subcommand> [args]
```

| Subcommand | Description |
|---|---|
| `migrate` | Migrate graph schema version |
| `schema` | Show graph schema version |
| `build-map` | Show build dependency map |
| `test-map` | Show test-to-source map |

## `gm snapshot`

Manage graph snapshots.

```sh
gm snapshot <subcommand> [args]
```

| Subcommand | Description |
|---|---|
| `create <name>` | Create a named snapshot |
| `list` | List snapshots and sizes |

## `gm gate`

Run quality gate workflows over graph diffs.

```sh
gm gate --diff <before> <after>
```

Use this in CI when agent-generated changes require graph-aware review.

## Common workflows

### Build and query

```sh
gm init
gm run . --no-semantic --no-viz
gm doctor --graph graphenium-out/graph.json
gm query "authentication flow" --mode hybrid --budget 3000
```

### CI gate

```sh
gm run . --no-semantic --no-viz
gm check --graph graphenium-out/graph.json --min-resolution 80 --max-ambiguous 10
```

### Incremental development

```sh
gm watch src/ --impact --graph graphenium-out/graph.json
```

### Snapshot and review

```sh
gm snapshot create before-change
# make changes
gm run . --update --no-semantic --no-viz
gm diff --before graphenium-snapshots/before-change.json --after graphenium-out/graph.json --impact --review-plan
```
