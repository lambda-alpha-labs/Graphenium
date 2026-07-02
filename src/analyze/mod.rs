pub mod diff;
pub mod god;
pub mod impact;
pub mod query;
pub mod questions;
pub mod rank;
pub mod surprise;
pub mod verifier;

use crate::cluster::CommunityStats;
use crate::model::graph::GrapheniumGraph;

pub use diff::{diff, GraphDiff};
pub use god::{god_nodes, god_nodes_in_scope, GodNode};
pub use questions::{suggest_questions, SuggestedQuestion};
pub use surprise::{surprising_connections, SurprisingEdge};

// ── Public aggregate types ────────────────────────────────────────────────────

/// All analysis results for a completed, clustered graph.
#[derive(Debug)]
pub struct AnalysisResult {
    pub god_nodes: Vec<GodNode>,
    pub surprising: Vec<SurprisingEdge>,
    pub questions: Vec<SuggestedQuestion>,
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Run all analysis passes and return combined results.
/// - `graph` must have been clustered (community IDs assigned) before calling.
/// - `community_stats` is the output of `cluster::cluster()`.
pub fn analyze(graph: &GrapheniumGraph, community_stats: &[CommunityStats]) -> AnalysisResult {
    AnalysisResult {
        god_nodes: god_nodes(graph, 20),
        surprising: surprising_connections(graph, 20),
        questions: suggest_questions(graph, community_stats),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Edge, FileType, Node};

    fn build_graph(node_ids: &[&str], edges: &[(&str, &str)]) -> GrapheniumGraph {
        let mut g = GrapheniumGraph::new();
        for &id in node_ids {
            g.upsert_node(Node::new(id, id, FileType::Code, "f.rs"));
        }
        for &(s, t) in edges {
            g.add_edge(Edge::extracted(s, t, "calls", "f.rs"));
        }
        g
    }

    #[test]
    fn analyze_empty_graph() {
        let g = GrapheniumGraph::new();
        let r = analyze(&g, &[]);
        assert!(r.god_nodes.is_empty());
        assert!(r.surprising.is_empty());
    }

    #[test]
    fn analyze_small_graph_does_not_panic() {
        let g = build_graph(&["a", "b", "c"], &[("a", "b"), ("b", "c"), ("a", "c")]);
        let _ = analyze(&g, &[]);
    }
}
