# Graphenium Harness Adapter

This adapter shows how to embed Graphenium inside an AI coding harness.

The harness keeps the graph current as the agent works, then exposes that graph to MCP clients or internal agent workflows.

## Install as a dependency

```toml
[dependencies]
graphenium = { git = "https://github.com/lambda-alpha-labs/Graphenium", default-features = false, features = ["harness"] }
```

Enable language features as needed, such as `lang-rust`, `lang-python`, or `lang-csharp`.

## Lifecycle

```text
Workspace open
  -> initialize_graph(root)
  -> snapshot_to_disk(graph)

File opened or saved
  -> on_file_open(graph, path)
  -> refresh_communities when needed
  -> snapshot_to_disk(graph)

AI verifies a relationship
  -> on_edge_discovered(graph, source, target, relation, file)

AI invalidates a relationship
  -> on_edge_invalid(graph, source, target, relation)

MCP sidecar serves current graph
  -> gm serve --graph graphenium-out/graph.json
```

## Planning integration

Use planning workspaces for multi-file agent changes.

1. Create a plan.
2. Add planned symbols.
3. Let the agent edit code.
4. Rebuild or patch the graph.
5. Verify planned symbols against extracted symbols.
6. Report implemented, missing, and unplanned work.

## Confidence policy

Only write edges that the AI verified through source inspection.

Do not write relationships based on naming conventions alone.

| Evidence | Write? |
|---|---|
| Source was inspected and relationship is visible | Yes |
| Relationship is only suspected | No |
| Relationship comes from generated code that was excluded | Add only if verified and documented |
| Relationship is uncertain | No |

## Test

```sh
cd contrib/harness-adapter
cargo test
```

## Full guide

See `docs/HARNESS_ADAPTER.md` for the expanded integration guide.
