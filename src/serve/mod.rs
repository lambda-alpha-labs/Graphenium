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
    let (graph, watch_path, watch_parent) = match crate::export::json::load_graph(graph_path) {
        Ok(g) => {
            eprintln!(
                "[graphenium] Loaded graph: {} ({} nodes, {} edges)",
                graph_path.display(),
                g.node_count(),
                g.edge_count()
            );
            (g, graph_path.to_path_buf(), false)
        }
        Err(e) if graph_path.exists() => {
            return Err(e);
        }
        Err(_) => {
            eprintln!(
                "[graphenium] Status: Graph file not found at {}. Starting server with empty state.",
                graph_path.display()
            );
            eprintln!(
                "[graphenium] Run `gm run . --no-semantic` in your workspace to generate the codebase map."
            );
            let mut g = crate::model::GrapheniumGraph::new();
            g.metadata.ast_only = true;
            (g, graph_path.to_path_buf(), true)
        }
    };

    let server = GrapheniumServer::with_path(graph, watch_path.clone());

    if watch {
        let srv = server.clone();
        let w_path = watch_path.clone();
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

            // If graph file doesn't exist yet, watch the parent directory instead
            let watch_target = if w_path.exists() {
                w_path.clone()
            } else if let Some(parent) = w_path.parent() {
                parent.to_path_buf()
            } else {
                eprintln!(
                    "[graphenium] Cannot watch: no parent directory for {}",
                    w_path.display()
                );
                return;
            };

            let mode = if w_path.exists() {
                RecursiveMode::NonRecursive
            } else {
                RecursiveMode::NonRecursive
            };

            if let Err(e) = watcher.watch(&watch_target, mode) {
                eprintln!("[graphenium] File watch failed: {e}");
                return;
            }
            eprintln!(
                "[graphenium] Watching {} for changes",
                w_path.file_name().unwrap_or_default().to_string_lossy()
            );

            for event in rx {
                if let Ok(Event {
                    kind: EventKind::Modify(_) | EventKind::Create(_),
                    ..
                }) = event
                {
                    // When watching a parent directory, only reload if the specific file changed
                    if !w_path.exists() {
                        continue;
                    }
                    std::thread::sleep(Duration::from_millis(200));
                    if let Err(err) = GrapheniumServer::reload_from_file(&w_path, &srv) {
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
