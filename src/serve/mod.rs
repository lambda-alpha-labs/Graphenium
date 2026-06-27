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
use std::sync::Arc;

use rmcp::{transport::io::stdio, ServiceExt};

// ── Public entry point ────────────────────────────────────────────────────────

/// Load `graph_path` and start the MCP server on stdio.
///
/// Blocks until the client disconnects (stdin closes).
pub async fn serve(graph_path: &Path) -> crate::Result<()> {
    serve_with_watch(graph_path, true).await
}

pub async fn serve_with_watch(graph_path: &Path, watch: bool) -> crate::Result<()> {
    eprintln!("[graphenium] Loading graph: {}", graph_path.display());

    let graph = crate::export::json::load_graph(graph_path)?;
    eprintln!(
        "[graphenium] Graph: {} nodes, {} edges",
        graph.node_count(),
        graph.edge_count()
    );

    let server = GrapheniumServer::with_path(graph, graph_path.to_path_buf());

    if watch {
        let graph_path = graph_path.to_path_buf();
        let srv = server.clone();
        std::thread::spawn(move || {
            use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
            use std::sync::mpsc;
            use std::time::Duration;
            let (tx, rx) = mpsc::channel();
            let mut watcher = match RecommendedWatcher::new(tx, Config::default()) {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("[graphenium] File watcher creation failed: {e}");
                    return;
                }
            };
            if let Err(e) = watcher.watch(&graph_path, RecursiveMode::NonRecursive) {
                eprintln!("[graphenium] File watch failed: {e}");
                return;
            }
            eprintln!(
                "[graphenium] Watching graph file for changes: {}",
                graph_path.display()
            );
            for event in rx {
                if let Ok(Event {
                    kind: EventKind::Modify(_),
                    ..
                }) = event
                {
                    std::thread::sleep(Duration::from_millis(200));
                    if let Err(err) = GrapheniumServer::reload_from_file(&graph_path, &srv) {
                        eprintln!("[graphenium] Failed to reload changed graph: {err}");
                    }
                }
            }
        });
    }

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
