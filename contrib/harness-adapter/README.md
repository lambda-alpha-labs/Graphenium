# Graphenium Harness Adapter

Reference integration showing how to embed the Graphenium knowledge graph
engine inside an AI coding harness. The adapter provides lifecycle functions
that a harness calls at specific moments: file open, edge discovery,
periodic clustering, to build and maintain the graph ambiently during
normal AI work.

## Adding the dependency

```toml
[dependencies]
graphenium = { git = "https://github.com/lambda-alpha-labs/Graphenium", default-features = false, features = ["harness"] }
```

The `harness` feature excludes the MCP server and watch-mode dependencies,
giving you a lean dependency tree. Enable specific language features
(`lang-python`, `lang-rust`, etc.) for the languages you need, or enable
the default feature set for all 9.

## Lifecycle

```
Workspace open
  └─ initialize_graph(root)       ← full AST scan, Louvain clustering, snapshot to disk

File opened / saved
  └─ on_file_open(graph, path)    ← re-extract one file, patch graph

AI discovers a relationship
  └─ on_edge_discovered(g, ...)   ← add EXTRACTED edge (AI verified by inspection)

AI finds a wrong edge
  └─ on_edge_invalid(g, ...)      ← remove the bad edge

Periodic (every N files or on timer)
  └─ refresh_communities(g, ...)  ← re-cluster, snapshot to disk

MCP server needs current data
  └─ snapshot_to_disk(g, path)    ← atomic JSON write
```

## Serving the graph

Once the graph is built and snapshotted to disk, any MCP client can
connect to it. The simplest approach is running `gm serve` as a sidecar:

```sh
gm serve --graph /path/to/graphenium-out/graph.json
```

The 12 tools (9 read + 3 write) are available to any connected AI
assistant. The `add_node`, `add_edge`, and `remove_edge` tools persist
to disk immediately, so edges added via MCP survive across sessions.

## Minimal harness integration (pseudocode)

```rust
use graphenium_harness_adapter::*;
use std::path::Path;

// On workspace open
let (mut graph, communities) = initialize_graph(Path::new("/path/to/project"));
snapshot_to_disk(&graph, Path::new("/path/to/project/graphenium-out/graph.json"))?;

// On file open
let stats = on_file_open(&mut graph, Path::new("src/auth/service.rs"));
if stats.nodes_replaced > 0 {
    // Enough changed; refresh communities and snapshot
    let stats = refresh_communities(&mut graph, &ClusterOptions::default());
    snapshot_to_disk(&graph, Path::new(".../graph.json"))?;
}

// AI discovered a relationship through code inspection
on_edge_discovered(&mut graph, "auth_service", "token_provider", "delegates_to", "src/auth/service.rs");
snapshot_to_disk(&graph, Path::new(".../graph.json"))?;

// AI found a false positive
on_edge_invalid(&mut graph, "auth_service", "unrelated_fn", Some("imports"));
```

## Confidence model (important)

When the AI adds edges through the harness adapter, it uses `EXTRACTED`
confidence; this is the same tier as tree-sitter edges. The AI confirmed
the relationship through actual code inspection. This is fundamentally
different from batch Claude API inference (`INFERRED`/`AMBIGUOUS` edges).

The harness should **not** automatically add edges the AI hasn't verified.
If the AI reads two files and traces a call chain, it should call
`on_edge_discovered`. If it merely suspects a relationship based on naming
conventions, it should not write the edge.

## Testing

```sh
cd contrib/harness-adapter
cargo test
```
