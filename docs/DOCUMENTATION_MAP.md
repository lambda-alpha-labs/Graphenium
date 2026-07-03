# Documentation Map

This pack restructures the Graphenium documentation around the strongest product lane: **the local trust and verification layer for AI coding agents**.

It preserves the original documentation coverage while improving the message, flow, and user journey.

## Coverage summary

| Original documentation area | Covered by |
|---|---|
| `README.md` | `README.md`, `docs/POSITIONING.md`, `docs/GETTING_STARTED.md` |
| `docs/GETTING_STARTED.md` | `docs/GETTING_STARTED.md`, `docs/AI_SETUP.md` |
| `docs/COMMAND_REFERENCE.md` | `docs/COMMAND_REFERENCE.md` |
| `docs/MCP_TOOLS.md` | `docs/MCP_TOOLS.md`, `skills/graphenium/SKILL.md` |
| `docs/AGENT_WORKFLOWS.md` | `docs/AGENT_WORKFLOWS.md`, `docs/TRUST_MODEL.md`, `docs/CI_AND_GOVERNANCE.md` |
| `docs/ARCHITECTURE.md` | `docs/ARCHITECTURE.md`, `docs/TRUST_MODEL.md` |
| `docs/COMPARISON.md` | `docs/COMPARISON.md`, `docs/POSITIONING.md` |
| `docs/BENCHMARKING.md` | `docs/BENCHMARKING.md` |
| `AI_SETUP.md` | `docs/AI_SETUP.md` |
| `contrib/harness-adapter/README.md` | `docs/HARNESS_ADAPTER.md`, `contrib/harness-adapter/README.md` |
| `skills/graphenium/SKILL.md` | `skills/graphenium/SKILL.md` |
| `CONTRIBUTING.md` | `docs/CONTRIBUTING.md` |
| `SECURITY.md` | `docs/SECURITY.md` |
| `CODE_OF_CONDUCT.md` | `docs/CODE_OF_CONDUCT.md` |
| `CHANGELOG.md` | `docs/CHANGELOG.md` |
| `LICENSE` | `docs/LICENSE.md` |
| `worked/README.md` | `worked/README.md`, `docs/WORKED_EXAMPLES.md` |
| `worked/TEMPLATE.md` | `worked/TEMPLATE.md` |
| `graphenium-out/GRAPH_REPORT.md` | `docs/GRAPH_REPORT.md` |

## Recommended repository placement

This documentation pack is designed to drop into the repository with minimal friction.

```text
README.md
AI_SETUP.md                       optional copy from docs/AI_SETUP.md
CHANGELOG.md                      optional copy from docs/CHANGELOG.md
CONTRIBUTING.md                   optional copy from docs/CONTRIBUTING.md
SECURITY.md                       optional copy from docs/SECURITY.md
CODE_OF_CONDUCT.md                optional copy from docs/CODE_OF_CONDUCT.md
docs/
  DOCUMENTATION_MAP.md
  POSITIONING.md
  GETTING_STARTED.md
  AGENT_WORKFLOWS.md
  COMMAND_REFERENCE.md
  MCP_TOOLS.md
  ARCHITECTURE.md
  TRUST_MODEL.md
  CI_AND_GOVERNANCE.md
  BENCHMARKING.md
  COMPARISON.md
  AI_SETUP.md
  HARNESS_ADAPTER.md
  CONTRIBUTING.md
  SECURITY.md
  CHANGELOG.md
  CODE_OF_CONDUCT.md
  LICENSE.md
  WORKED_EXAMPLES.md
  GRAPH_REPORT.md
worked/
  README.md
  TEMPLATE.md
skills/
  graphenium/
    SKILL.md
contrib/
  harness-adapter/
    README.md
```

## Message architecture

The original documentation already had many strong technical ideas. This pack makes the message more direct:

| Old center of gravity | New center of gravity |
|---|---|
| Persistent architecture graph | Trust and verification layer for AI-generated code changes |
| Codebase memory | Source-backed architecture map |
| Token reduction | Tokens-to-correct-plan |
| MCP tool catalog | Agent operating system for safe code modification |
| Search and traversal | Plan, read, edit, verify, gate |

## Primary audience

Graphenium is useful for individual developers, but the highest-value audience is:

- DevEx and platform teams rolling out coding agents
- Engineering leads responsible for large, high-change-cost repositories
- AI coding harness builders
- Teams that want to review and gate agent-generated changes
- Regulated or security-conscious teams that need local-first operation

## Core claim

**Graphenium lets teams use AI coding agents without surrendering architectural control.**
