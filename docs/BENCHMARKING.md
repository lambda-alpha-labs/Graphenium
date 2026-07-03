# Benchmarking

## Why measure tokens

LLM context windows are expensive. Every file read costs tokens that could be spent on reasoning and implementation. Graphenium's value is reducing the tokens needed for structural understanding.

**Token-chars ratio**: Graphenium's ASCII output averages ~4 characters per token. Use this to estimate your LLM cost per query.

## Self-analysis results

Benchmarks on Graphenium's own codebase (1061 nodes, 2104 edges, 22 communities) using `target/release/gm`:

| Query | Output chars | Tokens (~4 chars/token) | Time (ms) |
|---|---|---|---|
| `replace_file_extraction`: impact analysis | 8,674 | ~2,170 | 27 |
| `GrapheniumCluster`: community overview | 6,690 | ~1,670 | 18 |
| `GrapheniumGraph`: module architecture | 8,395 | ~2,100 | 22 |
| `node_data`: symbol with callers/dependents | 8,570 | ~2,140 | 20 |
| `authentication flow`: cross-module keyword | 8,409 | ~2,100 | 19 |
| `gm serve`: server topology | 6,635 | ~1,660 | 15 |

Typical queries return **6,600–8,700 chars** (~1,600–2,200 tokens): a 4-6x token reduction vs reading the raw source files that would be needed for the same structural understanding.

## Methodology

```
1. Run gm run . --no-semantic --no-viz to rebuild the graph
2. Run gm query with the target query and budget
3. Record output character count, timing, and whether the answer
   contained the expected structural information
```

Use `scripts/run_benchmarks.sh` for automated benchmarking:

```sh
chmod +x scripts/run_benchmarks.sh
./scripts/run_benchmarks.sh           # console output
./scripts/run_benchmarks.sh --json    # JSON output to benchmark_results.json
```

## Interpreting results

Good benchmarks show:
- **<10,000 chars per query** (manageable by most LLMs)
- **<50ms query time** (not network-bound; all computation is local)
- **All needed structural info present** (the graph is complete enough for the task)

Concerning signals:
- **>20,000 chars per query**: the graph may be too dense; increase specific keywords
- **Query time >500ms**: the graph may be too large; consider excluding vendored directories
- **Missing structural info**: extraction may need semantic pass or more languages covered

## Performance optimizations

### Brandes' betweenness centrality (O(V·E))

The `betweenness_centrality` implementation in `src/analyze/questions.rs` uses Brandes' O(V·E) algorithm, safely capped at the first 5,000 nodes per community. On graphs under 5,000 nodes, the computation runs in milliseconds. On larger codebases, the cap ensures analysis completes in bounded time while still identifying structural bridge nodes (chokepoints) in the most significant architectural communities.

### Salsa-backed incremental extraction

The `src/cache/query.rs` module provides Salsa-powered demand-driven incremental computation. Extraction results are memoized by content hash, so unchanged files skip tree-sitter parsing entirely on subsequent rebuilds. After an initial full scan, `gm run .` on a project where only a few files changed completes in near-constant time: only the delta is re-extracted.

### Token-reduction benchmarks

Graphenium's ASCII output averages approximately 4 characters per token. Typical queries return 6,600 to 8,700 characters (approximately 1,600 to 2,200 tokens), a 4-6x reduction compared to reading the raw source files needed for equivalent structural understanding.

## Token reduction vs task completion

Graphenium optimizes for **tokens-to-correct-plan**, not token reduction alone. A smaller output that lacks needed information is worse than a larger one that's correct. Always verify that the query result contains the structural information needed for the change.

## How to run your own benchmarks

1. Build the graph: `gm run . --no-semantic --no-viz`
2. Run benchmark script: `scripts/run_benchmarks.sh`
3. For custom queries, use: `gm query "<your question>" --budget <chars>`
4. Record: query, output chars, timing, and whether the result was sufficient
5. Compare iterations to track improvements
