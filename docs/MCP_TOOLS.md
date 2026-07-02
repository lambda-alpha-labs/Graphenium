# Graphenium MCP Tool Reference

Graphenium exposes compact graph tools through MCP so AI agents can ask structural questions without reading unrelated source files.

The tools are intended to support a disciplined loop:

```text
orient -> resolve target -> inspect trust -> select files -> read source -> plan change -> gate
```

---

## Recommended agent sequence

For architectural or pre-edit tasks:

1. `graph_info` or `architecture_summary`
2. `query_graph` or `analyse_symbol`
3. `get_neighbors`, `query_transitive`, `shortest_path`, or `safest_path`
4. `blast_radius` or `verification_plan`
5. `next_files_to_read`
6. Source-code reading in the selected files
7. `diff_graph`, `what_changed`, or `agent_change_gate` before review

Agents should not treat graph output as a replacement for source reading. Graphenium tells the agent what to read first and how much to trust the path that led there.

---

## Orientation tools

| Tool | Purpose |
|---|---|
| `graph_info` | Full graph metadata: schema version, project root, build timestamp, languages, and counts |
| `graph_stats` | Node/edge counts, file types, confidence and provenance breakdowns |
| `architecture_summary` | Communities, focus paths, god nodes, and confidence summary |
| `god_nodes` | Top N most-connected hub nodes |
| `get_community` | All nodes in a community cluster |
| `recluster` | Re-run community detection after manual node/edge edits |

Use these at the start of an unfamiliar repository or before a broad refactor.

---

## Query and navigation tools

| Tool | Purpose |
|---|---|
| `query_graph` | Keyword-scored BFS/DFS traversal within a token budget |
| `get_node` | Full node details by ID or label |
| `get_neighbors` | Direct neighbours with edge types and confidence |
| `summarize_file` | Extracted symbols from a source file |
| `query_transitive` | BFS transitive closure from a seed symbol with depth, direction, and relation filtering |
| `module_dependencies` | Module-to-module dependency summary between two directory paths |
| `reload_graph` | Hot-swap the graph without restarting the MCP server |
| `run_datalog` | Run a Datalog query against the graph: supports rules (`:-`), goals (`?-`), negation (`not`), and recursion |

Use these to narrow the agent's reading set before it opens source files.

---

## Path and impact tools

| Tool | Purpose |
|---|---|
| `shortest_path` | Path between any two components |
| `safest_path` | Confidence-aware pathfinding between nodes, preferring `EXTRACTED` edges |
| `references_to` | Structural reference lookup — containers, imports, inheritance, implementations (100% AST-only safe) |
| `blast_radius` | Downstream impact analysis for affected files and symbols |
| `verification_plan` | Prioritized verification plan based on impact and risk |
| `next_files_to_read` | Reading-order recommendation derived from a verification plan |
| `what_changed` | Risk-sorted delta against a stored snapshot: removed symbols, community moves, additions |
| `diff_graph` | Snapshot comparison and symbol-level diff between graph versions |
| `analyse_symbol` | Single-turn composite: resolves a symbol and groups behavioral/structural connections with trust profile |

Use these before editing, before review, and when explaining architecture connections.

---

## Trust and policy tools

| Tool | Purpose |
|---|---|
| `resolution_report` | Resolution quality statistics: resolved vs unresolved reference counts and ratios |
| `ambiguous_symbols` | List ambiguous/low-trust edges AND label collisions (same label, different node IDs) |
| `unresolved_references` | Missing dependencies the resolver could not bind |
| `agent_change_gate` | Policy-based gate checks for CI pipelines |

A good agent response should surface trust information explicitly. It should not bury ambiguous relationships in prose.

Example trust profile:

```text
Trust profile: 42 EXTRACTED, 8 INFERRED, 2 AMBIGUOUS
Resolution: 91 percent resolved, 9 percent unresolved
Decision: safe to plan against extracted paths; inspect ambiguous login edge before editing
```

---

## Write tools

| Tool | Purpose |
|---|---|
| `add_node` | Register concepts the AST cannot capture |
| `add_edge` | Record relationships confirmed through inspection |
| `remove_edge` | Correct false positives or stale relationships |

All writes persist to disk immediately.

Use manual writes for confirmed framework behavior, runtime wiring, architecture concepts, or documentation relationships that static extraction cannot see. Do not use manual writes to make unverified guesses look authoritative.

---

## Query behavior worth highlighting

- `summarize_file` supports `show_leaves`; default `false` hides low-degree leaf symbols to save tokens.
- `query_graph` and related tools support `include_tests`; default `false` excludes test/spec nodes.
- `get_neighbors` supports `extracted_only` for strict source-backed traversal.
- Query responses include a trust profile line such as `N EXTRACTED, N INFERRED, N AMBIGUOUS`.
- `shortest_path` output includes per-hop confidence breakdowns.
- Query outputs use project-root-relative paths.

---

## Example MCP agent prompt

```text
You are working in a repository with Graphenium available. Before editing
billing.retry_payment, use Graphenium to:

1. resolve the symbol,
2. identify direct callers and downstream dependents,
3. find the safest source-backed paths to affected modules,
4. list ambiguous or inferred relationships separately,
5. recommend the first files to read,
6. produce a change plan.

Do not edit code until the plan is complete.
```

---

## Example MCP answer shape

```text
Target: billing.retry_payment

Graph summary:
- Direct callers: payments.webhook_handler, jobs.retry_failed_payment
- Downstream dependents: ledger.record_charge, notifications.payment_failed
- Safest path: jobs.retry_failed_payment -> billing.retry_payment -> ledger.record_charge
- Ambiguous edge: payments.webhook_handler -> billing.retry_payment has two candidate targets

Trust profile:
- 31 EXTRACTED
- 6 INFERRED
- 1 AMBIGUOUS

Read first:
1. src/billing/retry.py
2. src/jobs/retry_failed_payment.py
3. src/ledger/charges.py
4. tests/billing/test_retry.py

Plan:
- Inspect retry semantics and idempotency.
- Verify ledger side effects.
- Update tests around failed webhook retries.
- Re-run blast_radius and agent_change_gate before review.
```
