# Graph Report Guide

`graphenium-out/GRAPH_REPORT.md` is a generated summary of a repository graph.

It is useful for humans and agents because it compresses architecture, confidence, hotspots, communities, surprising connections, and knowledge gaps into one reviewable artifact.

## Typical report sections

| Section | Purpose |
|---|---|
| Corpus warnings | Shows detection or scan issues |
| Summary | Shows nodes, edges, communities, hyperedges, and confidence distribution |
| God nodes | Identifies high-degree hubs and likely risk points |
| Surprising connections | Highlights unexpected or high-risk relationships |
| Hyperedges | Shows n-ary relationships if present |
| Communities | Summarizes architectural clusters |
| Ambiguous edges | Lists relationships requiring inspection |
| Knowledge gaps | Shows isolated or missing areas |
| Suggested questions | Gives prompts for architecture review |

## Example summary fields

A Graphenium self-analysis report included:

| Metric | Value |
|---|---:|
| Nodes | 1,061 |
| Edges | 2,104 |
| Communities | 22 |
| Hyperedges | 0 |
| EXTRACTED edges | 1,166 |
| INFERRED edges | 938 |
| AMBIGUOUS edges | 0 |

## How agents should use the report

Agents should use the report to orient, not to replace source reading.

Good uses:

- Identify architectural hubs.
- Find communities before editing.
- Spot surprising cross-boundary links.
- Notice isolated nodes.
- Generate questions for review.
- Decide which files deserve direct inspection.

Bad uses:

- Treat a report as implementation truth.
- Ignore confidence breakdowns.
- Change hub nodes without blast-radius analysis.
- Assume inferred relationships are source-backed.

## Reviewer checklist

When reading a graph report, check:

1. Are corpus warnings present?
2. Is the graph size plausible?
3. Are confidence tiers healthy?
4. Are god nodes expected or surprising?
5. Do communities match the known architecture?
6. Are there surprising cross-boundary edges?
7. Are isolated nodes intentional?
8. Which suggested questions should become review tasks?

## Regenerating the report

```sh
gm run . --no-semantic --no-viz
```

Skip report generation only when you are building graphs for machine-only workflows:

```sh
gm run . --no-report
```
