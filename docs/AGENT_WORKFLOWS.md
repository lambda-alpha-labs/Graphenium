# Agent Workflows

Graphenium is designed for the moments before and after an AI coding agent changes code.

The core operating rule is simple:

> Query the graph before editing. Read the source before committing. Verify impact after editing.

## Workflow 1: Pre-edit safety plan

Use this when an agent is about to modify a function, class, module, endpoint, schema, dependency, route, build target, or public API.

### Agent prompt

```text
Use Graphenium before editing. Resolve TARGET_SYMBOL, identify direct callers, downstream consumers, safest source-backed paths, ambiguous relationships, and the first files to read. Produce a change plan before modifying code.
```

### Recommended tool sequence

1. `graph_info`
2. `analyse_symbol` or `query_graph`
3. `get_neighbors` with `extracted_only` when strictness matters
4. `query_transitive` for downstream or upstream closure
5. `safest_path` for confidence-aware paths
6. `blast_radius`
7. `next_files_to_read`
8. Source-code reading in the recommended files
9. Change plan

### Expected output shape

```text
Target resolved: auth.validate_token
Trust profile: 28 EXTRACTED, 5 INFERRED, 3 AMBIGUOUS
Highest-risk dependents: routes.account, middleware.require_session
Read first: src/auth/session.py, src/middleware/session.py, tests/auth/test_session.py
Ambiguity: AccountController.login has two possible validate_token targets
Plan: update token validation, then verify session middleware and account-route tests
```

## Workflow 2: Architecture orientation

Use this when an agent enters an unfamiliar repository.

### Agent prompt

```text
Use Graphenium to summarize the repository architecture. Identify communities, hub nodes, chokepoints, confidence quality, and the most useful files to read for orientation. Do not claim implementation behavior until you inspect source.
```

### Recommended tool sequence

1. `graph_info`
2. `architecture_summary`
3. `god_nodes`
4. `get_community` for the largest or most relevant clusters
5. `next_files_to_read` or `query_graph` for a specific feature

### Good output behavior

The agent should say what the graph shows and what still needs source inspection.

```text
The graph shows three major communities: API, domain, and storage. The API-to-domain path is mostly EXTRACTED. The storage community has several INFERRED edges, so I would inspect src/storage before changing persistence behavior.
```

## Workflow 3: Path explanation

Use this when the agent needs to explain how two components connect.

### Agent prompt

```text
Use Graphenium to explain how COMPONENT_A connects to COMPONENT_B. Show the shortest path, the safest source-backed path if different, per-hop confidence, and the files to read before making changes.
```

### Recommended tool sequence

1. `shortest_path`
2. `safest_path`
3. `get_node` for ambiguous or high-degree nodes
4. `next_files_to_read`

### Trust rule

Shortest is not always safest.

```text
Shortest path uses one inferred edge. Safest path is longer but fully extracted. Use the safest path for change planning. Inspect the inferred edge only as a lead.
```

## Workflow 4: Review planning after a change

Use this when a pull request or agent patch changes architecture, public APIs, dependencies, high-degree symbols, or multi-file behavior.

### Agent prompt

```text
Use Graphenium to compare the previous and current graph. Produce a risk-sorted review plan. Prioritize removed symbols, changed dependencies, community moves, high-degree consumers, ambiguous new edges, and tests to inspect.
```

### Recommended tool sequence

1. `diff_graph` or `gm diff --impact`
2. `what_changed`
3. `blast_radius`
4. `verification_plan`
5. `agent_change_gate`

### Expected output shape

```text
Review priority 1: removed public symbol Parser.parse_file, 11 downstream consumers
Review priority 2: new inferred edge from CLI to resolver, verify manually
Review priority 3: community move for GrapheniumGraph, inspect module boundary
Tests: parser integration tests, CLI query tests, resolver fixtures
Gate: failed max ambiguous threshold by 2 edges
```

## Workflow 5: Design, plan, and verify

Use this when an agent is about to implement a multi-file architectural change.

### Agent prompt

```text
Use Graphenium to create a planning workspace for the proposed change. Declare the intended symbols and relationships before writing code. After implementation, verify compliance with verification_plan and get_plan_details, then report implemented, missing, and unplanned symbols.
```

### Recommended tool sequence

1. `create_planning_workspace`
2. `graph_info` plus `get_neighbors`
3. `add_planned_symbol` for each new or modified symbol
4. Implement the code
5. `get_plan_details` plus `verification_plan`
6. `blast_radius`
7. `agent_change_gate`

### Expected output shape

```text
Plan: refactor-auth-service
Implemented nodes: new_auth_service, token_provider_adapter (2 of 3)
Missing nodes: session_manager
Unplanned modified files: src/middleware/unrelated.rs
Compliance: 2 of 3 planned symbols implemented. 1 unplanned file touched.
Review priority: verify whether session_manager is intentionally deferred.
```

## Workflow 6: CI trust gate

Use this when a team wants agent-generated changes to meet a minimum graph-quality bar.

```sh
gm check --graph graphenium-out/graph.json --min-resolution 80 --max-ambiguous 10
```

Diff-based gate:

```sh
gm gate --diff old-graph.json graphenium-out/graph.json
```

Good CI policy starts permissive and tightens over time. Do not fail builds on unrealistic precision until extractor coverage, ignore rules, and manual graph corrections are mature.

## Workflow 7: Manual graph correction

Use this when source extraction cannot capture a relationship that is important for agent planning.

Examples:

- Framework convention links a route to a handler.
- Runtime dependency injection connects a service to an implementation.
- Documentation explains an architectural decision.
- A generated client is intentionally excluded from indexing.

Recommended tools:

1. `add_node` for concepts or external systems
2. `add_edge` for confirmed relationships
3. `remove_edge` for false positives
4. `recluster` after meaningful manual edits
5. `agent_change_gate` after correction

Manual writes should be rare and evidence-backed.

## Agent behavior rules

A Graphenium-aware coding agent should:

- call `graph_info` before relying on the graph
- query the graph before editing unfamiliar or high-impact code
- prefer `EXTRACTED` relationships for change plans
- treat `INFERRED` relationships as leads
- stop and inspect source for `AMBIGUOUS` relationships
- read implementation files before finalizing a patch
- run impact and gate checks before asking for review
- disclose trust limitations in its plan

A Graphenium-aware coding agent should not:

- replace source reading with graph output
- hide ambiguous facts
- use token reduction as the only success metric
- make universal benchmark claims from one repository
- treat semantic extraction as source-backed unless provenance says so
- write graph edges that it has not verified through source inspection

## One prompt to use everywhere

```text
Use Graphenium as the trust layer for this change. Start with graph_info, use source-backed paths where possible, identify ambiguous relationships, read the recommended source files, then produce a plan. After editing, compute blast radius and generate a verification plan before review.
```
