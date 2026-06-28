# Updated Graphenium Documentation Set

This zip contains a tightened documentation set for Graphenium based on the recommended positioning changes.

## Included files

| File | Purpose |
|---|---|
| `README.md` | Updated front-page documentation with sharper positioning, a clearer pre-edit workflow, guarded benchmark language, simplified quick start, and revised differentiation. |
| `docs/GETTING_STARTED.md` | New guided setup and first agent workflow. |
| `docs/AGENT_WORKFLOWS.md` | New prompt and workflow guide for pre-edit planning, architecture orientation, path explanation, review planning, CI gates, and manual graph correction. |
| `docs/COMMAND_REFERENCE.md` | Updated command reference with a three-command adoption path up front and the full CLI reference preserved. |
| `docs/MCP_TOOLS.md` | Updated MCP tool reference organized around agent workflows and trust-aware usage. |
| `docs/BENCHMARKING.md` | Updated benchmark methodology with stronger claim discipline, correctness rubric, and a clearly labeled self-benchmark section. |
| `docs/COMPARISON.md` | Updated positioning against grep, vector search, repo maps, generic MCP graphs, and enterprise indexers. |
| `docs/ARCHITECTURE.md` | Updated architecture document with evidence, confidence model, data pipeline, and limitations. |
| `docs/DEMO_SCRIPT.md` | New demo copy and terminal storyboard showing a concrete blast-radius query. |

## Main changes made

- Reframed Graphenium around "before your agent edits code, give it a map it can trust."
- Made the primary wedge pre-edit safety: dependency paths, blast radius, trust profile, and files to read first.
- Simplified the README quick start and moved deeper CLI detail into the command reference.
- Made confidence and provenance more visible in agent-facing examples.
- Added benchmark guardrails so token-reduction claims require correctness scoring and measured baselines.
- Added concrete agent prompts and answer shapes to make the MCP value clearer.
- Added a demo script with a specific query/result moment instead of abstract product copy only.

## Suggested repository layout

```text
README.md
docs/
  GETTING_STARTED.md
  AGENT_WORKFLOWS.md
  COMMAND_REFERENCE.md
  MCP_TOOLS.md
  BENCHMARKING.md
  COMPARISON.md
  ARCHITECTURE.md
  DEMO_SCRIPT.md
```
