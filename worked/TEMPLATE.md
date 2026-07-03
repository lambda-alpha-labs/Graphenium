# Worked Example: <Repository Name>

## Repository

| Field | Value |
|---|---|
| Repository |  |
| URL |  |
| Language or languages |  |
| Graphenium version |  |
| Graphenium mode | AST-only or semantic |
| Date analyzed |  |

## Graph summary

| Metric | Value |
|---|---:|
| Nodes |  |
| Edges |  |
| Communities |  |
| EXTRACTED edges |  |
| INFERRED edges |  |
| AMBIGUOUS edges |  |

## Setup commands

```sh
gm init
gm run . --no-semantic --no-viz
gm doctor --graph graphenium-out/graph.json
```

## Most useful queries

```sh
gm query "<feature or symbol>" --mode hybrid --budget 3000
gm query "<symbol>" --safe --depth 2 --budget 1500
gm diff --before <before.json> --after <after.json> --impact --review-plan
```

## What Graphenium got right

- 
- 
- 

## What Graphenium missed

- 
- 
- 

## Trust observations

- Which edges were source-backed?
- Which edges were inferred?
- Which edges were ambiguous?
- What required direct source inspection?

## Agent workflow impact

Describe how Graphenium changed the agent's plan, file reads, verification steps, or review quality.

## Would I use this again?

Answer yes, no, or conditional. Explain why.

## Notes for maintainers

List extractor gaps, documentation gaps, or benchmark ideas revealed by this example.
