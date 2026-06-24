/// MCP server for the Graphenium knowledge graph.
///
/// Exposes 8 tools over the stdio JSON-RPC transport:
/// - `query_graph`   — BFS/DFS traversal from keyword-matched seed nodes
/// - `get_node`      — Full node details by ID or label
/// - `get_neighbors` — Direct neighbors + edge details, optional relation filter
/// - `get_community` — All nodes in a community by ID
/// - `god_nodes`     — Top N most-connected hub nodes
/// - `graph_stats`   — Summary statistics
/// - `architecture_summary` — Repo-level structural highlights
/// - `shortest_path` — Path search between two nodes
pub mod handlers;
pub mod traversal;

pub use handlers::GrapheniumServer;

use std::path::Path;

use rmcp::{transport::io::stdio, ServiceExt};

// ── Public entry point ────────────────────────────────────────────────────────

/// Load `graph_path` and start the MCP server on stdio.
///
/// Blocks until the client disconnects (stdin closes).
pub async fn serve(graph_path: &Path) -> crate::Result<()> {
    eprintln!("[graphenium] Loading graph: {}", graph_path.display());

    let graph = crate::export::json::load_graph(graph_path)?;
    eprintln!(
        "[graphenium] Graph: {} nodes, {} edges",
        graph.node_count(),
        graph.edge_count()
    );

    let server = GrapheniumServer::with_path(graph, graph_path.to_path_buf());
    eprintln!("[graphenium] MCP server starting on stdio...");

    let service = server
        .serve(stdio())
        .await
        .map_err(|e: std::io::Error| crate::GrapheniumError::Serve(e.to_string()))?;

    eprintln!("[graphenium] MCP server ready (waiting for client).");

    service
        .waiting()
        .await
        .map_err(|e| crate::GrapheniumError::Serve(e.to_string()))?;

    Ok(())
}
