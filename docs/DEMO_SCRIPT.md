# Graphenium Demo Script

This document provides a concise demo narrative for the README, terminal demos, GIFs, and launch material.

---

## Hero copy

```text
Graphenium: Provenance-aware repo memory for AI agents

Most code tools help humans search files.
Graphenium gives AI agents a durable, queryable map of your repo.

Before your agent edits code, give it a map it can trust.
```

---

## Concrete demo moment

Use one specific question instead of only abstract positioning:

```text
agent: What is the blast radius of changing auth.validate_token?

graphenium: 17 downstream symbols, 4 high-risk paths, 3 ambiguous edges.
Safest path: auth.validate_token -> auth.require_session -> routes.account.
Read first: src/auth/session.py, src/routes/account.py, tests/auth/test_session.py.
Trust profile: 28 EXTRACTED, 5 INFERRED, 3 AMBIGUOUS.
```

This makes the value visible: the agent gets impact, trust, and file-reading order before opening source.

---

## 60-second terminal storyboard

### Frame 1: Problem

```text
Large repo. Cold agent session. Limited context.

Without a map, the agent has to grep, open files, trace imports,
and rebuild the same mental model every time.
```

### Frame 2: Build graph

```sh
gm init
gm run . --no-semantic --no-viz
```

Overlay:

```text
Graphenium builds a local graph of files, symbols, dependencies, tests, and CI jobs.
Every relationship carries confidence and provenance.
```

### Frame 3: Ask pre-edit question

```sh
gm query "auth validate token session" --safe --budget 1000
```

Overlay:

```text
The agent asks structural questions before reading source.
```

### Frame 4: Trust-aware answer

```text
Target: auth.validate_token
Direct callers: require_session, refresh_session
Downstream: account routes, billing routes, websocket auth
Trust profile: 28 EXTRACTED, 5 INFERRED, 3 AMBIGUOUS
Read first: src/auth/session.py, src/middleware/session.py, tests/auth/test_session.py
```

Overlay:

```text
The agent sees what is source-backed, what is inferred, and what needs inspection.
```

### Frame 5: Gate before review

```sh
gm diff --before old-graph.json --after graphenium-out/graph.json --impact
gm check --min-resolution 80 --max-ambiguous 10
```

Overlay:

```text
After the change, Graphenium produces a review plan and checks graph quality.
```

---

## Short README demo block

```text
Before editing:
  agent -> Graphenium -> blast radius, safest paths, ambiguous facts, files to read

After editing:
  agent -> Graphenium -> graph diff, review plan, trust gate
```

---

## Landing page sections

1. Headline: "Before your agent edits code, give it a map it can trust."
2. Problem: agents waste tokens rediscovering repo architecture.
3. Solution: durable, queryable graph with confidence and provenance.
4. Demo: blast radius of one symbol, including trust profile and files to read.
5. Workflow: build graph, query graph, read source, plan change, gate.
6. Differentiation: not grep, not semantic search, not just a repo map.
7. Proof: benchmark protocol and initial self-benchmark clearly labeled.
8. Integration: MCP for Claude, Cursor, CodeWhale.

---

## Do and do not

Do say:

```text
Graphenium helps agents spend fewer tokens on navigation and more context on reasoning.
```

Do say:

```text
Graphenium separates extracted, inferred, and ambiguous relationships.
```

Do not say:

```text
Graphenium replaces source reading.
```

Do not say:

```text
Graphenium is compiler-perfect across all languages.
```

Do not say:

```text
Graphenium always reduces tokens by a fixed percentage.
```
