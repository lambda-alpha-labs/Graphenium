/// God-node detection.
///
/// A "god node" is a highly-connected entity that many other nodes depend on.
/// We filter out two categories of false positives:
///   - **File-level hubs**: nodes whose label matches the source-file stem
///     (e.g. a `mod.rs` "module" node is not a real architectural god node).
///   - **Method stubs**: nodes with degree ≤ 1 (connected to at most one thing).
use std::collections::HashSet;
use std::path::Path;

use crate::model::graph::GrapheniumGraph;
use crate::ranking;

// ── Public types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct GodNode {
    pub node_id: String,
    pub label: String,
    /// Namespaced label when the extractor produced one; empty otherwise.
    pub qualified_label: Option<String>,
    pub degree: usize,
    pub community: Option<usize>,
    pub source_file: String,
}

impl GodNode {
    /// Label to show users: prefers `qualified_label`, falls back to `label`.
    pub fn display_label(&self) -> &str {
        self.qualified_label.as_deref().unwrap_or(&self.label)
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Return the top `top_n` nodes sorted by degree, after filtering stubs and
/// file-level hubs.
pub fn god_nodes(graph: &GrapheniumGraph, top_n: usize) -> Vec<GodNode> {
    god_nodes_in_scope(graph, top_n, None)
}

/// Scoped variant of [`god_nodes`] that only considers nodes inside `allowed`.
pub fn god_nodes_in_scope(
    graph: &GrapheniumGraph,
    top_n: usize,
    allowed: Option<&HashSet<String>>,
) -> Vec<GodNode> {
    let mut candidates: Vec<GodNode> = graph
        .nodes()
        .filter_map(|node| {
            if allowed.is_some_and(|allowed| !allowed.contains(&node.id)) {
                return None;
            }

            let deg = ranking::degree_in_scope(graph, &node.id, allowed);
            if deg <= 1 {
                return None; // method stub
            }
            // File-level hub: label matches source-file stem (case-insensitive)
            let file_stem = Path::new(&node.source_file)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();
            if node.label.to_lowercase() == file_stem {
                return None;
            }
            if ranking::is_framework_noise_node(graph, node) {
                return None;
            }
            // Phase 1: Filter out namespace/import aggregation hubs (low-signal hubs that
            // merely re-export external symbols). This runs BEFORE candidates.truncate().
            if ranking::is_namespace_aggregation_node(node, graph) {
                return None;
            }
            Some(GodNode {
                node_id: node.id.clone(),
                label: node.label.clone(),
                qualified_label: node.qualified_label.clone(),
                degree: deg,
                community: node.community,
                source_file: node.source_file.clone(),
            })
        })
        .collect();

    candidates.sort_unstable_by(|a, b| b.degree.cmp(&a.degree));
    candidates.truncate(top_n);
    candidates
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Edge, FileType, Node};

    fn build(nodes: &[(&str, &str, &str)], edges: &[(&str, &str)]) -> GrapheniumGraph {
        let mut g = GrapheniumGraph::new();
        for &(id, label, sf) in nodes {
            g.upsert_node(Node::new(id, label, FileType::Code, sf));
        }
        for &(s, t) in edges {
            g.add_edge(Edge::extracted(s, t, "calls", "f.rs"));
        }
        g
    }

    #[test]
    fn top_n_sorted_by_degree() {
        let g = build(
            &[
                ("a", "A", "f.rs"),
                ("b", "B", "f.rs"),
                ("c", "C", "f.rs"),
                ("d", "D", "f.rs"),
            ],
            &[("a", "b"), ("a", "c"), ("a", "d"), ("b", "c")],
        );
        let gods = god_nodes(&g, 2);
        assert_eq!(gods.len(), 2);
        assert_eq!(gods[0].node_id, "a"); // degree 3
    }

    #[test]
    fn stubs_are_filtered() {
        let g = build(
            &[("a", "A", "f.rs"), ("b", "B", "f.rs"), ("c", "C", "f.rs")],
            &[("a", "b"), ("a", "c")],
        );
        // b and c have degree 1 → stubs
        let gods = god_nodes(&g, 10);
        assert_eq!(gods.len(), 1);
        assert_eq!(gods[0].node_id, "a");
    }

    #[test]
    fn file_level_hub_filtered() {
        // Node whose label matches the source-file stem
        let g = build(
            &[
                ("f", "f", "f.rs"), // label "f" == stem of "f.rs"
                ("a", "A", "f.rs"),
                ("b", "B", "f.rs"),
                ("c", "C", "f.rs"),
            ],
            &[("f", "a"), ("f", "b"), ("f", "c"), ("a", "b")],
        );
        let gods = god_nodes(&g, 10);
        assert!(gods.iter().all(|n| n.node_id != "f"));
    }

    #[test]
    fn empty_graph_returns_empty() {
        let g = GrapheniumGraph::new();
        assert!(god_nodes(&g, 10).is_empty());
    }

    #[test]
    fn framework_import_hub_filtered() {
        let mut g = GrapheniumGraph::new();
        g.upsert_node(Node::new(
            "system",
            "System",
            FileType::Code,
            "tests/app.cs",
        ));
        g.upsert_node(Node::new(
            "service",
            "OrderService",
            FileType::Code,
            "src/Services.cs",
        ));
        g.upsert_node(Node::new("a", "A", FileType::Code, "src/A.cs"));
        g.upsert_node(Node::new("b", "B", FileType::Code, "src/B.cs"));
        g.upsert_node(Node::new("c", "C", FileType::Code, "src/C.cs"));
        g.add_edge(Edge::extracted("a", "system", "imports", "src/A.cs"));
        g.add_edge(Edge::extracted("b", "system", "imports", "src/B.cs"));
        g.add_edge(Edge::extracted("c", "system", "imports", "src/C.cs"));
        g.add_edge(Edge::extracted("a", "service", "calls", "src/A.cs"));
        g.add_edge(Edge::extracted("b", "service", "calls", "src/B.cs"));

        let gods = god_nodes(&g, 10);
        assert!(gods.iter().all(|node| node.node_id != "system"));
        assert!(gods.iter().any(|node| node.node_id == "service"));
    }
}
