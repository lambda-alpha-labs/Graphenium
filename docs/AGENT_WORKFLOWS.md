# Agent Workflows

Graphenium is designed for the moments before and after an AI coding agent changes code.

The agent should use the graph to narrow attention, evaluate trust, and identify the source files that deserve direct reading.

---

## Workflow 1: Pre-edit safety plan

Use this when an agent is about to modify a function, class, module, endpoint, schema, or dependency.

Agent prompt:

```text
Use Graphenium before editing. Resolve TARGET_SYMBOL, identify direct callers,
downstream consumers, safest source-backed paths, ambiguous relationships, and
the first files to read. Produce a change plan before modifying code.
```

Recommended tool sequence:

1. `analyse_symbol` or `query_graph`
2. `get_neighbors` with `extracted_only` when strictness matters
3. `query_transitive` for downstream or upstream closure
4. `safest_path` for confidence-aware paths
5. `blast_radius`
6. `next_files_to_read`
7. source-code reading in the recommended files
8. change plan

Expected output:

```text
Target resolved: auth.validate_token
Trust profile: 28 EXTRACTED, 5 INFERRED, 3 AMBIGUOUS
Highest-risk dependents: routes.account, middleware.require_session
Read first: src/auth/session.py, src/middleware/session.py, tests/auth/test_session.py
Ambiguity: AccountController.login has two possible validate_token targets
Plan: update token validation, then verify session middleware and account-route tests
```

---

## Workflow 2: Architecture orientation

Use this when an agent enters an unfamiliar repository.

Agent prompt:

```text
Use Graphenium to summarize the repository architecture. Identify communities,
hub nodes, chokepoints, confidence quality, and the most useful files to read
for orientation.
```

Recommended tool sequence:

1. `graph_info`
2. `architecture_summary`
3. `god_nodes`
4. `get_community` for the largest or most relevant clusters
5. `next_files_to_read` or `query_graph` for a specific feature

Expected output should avoid pretending to know implementation behavior before reading source. It should say what the graph shows and what still needs inspection.

---

## Workflow 3: Path explanation

Use this when the agent needs to explain how two components connect.

Agent prompt:

```text
Use Graphenium to explain how COMPONENT_A connects to COMPONENT_B. Show the
shortest path, the safest source-backed path if different, per-hop confidence,
and the files to read before making changes.
```

Recommended tool sequence:

1. `shortest_path`
2. `safest_path`
3. `get_node` for ambiguous or high-degree nodes
4. `next_files_to_read`

Output should distinguish convenience from trust:

```text
Shortest path uses one inferred edge. Safest path is longer but fully extracted.
Use the safest path for change planning; inspect the inferred edge only as a lead.
```

---

## Workflow 4: Review planning after a change

Use this when a pull request or agent patch changes architecture, public APIs, dependencies, or high-degree symbols.

Agent prompt:

```text
Use Graphenium to compare the previous and current graph. Produce a risk-sorted
review plan. Prioritize removed symbols, changed dependencies, community moves,
high-degree consumers, ambiguous new edges, and tests to inspect.
```

Recommended tool sequence:

1. `diff_graph` or `gm diff --impact`
2. `what_changed`
3. `blast_radius`
4. `verification_plan`
5. `agent_change_gate`

Expected output:

```text
Review priority 1: removed public symbol Parser.parse_file, 11 downstream consumers
Review priority 2: new inferred edge from CLI to resolver, verify manually
Review priority 3: community move for GrapheniumGraph, inspect module boundary
Tests: parser integration tests, CLI query tests, resolver fixtures
Gate: failed max ambiguous threshold by 2 edges
```

---

## Workflow 5: Design, Plan, and Verify (The Sandbox Loop)

Use this when an agent is about to implement a multi-file architectural change.

Agent prompt:

```text
Use Graphenium to create a planning workspace for the proposed change.
Declare the intended symbols and relationships before writing any code.
After implementation, verify compliance with verification_plan and
get_plan_details, then report implemented, missing, and unplanned symbols.
```

Recommended tool sequence:

1. `create_planning_workspace` — create a virtual workspace for the change
2. `graph_info` + `get_neighbors` — understand current architecture
3. `add_planned_symbol` for each new or modified symbol
4. Implement the code
5. `get_plan_details` + `verification_plan` — audit planned vs actual implementation
6. `blast_radius` — downstream impact of the completed change
7. `agent_change_gate` — trust quality gates before requesting review

Expected output:

```text
Plan: refactor-auth-service
Implemented nodes: new_auth_service, token_provider_adapter (2/3)
Missing nodes: session_manager (not yet implemented)
Unplanned modified files: src/middleware/unrelated.rs (1 unexpected change)
Compliance: 2/3 planned symbols implemented. 1 unplanned file touched.
Review priority: verify session_manager is intentionally deferred.
```

---

## Workflow 6: CI trust gate

Use this when a repository wants agent changes to meet a minimum graph-quality bar.

Local command:

```sh
gm check --min-resolution 80 --max-ambiguous 10
```

Diff-based command:

```sh
gm gate --diff old-graph.json graphenium-out/graph.json
```

Good CI policy starts permissive and tightens over time. A team should not fail builds on unrealistic precision until extractor coverage, ignore rules, and manual graph corrections are mature.

---

## Workflow 7: Manual graph correction

Use this when source extraction cannot capture a relationship that is still important for agents.

Examples:

- framework convention links a route to a handler;
- runtime dependency injection connects a service to an implementation;
- documentation explains an architectural decision;
- a generated client is intentionally excluded from indexing.

Recommended tools:

1. `add_node` for concepts or external systems
2. `add_edge` for confirmed relationships
3. `remove_edge` for false positives
4. `recluster` after meaningful manual edits
5. `agent_change_gate` after correction

Manual writes should be used sparingly and with clear provenance.

---

## Agent behavior rules

A Graphenium-aware coding agent should:

- query the graph before editing unfamiliar or high-impact code;
- prefer `EXTRACTED` relationships for change plans;
- treat `INFERRED` relationships as leads;
- stop and inspect source for `AMBIGUOUS` relationships;
- read implementation files before finalizing a patch;
- run impact and gate checks before asking for review;
- avoid claiming compiler-perfect precision unless backed by compiler-derived extraction.

A Graphenium-aware coding agent should not:

- replace source reading with graph output;
- hide ambiguous facts;
- use token reduction as the only success metric;
- make universal benchmark claims from a single repository;
- treat semantic extraction as source-backed unless the provenance says so.
