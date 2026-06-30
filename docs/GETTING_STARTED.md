# Getting Started with Graphenium

This guide gets you from an unindexed repository to a useful agent workflow.

The minimum valuable loop is:

```text
gm run -> gm query or MCP query -> read recommended files -> plan change -> gm check
```

Graphenium is most useful before an agent edits code. The agent should first use the graph to identify dependencies, source-backed paths, ambiguous facts, and the first files to inspect.

---

## 1. Install Graphenium

Use the installer:

```sh
curl -fsSL https://raw.githubusercontent.com/lambda-alpha-labs/Graphenium/main/install.sh | sh
```

Or build from source:

```sh
git clone https://github.com/lambda-alpha-labs/Graphenium
cd Graphenium
cargo install --path .
```

Confirm the binary is available:

```sh
gm --help
```

---

## 2. Initialize the repository

From the repository root:

```sh
gm init
```

This creates safe ignore defaults so the graph does not waste time on vendored dependencies, generated artifacts, or build output.

Review `.grapheniumignore` before running on large monorepos.

---

## 3. Build a local structural graph

No API key is needed for structural extraction:

```sh
gm run . --no-semantic --no-viz
```

This writes output into `graphenium-out/`.

Start here even if you plan to use semantic extraction later. The structural graph is the baseline that makes confidence and provenance useful.

---

## 4. Ask your first graph question

Use a query related to a real change you might make:

```sh
gm query "authentication login session" --safe --budget 1000
```

Use `--safe` when you want confidence-aware traversal that avoids ambiguous edges.

Good first queries:

```sh
gm query "database connection" --mode hybrid --budget 1200
gm query "parser ast walker" --safe --budget 1500
gm query "payment retry" --mode structural --budget 1000
```

The answer should orient you toward relevant modules, symbols, and paths. It is not a substitute for reading implementation details.

---

## 5. Inspect graph health

Check resolution quality:

```sh
gm doctor --resolution
```

Run a trust gate:

```sh
gm check --min-resolution 80 --max-ambiguous 10
```

Treat poor resolution or high ambiguity as a signal to inspect configuration, add ignore rules, improve extraction, or manually confirm critical relationships.

---

## 6. Connect an agent through MCP

Generate config for your assistant:

```sh
gm setup claude
gm setup cursor
gm setup codewhale
```

Then fully restart the assistant application.

Manual MCP server command:

```sh
gm serve --graph /absolute/path/to/graphenium-out/graph.json
```

---

## 7. Use the first safe-change prompt

Give your agent an explicit pre-edit instruction:

```text
Before changing SYMBOL_NAME, use Graphenium to identify downstream impact,
safest source-backed paths, ambiguous relationships, and the first files to
read. Produce a change plan before editing.
```

A good agent answer should include:

- the target symbol or module it resolved;
- a trust profile with counts for `EXTRACTED`, `INFERRED`, and `AMBIGUOUS` facts;
- source-backed paths or direct neighbors;
- the first files to read;
- tests or CI jobs likely to be affected;
- risks and ambiguous facts that require source inspection.

---

## 8. After changes, use diff and gates

Create or compare snapshots when reviewing changes:

```sh
gm diff --before old-graph.json --after graphenium-out/graph.json --impact
gm diff --before old-graph.json --after graphenium-out/graph.json --review-plan
gm check --min-resolution 80 --max-ambiguous 10
```

In CI, start with a permissive threshold and tighten it as extractor coverage improves.

---

## Common first-run issues

### The graph is too noisy

Add generated folders, vendor directories, and build artifacts to `.grapheniumignore`. Then rerun:

```sh
gm run . --no-semantic --no-viz
```

### Too many ambiguous edges

Use `gm doctor --resolution` and the MCP `ambiguous_symbols` tool. Improve resolver configuration, add manual graph writes for critical relationships, or lower trust gate strictness temporarily.

### The agent still reads too many files

Ask it to use `next_files_to_read`, `safest_path`, or `verification_plan` before opening source. The goal is not zero reading; the goal is prioritized reading.

### Semantic extraction is expensive

Run structural-only first. Use semantic extraction selectively for architecture concepts, rationale links, dynamic framework behavior, or code paths the AST cannot capture.

---

## What to do next

Read:

- [`AGENT_WORKFLOWS.md`](AGENT_WORKFLOWS.md) for prompts and MCP usage patterns.
- [`MCP_TOOLS.md`](MCP_TOOLS.md) for the complete agent tool surface.
- [`BENCHMARKING.md`](BENCHMARKING.md) before making public token-reduction claims.
- [`COMMAND_REFERENCE.md`](COMMAND_REFERENCE.md) when wiring Graphenium into CI.
