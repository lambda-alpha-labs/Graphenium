/// Graph diffing — detect added/removed nodes and edges between two snapshots.
use std::collections::HashSet;

use crate::model::graph::GrapheniumGraph;

// ── Public types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct GraphDiff {
    /// Node IDs present in `new` but not in `old`.
    pub added_nodes: Vec<String>,
    /// Node IDs present in `old` but not in `new`.
    pub removed_nodes: Vec<String>,
    /// `(source_id, target_id, relation)` triples present in `new` but not `old`.
    pub added_edges: Vec<(String, String, String)>,
    /// `(source_id, target_id, relation)` triples present in `old` but not `new`.
    pub removed_edges: Vec<(String, String, String)>,
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Compare two graph snapshots and return what changed.
/// Edge identity is `(source_id, target_id, relation)`.  Changes in weight,
/// confidence, or metadata do not produce a diff entry.
pub fn diff(old: &GrapheniumGraph, new: &GrapheniumGraph) -> GraphDiff {
    let old_nodes: HashSet<String> = old.node_ids().map(|s| s.to_string()).collect();
    let new_nodes: HashSet<String> = new.node_ids().map(|s| s.to_string()).collect();

    let mut added_nodes: Vec<String> = new_nodes.difference(&old_nodes).cloned().collect();
    let mut removed_nodes: Vec<String> = old_nodes.difference(&new_nodes).cloned().collect();
    added_nodes.sort();
    removed_nodes.sort();

    let edge_set = |g: &GrapheniumGraph| -> HashSet<(String, String, String)> {
        g.edges_with_endpoints()
            .map(|(s, t, e)| (s.to_string(), t.to_string(), e.relation.clone()))
            .collect()
    };

    let old_edges = edge_set(old);
    let new_edges = edge_set(new);

    let mut added_edges: Vec<_> = new_edges.difference(&old_edges).cloned().collect();
    let mut removed_edges: Vec<_> = old_edges.difference(&new_edges).cloned().collect();
    added_edges.sort();
    removed_edges.sort();

    GraphDiff {
        added_nodes,
        removed_nodes,
        added_edges,
        removed_edges,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Edge, FileType, Node};

    fn make(nodes: &[&str], edges: &[(&str, &str)]) -> GrapheniumGraph {
        let mut g = GrapheniumGraph::new();
        for &id in nodes {
            g.upsert_node(Node::new(id, id, FileType::Code, "f.rs"));
        }
        for &(s, t) in edges {
            g.add_edge(Edge::extracted(s, t, "calls", "f.rs"));
        }
        g
    }

    #[test]
    fn added_node_detected() {
        let old = make(&["a", "b"], &[("a", "b")]);
        let new = make(&["a", "b", "c"], &[("a", "b")]);
        let d = diff(&old, &new);
        assert_eq!(d.added_nodes, vec!["c"]);
        assert!(d.removed_nodes.is_empty());
    }

    #[test]
    fn removed_node_detected() {
        let old = make(&["a", "b", "c"], &[("a", "b")]);
        let new = make(&["a", "b"], &[("a", "b")]);
        let d = diff(&old, &new);
        assert_eq!(d.removed_nodes, vec!["c"]);
        assert!(d.added_nodes.is_empty());
    }

    #[test]
    fn added_edge_detected() {
        let old = make(&["a", "b", "c"], &[("a", "b")]);
        let new = make(&["a", "b", "c"], &[("a", "b"), ("b", "c")]);
        let d = diff(&old, &new);
        assert_eq!(d.added_edges.len(), 1);
        assert!(d.removed_edges.is_empty());
    }

    #[test]
    fn removed_edge_detected() {
        let old = make(&["a", "b", "c"], &[("a", "b"), ("b", "c")]);
        let new = make(&["a", "b", "c"], &[("a", "b")]);
        let d = diff(&old, &new);
        assert!(d.added_edges.is_empty());
        assert_eq!(d.removed_edges.len(), 1);
    }

    #[test]
    fn identical_graphs_produce_empty_diff() {
        let g = make(&["a", "b"], &[("a", "b")]);
        let d = diff(&g, &g);
        assert!(d.added_nodes.is_empty());
        assert!(d.removed_nodes.is_empty());
        assert!(d.added_edges.is_empty());
        assert!(d.removed_edges.is_empty());
    }

    #[test]
    fn both_empty_graphs() {
        let old = GrapheniumGraph::new();
        let new = GrapheniumGraph::new();
        let d = diff(&old, &new);
        assert!(d.added_nodes.is_empty());
        assert!(d.removed_nodes.is_empty());
    }
}
