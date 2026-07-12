# Graphenium Embedded Harness Adapter

This adapter serves as a reference implementation for programmatically embedding Graphenium's structural containment engine inside an AI coding harness or custom developer platform.

By compiling and embedding Graphenium as a dependency, your harness can maintain a **real-time, AST-proven index of the codebase** and enforce strict pre-flight policy gates as an agent opens, saves, and modifies files.

---

## 1. Installation

To embed the structural engine as a dependency, declare it in your `Cargo.toml`. Enable the `harness` feature flag to keep the dependency tree lightweight (this excludes background MCP server and watch-mode process dependencies):

```toml
[dependencies]
graphenium = { git = "https://github.com/lambda-alpha-labs/Graphenium", default-features = false, features = ["harness", "lang-rust", "lang-python"] }
```

*Enable compilation support for only the target programming languages your platform needs to govern.*

---

## 2. Integrated Lifecycle Loop

The harness adapter allows your platform to synchronize Graphenium's structural index with IDE or runtime events:

```text
Workspace Opened
  └── initialize_graph(root)         # Compile baseline AST/Stack Graphs index
  └── snapshot_to_disk(index)        # Persist compiled index file

File Opened or Saved
  └── on_file_open(index, path)      # Incrementally patch changed files
  └── refresh_communities()          # Re-cluster structural folder domains
  └── snapshot_to_disk(index)        # Update persisted index state

Agent Proposes Design Spec
  └── validate_plan_preflight()      # Run pre-flight Datalog layer analysis

Agent Implements Edits
  └── on_file_open(index, path)      # Re-extract actual physical changes
  └── verify_plan()                  # Audit compliance (flag scope creep)
```

---

## 3. Pre-Flight Design Integration

To prevent AI assistants from writing messy, unapproved code, integrate Graphenium's "Design-then-Verify" lifecycle directly into your agent's execution loop:

1.  **Initialize Planning Workspace:** When the agent begins a task, create a virtual workspace (`create_planning_workspace`).
2.  **Declare Intent:** Instruct the agent to register its planned classes, methods, and module dependencies (`add_planned_symbol`).
3.  **Pre-Flight Policy Check:** Evaluate the virtual plan against the repository's `.graphenium/policy.json` rules using `validate_plan_preflight`.
4.  **Enforce Safe Coding:** If Graphenium identifies strict layering bypasses or unauthorized imports, block execution and feed the structural violations back to the agent for redesign.
5.  **Post-Edit Compliance Audit:** Once the agent implements the code, re-index the modified files and execute `verify_plan` to verify that the agent did not commit scope creep (modifying files outside the declared plan) or leak unapproved dependencies.

---

## 4. Strict Confidence Policies

To maintain the integrity of the codebase index, your harness must enforce a strict contract when allowing agents to write manual relationship edits:

| Agent Evidence | Write to Index? | Target Confidence |
|---|---|---|
| Agent read source and confirmed dependency | Yes | `EXTRACTED` |
| Agent verified relationship via local tests | Yes | `EXTRACTED` |
| Agent assumes relationship based on file proximity | **No** | *Blocked* |
| Agent is uncertain or guessing | **No** | *Blocked* |

*Never let agents promote heuristic guesswork or semantic inferences to `EXTRACTED` confidence. This preserves Graphenium's role as a source-backed compile-time truth engine.*

---

## 5. Running the Adapter Tests

Graphenium includes a suite of integration tests verifying index patching, edge discovery, and deletion:

```sh
cd contrib/harness-adapter
cargo test
```

---

## 6. Comprehensive Integration Playbook

For a detailed walkthrough on API parameters, memory structures, and performance bounds, refer to the full adapter guide at [`docs/HARNESS_ADAPTER.md`](../../docs/HARNESS_ADAPTER.md).
