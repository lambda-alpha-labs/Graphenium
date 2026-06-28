# Graphenium Benchmarking Guide

Graphenium is designed to reduce the amount of source code an AI coding agent must load merely to understand repository structure.

This document is a placeholder and template for benchmark data. Replace the TODO values with measured results from real repositories.

## Recommended headline metric

**Tokens to correct change plan**

This captures both cost and quality. A benchmark should answer:

1. How many tokens did the baseline workflow consume?
2. How many tokens did the Graphenium workflow consume?
3. Did the agent identify the right files, symbols, dependencies, and risks?
4. Did the agent produce a usable change or review plan?

## Benchmark methodology

Use the same model, same repository, same task prompt, and same scoring rubric for both workflows.

### Baseline workflow

Allow the agent to use conventional tools only:

- grep / ripgrep
- file search
- read file
- directory listing
- standard IDE or terminal navigation

Record:

- Files opened
- Total input tokens
- Total output tokens
- Tool calls
- Time to first useful plan
- Whether the final plan was correct
- Missed dependencies or false assumptions

### Graphenium workflow

Allow the agent to use Graphenium plus file reading:

- `architecture_summary`
- `query_graph`
- `get_neighbors`
- `shortest_path`
- `safest_path`
- `query_transitive`
- `blast_radius`
- `verification_plan`
- `next_files_to_read`
- `gm diff --impact`
- `gm check`

Record the same metrics.

## Benchmark table

| Task | Repository | Baseline tools | Baseline tokens | Graphenium tools | Graphenium tokens | Reduction | Correct plan? | Notes |
|---|---|---|---:|---|---:|---:|---|---|
| Find downstream impact of `TODO_SYMBOL` | TODO | grep + read files | TODO | `query_transitive`, `blast_radius` | TODO | TODO | TODO | TODO |
| Explain path between `TODO_A` and `TODO_B` | TODO | grep + manual tracing | TODO | `shortest_path`, `safest_path` | TODO | TODO | TODO | TODO |
| Produce review order for changed symbols | TODO | manual inspection | TODO | `gm diff --impact` | TODO | TODO | TODO | TODO |
| Orient agent in unfamiliar repo | TODO | tree/list/readme/grep | TODO | `architecture_summary`, `god_nodes` | TODO | TODO | TODO | TODO |

## Suggested benchmark prompts

### Impact analysis

```text
You are about to change SYMBOL_NAME. Identify the downstream impact, the files you would inspect first, and the highest-risk dependencies.
```

### Architecture path

```text
Explain how MODULE_A connects to MODULE_B. Identify the shortest path, the safest source-backed path if different, and which files should be read before making changes.
```

### Review planning

```text
Given this graph diff, produce a risk-sorted review plan. Prioritize removed symbols, changed dependencies, community moves, and high-degree consumers.
```

## Reporting guidance

Do not report only token reduction. Report correctness and safety.

Good claims:

- Reduced navigation tokens from TODO to TODO on TASK.
- Identified TODO downstream consumers that the baseline missed.
- Produced a correct first-pass review plan with TODO files to inspect.

Avoid unsupported claims:

- Universal percentage reductions without measured data.
- Claims that Graphenium replaces code reading.
- Claims of compiler-perfect call graph precision.

