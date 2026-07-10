# Trust Model

Graphenium is built around a simple idea: AI coding agents must know the difference between facts, leads, and uncertainty.

## Trust levels

| Confidence | Meaning | Agent behavior |
|---|---|---|
| `EXTRACTED` | Source-backed or manually verified through source inspection | Safe to plan against, then read source before editing |
| `INFERRED` | Likely relationship from semantic extraction or heuristic evidence | Treat as a lead and verify |
| `AMBIGUOUS` | Multiple possible targets or uncertain relationship | Do not act until inspected |

## Resolution status

| Status | Meaning | Agent behavior |
|---|---|---|
| `resolved` | Target exists in the graph | More trustworthy |
| `unresolved` | Target was not found | Investigate missing files, dynamic code, generated code, or unsupported patterns |
| `heuristic` | Linked by heuristic rather than direct resolution | Verify before acting |

## Extractor provenance

The `extractor` field explains where an edge came from.

| Extractor | Typical trust profile |
|---|---|
| `tree-sitter` | Source-backed syntax extraction |
| `resolver` | Source-backed cross-file resolution when resolved |
| `tree-sitter-stack-graphs` | Resolver-derived cross-file relationship |
| `csproj-parser` | C# build boundary extraction |
| `llm` | Semantic inference, verify before high-risk edits |
| manual write tools | Trust depends on whether the agent actually inspected source |

## Practical trust policy

```text
Use EXTRACTED + resolved edges as the planning backbone.
Use INFERRED edges as hypotheses.
Use AMBIGUOUS edges as review questions.
Use unresolved edges as graph quality gaps.
```

## What agents should say

Good agent output:

```text
The direct call path is source-backed through EXTRACTED edges. One downstream consumer is INFERRED, so I will inspect that file before editing. There are two AMBIGUOUS edges involving validate_token, so I will not assume which target is used until I read the source.
```

Bad agent output:

```text
Graphenium says validate_token is used by AccountController, so I will update it now.
```

Why bad: it hides confidence, avoids source inspection, and treats the graph as compiler truth.

## Trust-first planning template

```text
Target:
Resolved node:
Source files:
Trust profile:
  EXTRACTED:
  INFERRED:
  AMBIGUOUS:
Unresolved references:
Safest path:
Highest-risk consumers:
First files to read:
Assumptions to verify:
Change plan:
Post-edit verification:
```

## When to block an agent

Block or slow down the agent when:

- the target symbol is ambiguous
- the safest path contains only inferred evidence
- downstream impact includes high-degree public nodes
- many new ambiguous edges appear after the change
- graph health is too weak for the intended change
- the agent has not read the recommended source files
- the change modifies files outside the declared planning workspace

## CI policy examples

Start permissive:

```sh
gm check --min-resolution 50 --max-ambiguous 50
```

Move to moderate:

```sh
gm check --min-resolution 70 --max-ambiguous 25
```

Stricter policy:

```sh
gm check --min-resolution 80 --max-ambiguous 10 --strict
```

Do not over-tighten before the graph is mature. Good policy evolves with extractor coverage and repository conventions.

## Manual edge policy

AI-added edges should be rare and evidence-backed.

| Situation | Write to graph? | Confidence |
|---|---|---|
| Agent read source and confirmed call | Yes | `EXTRACTED` |
| Agent inferred based on naming | No | none |
| Agent saw framework convention in docs and source | Yes, with provenance | `EXTRACTED` or documented manual confidence |
| Agent is unsure | No | none |

## Trust anti-patterns

Avoid these behaviors:

- treating all graph edges equally
- letting agents write guessed relationships
- using token reduction as the only metric
- ignoring ambiguous symbols because they are inconvenient
- claiming full correctness without compiler-backed extraction
- using stale graphs after major edits

When `graph_info` reports **Graph may be stale**, the loaded graph predates recent source changes or a newer `gm` binary. The server still answers queries, but structural results may be incomplete. Rebuild with `gm run . --no-semantic --no-viz`, then call `reload_graph` to hot-swap without restarting MCP. See `docs/MCP_TOOLS.md` for `graph_info` and `reload_graph` details.

## Review checklist

Before accepting an agent-generated change, ask:

1. Did the agent call `graph_info`?
2. Did it resolve the target symbol?
3. Did it distinguish extracted, inferred, and ambiguous edges?
4. Did it read the recommended files?
5. Did it compute blast radius?
6. Did it produce a verification plan?
7. Did the gate pass or explain failures?
8. Did it avoid unplanned files?
