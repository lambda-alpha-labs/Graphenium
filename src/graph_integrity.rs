//! Graph invariant checker — validates graph consistency after mutations.
//!
//! Used in debug builds after incremental updates and by `gm doctor --graph-integrity`.

use crate::model::GrapheniumGraph;

/// Result of a consistency check.
#[derive(Debug, Clone, Default)]
pub struct IntegrityReport {
    pub passed: bool,
    pub total_nodes: usize,
    pub total_edges: usize,
    pub id_index_mismatches: Vec<String>,
    pub duplicate_ids: Vec<String>,
    pub dangling_edges: Vec<(String, String, String)>,
}

impl IntegrityReport {
    pub fn is_healthy(&self) -> bool {
        self.id_index_mismatches.is_empty()
            && self.duplicate_ids.is_empty()
            && self.dangling_edges.is_empty()
    }

    pub fn format(&self) -> String {
        if self.is_healthy() && self.passed {
            return format!(
                "PASSED ({} nodes, {} edges).",
                self.total_nodes, self.total_edges
            );
        }
        let mut out = format!(
            "ISSUES ({} nodes, {} edges)\n",
            self.total_nodes, self.total_edges
        );
        if !self.id_index_mismatches.is_empty() {
            out.push_str(&format!(
                "  id_index mismatches: {}\n",
                self.id_index_mismatches.len()
            ));
        }
        if !self.dangling_edges.is_empty() {
            out.push_str(&format!(
                "  dangling edges: {}\n",
                self.dangling_edges.len()
            ));
        }
        out
    }
}

/// Check all graph invariants.
pub fn check_invariants(graph: &GrapheniumGraph) -> IntegrityReport {
    let mut report = IntegrityReport {
        total_nodes: graph.node_count(),
        total_edges: graph.edge_count(),
        ..Default::default()
    };

    for (id, idx) in &graph.id_index {
        if let Some(node) = graph.inner.node_weight(*idx) {
            if node.id != *id {
                report.id_index_mismatches.push(format!(
                    "id_index[{id}] -> NodeIndex({:?}) has id '{}'",
                    idx, node.id
                ));
            }
        }
    }

    for edge in graph.edges_iter() {
        if !graph.id_index.contains_key(&edge.source) {
            report.dangling_edges.push((
                edge.source.clone(),
                edge.target.clone(),
                edge.relation.clone(),
            ));
        }
        if !graph.id_index.contains_key(&edge.target) {
            report.dangling_edges.push((
                edge.source.clone(),
                edge.target.clone(),
                edge.relation.clone(),
            ));
        }
    }

    report.passed = report.is_healthy();
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Edge, FileType, Node};

    #[test]
    fn clean_passes() {
        let mut g = GrapheniumGraph::new();
        g.upsert_node(Node::new("a", "A", FileType::Code, "src/a.rs"));
        g.upsert_node(Node::new("b", "B", FileType::Code, "src/b.rs"));
        g.add_edge(Edge::extracted("a", "b", "calls", "src/a.rs"));
        assert!(check_invariants(&g).passed);
    }

    #[test]
    fn corrupted_index_caught() {
        let mut g = GrapheniumGraph::new();
        g.upsert_node(Node::new("a", "A", FileType::Code, "src/a.rs"));
        if let Some(idx) = g.id_index.get("a").copied() {
            g.id_index.insert("wrong".to_string(), idx);
        }
        assert!(!check_invariants(&g).passed);
    }
}
