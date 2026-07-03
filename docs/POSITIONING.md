# Positioning

## Category

Graphenium should own this lane:

> The local trust and verification layer for AI coding agents.

This is more valuable than the narrower lane of codebase graph, code search, or AI memory.

## Primary message

**Graphenium gives AI coding agents a trusted map of your codebase and a preflight check before they change it.**

## Expanded message

Graphenium builds a local, provenance-aware architecture graph of your repository. AI coding agents use that graph to plan changes, route through source-backed relationships, identify blast radius, and verify edits before they land.

## Why this lane matters

AI agents can now edit real systems, but the main blocker is trust. Teams do not need another search box. They need a way to know whether the agent understands the architecture it is about to change.

Graphenium addresses the trust gap directly:

- It separates facts from guesses.
- It shows what depends on what.
- It tells agents what to read before editing.
- It creates verification plans after edits.
- It can gate changes in CI.

## The strategic shift

Do not lead with the implementation. Lead with the outcome.

| Mechanism-led phrasing | Outcome-led phrasing |
|---|---|
| Persistent architecture graph | Trusted architecture map for AI agents |
| MCP server with 34 tools | Agent workflow layer for safe code changes |
| Louvain communities and graph traversal | Architecture overview in one query |
| Edge confidence metadata | Agents know what is safe to trust |
| Graph diff | Risk-sorted review plan |
| Token reduction | More context left for reasoning and implementation |

## Best one-liners

Use these in the README, website, CLI banner, and launch posts.

1. Graphenium is the local trust and verification layer for AI coding agents.
2. Graphenium gives AI agents a trusted map of your codebase and a preflight check before they change it.
3. Graphenium helps AI coding agents plan, read, edit, verify, and gate structural code changes.
4. Graphenium turns repository structure into source-backed context that agents can safely act on.
5. Graphenium is architecture-aware preflight for AI-generated code changes.

## What to avoid

Avoid leading with phrases that make Graphenium sound like a generic graph database or search tool.

| Avoid | Why |
|---|---|
| Codebase knowledge graph | Sounds broad and crowded |
| AI memory for code | Too vague and passive |
| Better context for coding agents | Every AI tool claims this |
| MCP server for code intelligence | Implementation-led |
| Static analysis tool | Undersells the agent workflow |

## Ideal homepage hero

```text
Graphenium is the trust layer for AI coding agents.

It builds a local, provenance-aware architecture graph of your repository so agents can plan changes, inspect blast radius, follow source-backed paths, and verify edits before they land.
```

Primary call to action:

```text
Build your first graph
```

Secondary call to action:

```text
See agent workflows
```

## Buyer pain

The core problem Graphenium solves:

> We want to use AI agents on real codebases, but we do not trust them enough to make non-trivial changes safely.

Symptoms:

- Agents modify files before understanding dependencies.
- Agents infer call paths from names rather than source evidence.
- Reviewers cannot quickly see blast radius.
- Multi-file refactors become hard to audit.
- CI only checks test results, not whether the agent respected architectural intent.
- Repeated agent sessions waste tokens rediscovering the same structure.

## ICP

Graphenium is strongest for:

- Repositories large enough that agents cannot read everything directly
- Multi-language monorepos
- C# and enterprise systems with build boundaries
- Teams using Claude, Cursor, CodeWhale, or custom agent harnesses
- Platform teams standardizing agent workflows
- Teams that need local-first source handling

## Value propositions by audience

### For AI coding agents

Graphenium tells the agent what to inspect, what to trust, and how to verify the change.

### For reviewers

Graphenium turns a patch into a risk-sorted review plan with dependency paths and impacted files.

### For platform teams

Graphenium creates reusable guardrails for agent adoption across repositories.

### For security-conscious teams

Graphenium runs locally by default and does not send source code to remote services unless semantic extraction is explicitly configured.

### For harness builders

Graphenium provides a repository intelligence substrate that can be embedded into planning, editing, and verification loops.

## Category narrative

```text
First generation: code search helped humans find text.
Second generation: AI assistants helped humans write code.
Third generation: autonomous agents need structural trust before they edit.

Graphenium is built for that third generation.
```

## Positioning test

A strong Graphenium message should answer these questions in ten seconds:

1. Who is it for? AI coding agents and teams adopting them.
2. What does it do? Builds a local, trust-aware architecture graph.
3. Why does it matter? Prevents blind edits and unsafe assumptions.
4. What is unique? Provenance on relationships plus planning, blast radius, verification, and CI gates.
5. Why now? AI coding agents are moving from autocomplete to autonomous multi-file changes.
