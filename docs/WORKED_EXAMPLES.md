# Worked Examples

Worked examples show what Graphenium does in real repositories. They are valuable because they show both strengths and misses.

## What a good worked example includes

- repository name
- language or languages
- Graphenium mode
- node count
- edge count
- community count
- most useful queries
- what Graphenium got right
- what Graphenium missed
- how the graph changed the agent workflow
- whether the author would use it again

## Why worked examples matter

Graphenium should not be judged only by internal benchmarks. Real examples reveal:

- whether the graph finds the right files
- whether trust levels are useful
- whether ambiguous edges are understandable
- whether reviewers get better impact summaries
- whether agents produce safer plans

## Current example area

The existing worked examples area includes Graphenium self-analysis, with an AST-only analysis of Graphenium's own Rust codebase.

## Suggested example categories

| Category | Why useful |
|---|---|
| Small Python API | Shows fast onboarding and feature tracing |
| Large C# solution | Shows assembly boundaries and cross-file resolution |
| TypeScript frontend | Shows component and utility coupling |
| Rust CLI | Shows module structure and test mapping |
| Mixed monorepo | Shows language-family guardrails |
| Research codebase | Shows paper/document linking |

## Evaluation prompts

Use these prompts when creating examples:

```text
Use Graphenium to understand the architecture before changing the authentication flow.
```

```text
Use Graphenium to find the blast radius of removing SYMBOL.
```

```text
Use Graphenium to compare the graph before and after this patch and generate a review plan.
```

```text
Use Graphenium to identify ambiguous relationships that a human reviewer should inspect.
```

## Success criteria

A worked example is strong when a reader can answer:

1. What did Graphenium reveal quickly?
2. What did it miss?
3. Which graph outputs were trustworthy?
4. Which outputs required source inspection?
5. How did it change the agent's plan?
6. Would the same workflow help another team?
