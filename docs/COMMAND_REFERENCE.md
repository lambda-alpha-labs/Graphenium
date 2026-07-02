# Graphenium Command Reference

This reference keeps the front-page README focused on the main value proposition while documenting the CLI in detail.

Start with three commands:

```sh
gm run . --no-semantic
gm setup claude
gm query "symbol or feature" --safe
```

Then add health checks, diffing, watch mode, and CI gates as your workflow matures.

---

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

Use this before the first run so generated files, build output, and vendored dependencies can be excluded.

---

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
gm run . --exclude-dirs target,node_modules,dist
```

Recommended first run:

```sh
gm run . --no-semantic --no-viz
```

---

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
| `--datalog` | string | Run a Datalog query instead of keyword search (e.g. `--datalog "?- node(X, _, _, _, _)."`) |
| `--safe` | off | Confidence-aware traversal; skips `AMBIGUOUS` edges |
| `--min-degree N` | `0` | Minimum node degree to include |
| `--exclude-test-nodes` | off | Exclude test/spec nodes from results |

Examples:

```sh
gm query "parser ast walker" --safe
gm query "authentication login" --mode lexical
gm query "database connection" --mode structural
gm query "parser ast walker" --mode hybrid
gm query "billing retry" --safe --budget 1000
```

Use `--safe` for pre-edit planning. Use `--mode hybrid` when naming may be inconsistent or the feature spans multiple modules.

---

## `gm serve`

Start an MCP server exposing the graph over stdio.

```text
gm serve [OPTIONS]
```

| Option | Default | Description |
|---|---:|---|
| `--graph PATH` | `graphenium-out/graph.json` | Path to graph file |
| `--watch` | off | Watch graph file for changes and auto-reload |

Example:

```sh
gm serve --graph /absolute/path/to/graphenium-out/graph.json
```

Use `gm setup` when possible so you do not need to hand-write MCP config.

---

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
gm setup claude --graph /absolute/path/to/graphenium-out/graph.json
```

After changing MCP configuration, fully restart the client application.

---

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

Use watch mode during active agent sessions so MCP queries see recent graph updates.

---

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

| Option | Use case |
|---|---|
| `--schema` | Dump graph schema, node kinds, edge kinds, confidence levels, and provenance metadata |
| `--resolution` | Report resolved vs unresolved references |
| `--repository` | Summarize repository metadata |
| `--json` | Emit machine-readable diagnostics for IDE extensions or CI |

Examples:

```sh
gm doctor --resolution
gm doctor --repository
gm doctor --json
```

---

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
| `--strict` | off | Use stricter policy behavior when available |

Examples:

```sh
gm check
gm check --min-resolution 80 --max-ambiguous 10
gm check --min-resolution 70 --max-ambiguous 20 --strict
```

Policy advice: start permissive, collect data, then tighten thresholds once graph extraction is stable.

---

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

Use `--impact` when reviewing agent edits to high-degree modules or public interfaces.

---

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
gm graph migrate old-graph.json
```

---

## `gm snapshot`

Manage graph snapshots for diff and drift analysis.

```text
gm snapshot create --name <name> [--graph PATH]
gm snapshot list
```

Examples:

```sh
gm snapshot create --name before-auth-refactor
gm snapshot list
```

Snapshot before major agent-led changes so review planning can compare graph state.

---

## `gm gate`

Run quality gates with diff-based analysis.

```text
gm gate --diff <before.json> <after.json>
```

Example:

```sh
gm gate --diff old-graph.json graphenium-out/graph.json
```

Use this in CI to prevent graph quality regressions or risky unverified topology changes.

---

## Command selection guide

| Goal | Command |
|---|---|
| First setup | `gm init` |
| Build graph | `gm run . --no-semantic` |
| Ask a graph question | `gm query "keywords" --safe` |
| Connect agent | `gm setup claude` or `gm serve` |
| Keep graph fresh while coding | `gm watch` |
| Inspect health | `gm doctor --resolution` |
| Enforce quality | `gm check` |
| Review changed topology | `gm diff --impact` |
| Create a comparison point | `gm snapshot create` |
| Run CI diff gate | `gm gate --diff` |
