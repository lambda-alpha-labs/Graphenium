# Graphenium Benchmarking

## Approach

The right metric for a code-graph tool is **tokens to correct change plan**, not
query speed or token reduction alone. The table below measures the end-to-end
cost of answering four common structural questions using Graphenium against
Graphenium's own codebase.

**Benchmark repository:** `lambda-alpha-labs/Graphenium` at commit `7794607`.
**Graph:** AST-only (no semantic/LLM pass), 1,061 nodes, 2,104 edges, 22 communities.
**Schema:** 0.2.0. Graph file: 980KB.
**Binary:** `gm 0.8.0`. Query response time: 23-31ms per query.

---

## Results: Graphenium queries on Graphenium itself

| Task | Graphenium workflow | Output chars | Tokens (~4 c/t) | Response time |
|---|---|---|---|---:|---:|
| Impact analysis of `replace_file_extraction` | `query_transitive` + `blast_radius` | 8,674 | ~2,170 | 27ms |
| Community and cluster overview | `query_graph "GrapheniumCluster" --budget 1500` | 6,690 | ~1,670 | 31ms |
| Module architecture of `GrapheniumGraph` | `query_graph "GrapheniumGraph" --budget 2000` | 8,395 | ~2,100 | 24ms |
| Symbol with callers/dependents | `query_graph "node_data" --budget 2000` | 8,570 | ~2,140 | 25ms |
| Cross-module keyword search | `query_graph "authentication flow" --budget 2000` | 8,409 | ~2,100 | 31ms |
| Server topology | `query_graph "gm serve" --budget 1500` | 6,635 | ~1,660 | 23ms |

**Comparison to baseline (grep + manual tracing):** answering "what calls this
function?" via grep requires opening and reading 5-12 files and their import
chains, typically consuming 30,000-50,000 characters (~8,000-12,000 tokens) —
a **4-6x token reduction** with Graphenium.

---

## How to reproduce

```sh
# Build the graph
gm run . --no-semantic --no-viz

# Run the queries
gm query "replace_file_extraction" --budget 2000 --mode hybrid
gm query "GrapheniumCluster" --budget 1500
gm query "GrapheniumGraph" --budget 2000
gm query "node_data" --budget 2000
gm query "authentication flow" --budget 2000
gm query "gm serve" --budget 1500

# Get graph statistics
gm doctor
```

Character counts can be captured with `wc -c`. Timing with `time gm query ...`.

The automated benchmark runner at `scripts/run_benchmarks.sh` runs these
queries and asserts character-count budgets.

---

## Correctness notes

These benchmarks measure **output size**, not correctness. On Graphenium's
own codebase, manual verification confirmed:

- All callers of `replace_file_extraction` were identified (3 call sites)
- The path `GrapheniumServer → GrapheniumGraph` resolves in 2 hops
- No false positives in the "authentication flow" cross-module query
- Trust gate (`gm check --min-resolution 80 --max-ambiguous 10`) passes
  on the AST-only graph (100% effective on import resolution, N/A on calls)

---

## Known limits

- These benchmarks are on a single codebase (Graphenium itself). Results will
  vary by repository size, language mix, and project structure.
- AST-only extraction is used. Semantic (LLM-enriched) graphs will have
  different edge profiles and token costs.
- Characters-to-tokens ratio (4:1) is a rough approximation for English prose.
  Code-heavy or structured output will differ.
