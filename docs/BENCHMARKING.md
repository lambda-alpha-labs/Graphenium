# Benchmarking

Graphenium should be benchmarked by how quickly and reliably it helps an agent produce a correct change plan, not by token reduction alone.

The core metric is:

> tokens-to-correct-plan

## Why measure tokens

LLM context is expensive. Every irrelevant file read competes with reasoning, implementation, and review context.

Graphenium reduces navigation cost by giving agents compact structural answers instead of forcing them to read many raw files.

The current documented heuristic is approximately 4 characters per token for Graphenium ASCII output.

## Self-analysis baseline

Graphenium's own repository report documents:

| Metric | Value |
|---|---|
| Nodes | 1,061 |
| Edges | 2,104 |
| Communities | 22 |
| EXTRACTED edges | 1,166 |
| INFERRED edges | 938 |
| AMBIGUOUS edges | 0 |

Typical self-analysis query output has been measured around 6,600 to 8,700 characters, or roughly 1,600 to 2,200 tokens.

## Example benchmark table

| Query | Output chars | Approx tokens | Time |
|---|---:|---:|---:|
| `replace_file_extraction` impact analysis | 8,674 | about 2,170 | 27 ms |
| `GrapheniumCluster` community overview | 6,690 | about 1,670 | 18 ms |
| `GrapheniumGraph` module architecture | 8,395 | about 2,100 | 22 ms |
| `node_data` symbol with callers and dependents | 8,570 | about 2,140 | 20 ms |
| `authentication flow` cross-module keyword | 8,409 | about 2,100 | 19 ms |
| `gm serve` server topology | 6,635 | about 1,660 | 15 ms |

These numbers are useful as a baseline, not as a universal guarantee.

## Benchmark methodology

1. Rebuild the graph.
2. Run a realistic query.
3. Record character count.
4. Estimate token count.
5. Record latency.
6. Judge whether the output contained the structural facts needed for the task.
7. Compare against the raw files an agent would otherwise need to read.

```sh
gm run . --no-semantic --no-viz
gm query "authentication flow" --budget 2000
```

Automated script:

```sh
chmod +x scripts/run_benchmarks.sh
./scripts/run_benchmarks.sh
./scripts/run_benchmarks.sh --json
```

## What good looks like

| Signal | Good range | Why |
|---|---|---|
| Output size | under 10,000 chars | Small enough for common agent workflows |
| Latency | under 50 ms for typical queries | Feels local and interactive |
| Structural completeness | enough to plan next read | Avoids blind file browsing |
| Confidence visibility | trust profile included | Prevents false certainty |
| Actionability | first files to read are clear | Moves the agent toward source inspection |

## Concerning signals

| Signal | What it may mean | Action |
|---|---|---|
| Over 20,000 chars | Query too broad or graph too dense | Tighten keywords, path scope, relation filters, or budget |
| Query over 500 ms | Repository or graph is very large | Exclude vendored dirs and generated code |
| Missing expected symbols | Unsupported language feature or ignore issue | Check `.grapheniumignore`, extractor support, semantic pass |
| Too many inferred edges | Static extraction cannot see enough | Inspect manually or improve extractor |
| Too many ambiguous edges | Label collisions or dynamic patterns | Use `get_node` disambiguation and source reads |

## Task-quality scoring

A benchmark should score whether Graphenium helped the agent create a better plan.

| Score | Meaning |
|---|---|
| 0 | Output was irrelevant |
| 1 | Output found some names but no useful structure |
| 2 | Output found related files but not enough dependencies |
| 3 | Output supported a reasonable first source read |
| 4 | Output supported a clear pre-edit plan |
| 5 | Output supported plan, trust profile, blast radius, and verification steps |

## Recommended benchmark tasks

Use tasks that represent real agent work:

- Modify a public API.
- Change an authentication or authorization path.
- Refactor a service boundary.
- Remove a symbol with downstream callers.
- Move logic between modules.
- Change a C# project reference.
- Add a new integration test target.
- Explain why two modules are connected.

## Compare against alternatives

For each task, compare:

1. Raw file reads only
2. grep or ripgrep navigation
3. IDE symbol navigation
4. Graphenium query plus targeted source reads

Measure:

- files read
- tokens consumed
- time to usable plan
- correctness of dependency understanding
- missed downstream consumers
- reviewer effort

## Benchmark principle

A smaller answer is not automatically better. The goal is the smallest answer that still lets the agent make a correct, trustworthy next move.
