# Graphenium Benchmarking Guide

Graphenium is designed to reduce the amount of source code an AI coding agent must load merely to understand repository structure. It should be evaluated on both cost and correctness.

The headline metric is:

> **Tokens to correct change plan**

Do not report token reduction alone. A small answer that produces the wrong plan is not a win.

---

## Benchmark status and claim discipline

The README may include initial self-benchmark numbers from Graphenium's own codebase. Treat those as an early signal, not a universal claim.

Good public claims:

- "On repository X at commit Y, Graphenium reduced navigation tokens from A to B for task T while preserving a correct plan."
- "Graphenium identified N downstream consumers the baseline missed in this benchmark."
- "Graphenium produced a correct first-pass review plan with these files to inspect."

Avoid unsupported claims:

- universal percentage reductions without measured data;
- claims that Graphenium replaces code reading;
- claims of compiler-perfect call graph precision;
- claims based only on Graphenium's own repository;
- token-reduction claims without correctness scoring.

---

## Recommended headline metric

**Tokens to correct change plan** captures both cost and quality. A benchmark should answer:

1. How many tokens did the baseline workflow consume?
2. How many tokens did the Graphenium workflow consume?
3. Did the agent identify the right files, symbols, dependencies, and risks?
4. Did the agent produce a usable change or review plan?
5. What dependencies, tests, or risks did each workflow miss?

---

## Benchmark methodology

Use the same model, same repository, same commit, same task prompt, and same scoring rubric for both workflows.

### Baseline workflow

Allow the agent to use conventional tools only:

- grep / ripgrep;
- file search;
- read file;
- directory listing;
- standard IDE or terminal navigation.

Record:

- files opened;
- total input tokens;
- total output tokens;
- tool calls;
- wall-clock time or time to first useful plan;
- whether the final plan was correct;
- missed dependencies;
- false assumptions;
- whether the agent read irrelevant files.

### Graphenium workflow

Allow the agent to use Graphenium plus direct file reading:

- `architecture_summary`;
- `query_graph`;
- `get_neighbors`;
- `shortest_path`;
- `safest_path`;
- `query_transitive`;
- `blast_radius`;
- `verification_plan`;
- `next_files_to_read`;
- `gm diff --impact`;
- `gm check`.

Record the same metrics as the baseline.

The Graphenium workflow still needs source reading. The benchmark should measure whether the graph reduced unnecessary reading and improved first-pass planning.

---

## Correctness rubric

Score each task on a 0-3 scale.

| Score | Meaning |
|---:|---|
| 0 | Incorrect or unsafe plan; major affected components missed |
| 1 | Partially useful plan; important dependency or risk missing |
| 2 | Correct core plan; minor missing details or unnecessary files |
| 3 | Correct, concise plan with relevant files, dependencies, tests, and risks |

Also record qualitative notes:

- missed downstream consumers;
- false dependencies;
- ambiguous relationships handled correctly or incorrectly;
- tests omitted;
- over-reading or under-reading;
- unsupported assumptions.

A benchmark should report both token counts and correctness score.

---

## Benchmark table template

| Task | Repository | Commit | Baseline tools | Baseline tokens | Baseline score | Graphenium tools | Graphenium tokens | Graphenium score | Reduction | Notes |
|---|---|---|---|---:|---:|---|---:|---:|---:|---|
| Find downstream impact of `TODO_SYMBOL` | TODO | TODO | grep + read files | TODO | TODO | `query_transitive`, `blast_radius` | TODO | TODO | TODO | TODO |
| Explain path between `TODO_A` and `TODO_B` | TODO | TODO | grep + manual tracing | TODO | TODO | `shortest_path`, `safest_path` | TODO | TODO | TODO | TODO |
| Produce review order for changed symbols | TODO | TODO | manual inspection | TODO | TODO | `gm diff --impact` | TODO | TODO | TODO | TODO |
| Orient agent in unfamiliar repo | TODO | TODO | tree/list/readme/grep | TODO | TODO | `architecture_summary`, `god_nodes` | TODO | TODO | TODO | TODO |

---

## Initial self-benchmark example

The following table can be used in the README only if clearly labeled as an initial self-benchmark from Graphenium's own codebase. Replace or supplement it with external repositories before making stronger claims.

Repository: Graphenium's own codebase  
Graph size: 1,061 nodes, 2,104 edges, 22 communities

| Task | Graphenium workflow | Output chars | Tokens, approximate 4 chars/token |
|---|---|---:|---:|
| Impact analysis of `replace_file_extraction` | `query_transitive` / `blast_radius` | 8,677 | ~2,170 |
| Community overview | `query_graph "GrapheniumCluster" --budget 1500` | 6,690 | ~1,670 |
| Module architecture of `GrapheniumGraph` | `query_graph "GrapheniumGraph" --budget 2000` | 8,395 | ~2,100 |
| Symbol with callers/dependents | `query_graph "node_data" --budget 2000` | 8,570 | ~2,140 |
| Cross-module keyword search | `query_graph "authentication flow" --budget 2000` | 8,409 | ~2,100 |
| Server topology | `query_graph "gm serve" --budget 1500` | 6,635 | ~1,660 |

When comparing this to grep + source reading, report the actual measured baseline for the same prompt, not a generic estimate.

---

## Suggested benchmark prompts

### Impact analysis

```text
You are about to change SYMBOL_NAME. Identify downstream impact, the files you
would inspect first, tests likely to be affected, and the highest-risk
dependencies. Produce a change plan but do not edit code.
```

### Architecture path

```text
Explain how MODULE_A connects to MODULE_B. Identify the shortest path, the
safest source-backed path if different, and which files should be read before
making changes.
```

### Review planning

```text
Given this graph diff, produce a risk-sorted review plan. Prioritize removed
symbols, changed dependencies, community moves, ambiguous edges, and high-degree
consumers.
```

### Cold-start orientation

```text
You are entering this repository for the first time. Identify the main modules,
architectural hubs, likely chokepoints, and the first files you would read to
work on FEATURE_NAME.
```

---

## Reporting format

A good benchmark report has this structure:

```text
Repository: NAME
Commit: SHA
Task: PROMPT
Model: MODEL_NAME
Baseline tools allowed: LIST
Graphenium tools allowed: LIST

Baseline:
- input tokens:
- output tokens:
- files opened:
- tool calls:
- correctness score:
- missed dependencies:
- false assumptions:

Graphenium:
- input tokens:
- output tokens:
- files opened:
- tool calls:
- correctness score:
- missed dependencies:
- false assumptions:

Result:
- token delta:
- correctness delta:
- reviewer notes:
```

---

## Automation guidance

A benchmark runner should:

1. reset the repository to a known commit;
2. clear model memory or use fresh sessions;
3. execute the baseline prompt with only baseline tools;
4. execute the Graphenium prompt with Graphenium and file reading;
5. capture tool call logs and token usage;
6. score outputs with a human-authored answer key;
7. export results as JSON and Markdown.

The answer key matters. Without it, a benchmark can only measure verbosity and tool usage, not correctness.
