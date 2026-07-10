---
name: graphenium
description: Use when navigating a Graphenium architecture graph, querying code structure, tracing dependencies, identifying hubs, understanding communities, checking blast radius, or verifying AI-generated code changes.
---

# Graphenium Skill

Graphenium is the local trust and verification layer for AI coding agents.

It builds a provenance-aware architecture graph of the current codebase and exposes it through MCP tools or the `gm query` CLI.

## When to use

Use Graphenium when the user asks about:

- what calls a symbol
- what depends on a symbol
- how two modules connect
- which files to read before editing
- architecture overview
- community or hub nodes
- graph stats
- blast radius
- verification plan
- agent change gates
- ambiguous relationships
- structural questions about a codebase

## First action

Call `graph_info` first when MCP tools are available.

Confirm:

- project root
- schema version
- build timestamp
- extraction mode
- languages
- node and edge counts

If no graph is available, suggest:

```sh
gm run . --no-semantic --no-viz
```

## MCP tool selection

| User asks | Use |
|---|---|
| What does this repo look like? | `graph_info` plus `architecture_summary` |
| What calls X? | `get_neighbors` with relation filter |
| What does X connect to? | `get_neighbors` |
| Tell me about X | `analyse_symbol` or `get_node` |
| What community is X in? | `get_node`, then `get_community` |
| What are the hubs? | `god_nodes` |
| How are A and B connected? | `shortest_path` plus `safest_path` |
| What is in this file? | `summarize_file` |
| Find code related to Y | `query_graph` |
| What is the downstream impact? | `blast_radius` or `query_transitive` |
| Is the graph trustworthy? | `resolution_report` |
| What should I verify after editing? | `verification_plan` |
| Should this change pass policy? | `agent_change_gate` |
| What changed since a snapshot? | `what_changed` or `diff_graph` |
| Need a custom constraint query | `run_datalog` |

## Datalog Queries (Advanced Pathfinding)

Graphenium features a pre-loaded Datalog Standard Library. **Do not write recursive rules manually**; instead, use the pre-loaded predicates:

- **Check if a component bypasses the service layer:**
  `?- bypasses_layer('auth_controller', 'auth_service', 'auth_repository').`
- **Find circular dependencies in a module:**
  `?- circular_dependency(X, Y), node(X, _, _, 'src/parser/mod.rs', _).`
- **Find all transitively called dependencies of a target:**
  `?- calls_transitive('auth_service', X).`

Available stdlib predicates: `calls_transitive`, `imports_transitive`, `depends_transitive`, `same_community`, `is_hub`, `is_orphan`, `circular_dependency`, `bypasses_layer`.

Base EDB relations (always available): `node`, `calls`, `imports`, `contains`, `inherits`, `implements`, `degree`, `edge`.

## Trust model

Every edge carries confidence.

| Confidence | Meaning | Behavior |
|---|---|---|
| `EXTRACTED` | Source-backed or manually verified through source inspection | Safe planning backbone |
| `INFERRED` | Strong lead but not ground truth | Verify before acting |
| `AMBIGUOUS` | Uncertain or multiple targets | Inspect source before acting |

Always disclose trust quality when it affects the answer.

## Agent behavior rules

Do:

- query the graph before editing unfamiliar or high-impact code
- prefer source-backed paths
- identify ambiguous relationships
- read implementation files before editing
- compute blast radius after changes
- produce a verification plan before review

Do not:

- treat graph output as a substitute for source reading
- hide ambiguous relationships
- claim compiler-perfect precision unless the extractor supports it
- write guessed relationships into the graph
- act on inferred edges without verification for high-risk changes

## Write-back rules

Only use write tools after inspection.

| Situation | Tool | Confidence |
|---|---|---|
| Confirmed relationship in source | `add_edge` | `EXTRACTED` |
| Confirmed architectural concept | `add_node` | appropriate provenance |
| Found false positive | `remove_edge` | not applicable |
| Made meaningful manual edits | `recluster` | not applicable |

Do not add an edge based on naming, file proximity, or assumption alone.

## CLI fallback

When MCP tools are unavailable, use:

```sh
gm query "<question>" --budget 2000
```

Useful flags:

| Flag | Purpose |
|---|---|
| `--mode hybrid` | Combine lexical and structural ranking |
| `--safe` | Prefer confidence-aware traversal |
| `--depth N` | Control traversal depth |
| `--path-prefix P` | Scope to a module or directory |
| `--exclude-path P` | Remove noisy paths |
| `--include-tests` | Include test nodes |
| `--graph path` | Use a non-default graph file |
| `--datalog` | Run declarative graph queries |

Examples:

```sh
gm query "authentication flow" --mode hybrid --budget 3000
gm query "process_batch" --path-prefix parser --safe --budget 3000
gm query --datalog "?- calls(X, Y, _)."
```

## Standard response pattern

When using Graphenium for a change, respond with:

```text
Graph loaded:
Target resolved:
Trust profile:
Source-backed path:
Inferred leads:
Ambiguous relationships:
First files to read:
Blast radius:
Verification plan:
Recommended next step:
```
