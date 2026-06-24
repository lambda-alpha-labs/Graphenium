//! ## Graphenium Harness Adapter
//!
//! Reference integration for embedding the Graphenium knowledge graph engine
//! inside an AI coding harness. The adapter provides a small, opinionated API
//! surface that a harness can call at the right lifecycle points:
//!
//! - **Workspace open** → `initialize_graph(path)`
//! - **File opened/edited** → `on_file_open(graph, path)`
//! - **Relationship discovered** → `on_edge_discovered(graph, ...)`
//! - **Stale relationship corrected** → `on_edge_invalid(graph, ...)`
//! - **Periodic or on-change** → `refresh_communities(graph)`
//! - **MCP server needs current data** → `snapshot_to_disk(graph, path)`
//!
//! The adapter is read-only over the harness's event loop — it receives
//! events and mutates the graph. The harness owns the event loop and
//! lifecycle.

use std::path::{Path, PathBuf};

use graphenium::{
    analyze, build, cluster,
    cluster::ClusterOptions,
    extract::{extract_file, ExtractOptions},
    model::{Confidence, DetectedFile, Edge, FileType, GrapheniumGraph, Node, ReplaceStats},
    CommunityStats,
};

// ── Public types ──────────────────────────────────────────────────────────

/// Statistics returned by [`on_file_open`].
#[derive(Debug, Clone, Default)]
pub struct PatchStats {
    /// Nodes replaced (old removal + new insertion).
    pub nodes_replaced: usize,
    /// New edges inserted.
    pub edges_inserted: usize,
    /// Edges dropped because endpoints no longer existed.
    pub edges_dropped: usize,
}

/// Summary after a full initialization scan.
#[derive(Debug, Clone)]
pub struct InitResult {
    pub nodes: usize,
    pub edges: usize,
    pub communities: usize,
}

// ── Lifecycle functions ───────────────────────────────────────────────────

/// Build a fresh knowledge graph from all code files under `root`.
///
/// Call this once when the harness opens a workspace for the first time.
/// Uses AST-only extraction (no API key required). Returns the graph and
/// community statistics.
pub fn initialize_graph(root: &Path) -> (GrapheniumGraph, Vec<CommunityStats>) {
    let files = match graphenium::detect::detect(root, &graphenium::detect::DetectOptions::default())
    {
        Ok((files, _warnings)) => files,
        Err(_) => return (GrapheniumGraph::new(), Vec::new()),
    };

    let ast_result = extract_all(&files);
    let (mut graph, _stats) = build::build_merged([ast_result]);
    graph.set_ast_only(true);

    let community_stats = cluster::cluster(&mut graph, &ClusterOptions::default());

    (graph, community_stats)
}

/// Same as `initialize_graph` but returns a summary suitable for user display.
pub fn initialize_and_summarize(root: &Path) -> InitResult {
    let (graph, stats) = initialize_graph(root);
    InitResult {
        nodes: graph.node_count(),
        edges: graph.edge_count(),
        communities: stats.len(),
    }
}

/// Call when the harness opens or saves a file. Extracts AST structure from
/// the file and patched the graph incrementally. Returns stats about what
/// changed.
///
/// If the extraction produces no nodes (unsupported language, empty file),
/// returns `PatchStats::default()` without touching the graph.
pub fn on_file_open(graph: &mut GrapheniumGraph, path: &Path) -> PatchStats {
    let file = DetectedFile {
        path: path.to_path_buf(),
        file_type: FileType::Code,
    };

    let result = extract_file(&file, &ExtractOptions::default());
    if result.is_empty() {
        return PatchStats::default();
    }

    let source_file = path.to_string_lossy().to_string();
    let stats = graph.replace_file_extraction(&source_file, &result);

    PatchStats {
        nodes_replaced: stats.nodes_removed.max(stats.nodes_inserted),
        edges_inserted: stats.edges_inserted,
        edges_dropped: stats.edges_dropped_dangling,
    }
}

/// Call when the harness (or the AI working through it) has confirmed a
/// relationship exists. Adds an edge with `EXTRACTED` confidence — the
/// relationship was verified through actual code inspection.
///
/// Returns `true` if the edge was added, `false` if a logically equivalent
/// edge already existed.
pub fn on_edge_discovered(
    graph: &mut GrapheniumGraph,
    source: &str,
    target: &str,
    relation: &str,
    source_file: &str,
) -> bool {
    let edge = Edge::extracted(source, target, relation, source_file);
    graph.add_edge(edge)
}

/// Call when the harness detects that an existing edge is incorrect or stale.
/// Removes all edges matching the given criteria and returns the count removed.
///
/// If `relation` is `None`, all edges between `source` and `target` are removed.
pub fn on_edge_invalid(
    graph: &mut GrapheniumGraph,
    source: &str,
    target: &str,
    relation: Option<&str>,
) -> usize {
    let src_idx = match graph.node_index(source) {
        Some(idx) => idx,
        None => return 0,
    };
    let tgt_idx = match graph.node_index(target) {
        Some(idx) => idx,
        None => return 0,
    };

    let edge_indices: Vec<_> = graph
        .inner()
        .edges_connecting(src_idx, tgt_idx)
        .filter(|eref| {
            if let Some(rel) = relation {
                eref.weight().relation.to_lowercase() == rel.trim().to_lowercase()
            } else {
                true
            }
        })
        .map(|eref| eref.id())
        .collect();

    let count = edge_indices.len();
    for ei in edge_indices {
        graph.inner.remove_edge(ei);
    }
    count
}

/// Re-run Louvain community detection and return updated statistics.
///
/// Call this periodically (e.g. after every N file changes) or after the
/// graph has grown significantly since the last clustering pass.
pub fn refresh_communities(
    graph: &mut GrapheniumGraph,
    opts: &ClusterOptions,
) -> Vec<CommunityStats> {
    cluster::cluster(graph, opts)
}

/// Serialize the current graph to a JSON file so an MCP server (or
/// `gm serve` sidecar) can serve it.
pub fn snapshot_to_disk(graph: &GrapheniumGraph, path: &Path) -> Result<(), String> {
    let json = graphenium::export::json::to_json(graph).map_err(|e| e.to_string())?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }

    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &json).map_err(|e| format!("write: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("rename: {e}"))?;

    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn extract_all(files: &[DetectedFile]) -> graphenium::ExtractionResult {
    graphenium::extract::extract_all(files, &ExtractOptions::default())
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn write_file(dir: &TempDir, name: &str, content: &str) -> PathBuf {
        let path = dir.path().join(name);
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn initialize_empty_directory() {
        let dir = TempDir::new().unwrap();
        let (graph, stats) = initialize_graph(dir.path());
        assert_eq!(graph.node_count(), 0);
        assert!(stats.is_empty());
    }

    #[test]
    fn on_file_open_extracts_python() {
        let dir = TempDir::new().unwrap();
        write_file(&dir, "lib.py", "def hello():\n    pass\n");

        let mut graph = GrapheniumGraph::new();
        let stats = on_file_open(&mut graph, &dir.path().join("lib.py"));

        assert!(stats.nodes_replaced > 0, "should extract at least one node");
        assert!(graph.node_count() > 0);
    }

    #[test]
    fn on_edge_discovered_adds_edge() {
        let mut graph = GrapheniumGraph::new();
        graph.upsert_node(Node::new("a_fn", "fn", FileType::Code, "a.py"));
        graph.upsert_node(Node::new("b_fn", "fn", FileType::Code, "b.py"));

        let added = on_edge_discovered(&mut graph, "a_fn", "b_fn", "calls", "a.py");
        assert!(added);
        assert_eq!(graph.edge_count(), 1);
    }

    #[test]
    fn on_edge_invalid_removes_edge() {
        let mut graph = GrapheniumGraph::new();
        graph.upsert_node(Node::new("a_fn", "fn", FileType::Code, "a.py"));
        graph.upsert_node(Node::new("b_fn", "fn", FileType::Code, "b.py"));
        graph.add_edge(Edge::extracted("a_fn", "b_fn", "calls", "a.py"));

        let removed = on_edge_invalid(&mut graph, "a_fn", "b_fn", Some("calls"));
        assert_eq!(removed, 1);
        assert_eq!(graph.edge_count(), 0);
    }

    #[test]
    fn on_edge_invalid_no_match_returns_zero() {
        let mut graph = GrapheniumGraph::new();
        let removed = on_edge_invalid(&mut graph, "x", "y", None);
        assert_eq!(removed, 0);
    }

    #[test]
    fn snapshot_roundtrip() {
        let mut graph = GrapheniumGraph::new();
        graph.upsert_node(Node::new("n", "n", FileType::Code, "f.rs"));

        let dir = TempDir::new().unwrap();
        let path = dir.path().join("graph.json");

        snapshot_to_disk(&graph, &path).unwrap();
        assert!(path.exists());

        let reloaded = graphenium::export::json::load_graph(&path).unwrap();
        assert_eq!(reloaded.node_count(), 1);
    }

    #[test]
    fn refresh_communities_on_small_graph() {
        let mut graph = GrapheniumGraph::new();
        graph.upsert_node(Node::new("a", "a", FileType::Code, "f.rs"));
        graph.upsert_node(Node::new("b", "b", FileType::Code, "f.rs"));
        graph.add_edge(Edge::extracted("a", "b", "calls", "f.rs"));

        let stats = refresh_communities(&mut graph, &ClusterOptions::default());
        assert!(!stats.is_empty());
        assert!(graph.node_data("a").unwrap().community.is_some());
    }
}
