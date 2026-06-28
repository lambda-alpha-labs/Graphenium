# Graphenium Command Reference

This document contains the detailed CLI reference moved out of the top-level README to keep the front page focused on positioning, quick start, and common workflows.

## `gm init`

Initialize a Graphenium workspace with a default `.grapheniumignore` file.

```text
gm init [PATH]
```

Examples:

```sh
gm init
gm init ~/projects/my-repo
```

## `gm run`

Run the full analysis pipeline on a directory.

```text
gm run [PATH] [OPTIONS]
```

| Option | Description |
|---|---|
| `PATH` | Directory to analyze, default `.` |
| `--no-semantic` | Skip LLM extraction and use local structural results |
| `--no-viz` | Skip HTML generation |
| `--provider NAME` | AI provider: `anthropic`, `openai`, `openrouter`, `deepseek`, or `openai-compatible` |
| `--model NAME` | Model to use, defaults to provider-specific default |
| `--api-key KEY` | API key, overrides provider-specific env var |
| `--api-base URL` | API base URL for `openai-compatible` provider |
| `--mode deep` | Aggressive LLM inference |
| `--update` | Incremental mode: only re-extract changed files |
| `--no-report` | Skip `GRAPH_REPORT.md` generation |
| `--exclude-dirs DIRS` | Comma-separated directory names to exclude, such as `target,node_modules` |

Examples:

```sh
gm run . --no-semantic --no-viz
gm run . --provider openai
gm run . --update
gm run . --no-report
```

## `gm query`

Query an existing graph using lexical, structural, or hybrid retrieval.

```text
gm query "<keywords>" [OPTIONS]
```

| Option | Default | Description |
|---|---:|---|
| `--graph PATH` | `graphenium-out/graph.json` | Path to graph file |
| `--budget N` | `2000` | Output token budget |
| `--mode MODE` | `lexical` | Retrieval model: `lexical`, `structural`, or `hybrid` |
| `--dfs` | off | Use depth-first traversal |
| `--safe` | off | Confidence-aware traversal; skips `AMBIGUOUS` edges |
| `--min-degree N` | `0` | Minimum node degree to include |
| `--exclude-test-nodes` | off | Exclude test/spec nodes from results |

Examples:

```sh
gm query "parser ast walker" --safe
gm query "authentication login" --mode lexical
gm query "database connection" --mode structural
gm query "parser ast walker" --mode hybrid
```

## `gm serve`

Start an MCP server exposing the graph over stdio.

```text
gm serve [OPTIONS]
```

| Option | Default | Description |
|---|---:|---|
| `--graph PATH` | `graphenium-out/graph.json` | Path to graph file |
| `--watch` | off | Watch graph file for changes and auto-reload |

## `gm watch`

Watch a directory and auto-rebuild the graph on changes.

```text
gm watch [PATH] [OPTIONS]
```

| Option | Default | Description |
|---|---:|---|
| `PATH` | `.` | Directory to watch |
| `--debounce SECS` | `3.0` | Wait after last event before rebuild |
| `--incremental` | `true` | Patch changed files. Use `false` for full rebuilds |

Example:

```sh
gm watch . --debounce 2.0
```

## `gm doctor`

Run diagnostic checks on the Graphenium installation and graph health.

```text
gm doctor [--graph PATH]
gm doctor --schema
gm doctor --resolution
gm doctor --repository
gm doctor --json
```

Use cases:

- `--schema`: dump graph schema, node kinds, edge kinds, confidence levels, and provenance metadata.
- `--resolution`: report resolved vs unresolved references.
- `--repository`: summarize repository metadata.
- `--json`: emit machine-readable diagnostics for IDE extensions or CI.

## `gm check`

Run trust-quality gates for local development or CI.

```text
gm check [OPTIONS]
```

| Option | Default | Description |
|---|---:|---|
| `--graph PATH` | `graphenium-out/graph.json` | Path to graph file |
| `--min-resolution N` | `80` | Minimum accepted resolution coverage percentage |
| `--max-ambiguous N` | `10` | Maximum allowed ambiguous edge count |

Examples:

```sh
gm check
gm check --min-resolution 80 --max-ambiguous 10
gm check --min-resolution 70 --max-ambiguous 20 --strict
```

## `gm diff`

Diff two graph snapshots and show symbol-level changes.

```text
gm diff [OPTIONS]
```

| Option | Default | Description |
|---|---:|---|
| `--before PATH` | empty graph | Path to the old `graph.json` |
| `--after PATH` | `graphenium-out/graph.json` | Path to the new `graph.json` |
| `--impact` | off | Show downstream impact analysis and review order |
| `--review-plan` | off | Generate a prioritized verification plan |

Examples:

```sh
gm diff --before old-graph.json --after new-graph.json
gm diff --after new-graph.json --impact
gm diff --before old-graph.json --after new-graph.json --review-plan
```

## `gm setup`

Print ready-to-paste MCP config for an AI assistant.

```text
gm setup <claude|cursor|codewhale> [--graph PATH]
```

Examples:

```sh
gm setup claude
gm setup cursor
gm setup codewhale
```

## `gm graph`

Inspect repository metadata and CI extraction results.

```text
gm graph schema [--graph PATH]
gm graph build-map [--graph PATH]
gm graph test-map [--graph PATH]
gm graph migrate <graph.json>
```

Examples:

```sh
gm graph schema
gm graph build-map
gm graph test-map
```

## `gm snapshot`

Manage graph snapshots for diff and drift analysis.

```text
gm snapshot create --name <name> [--graph PATH]
gm snapshot list
```

## `gm gate`

Run quality gates with diff-based analysis.

```text
gm gate --diff <before.json> <after.json>
```

