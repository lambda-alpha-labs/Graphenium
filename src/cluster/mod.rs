pub mod cohesion;
pub mod drift;
pub mod focus;
pub mod louvain;
pub mod split;

use std::collections::HashMap;

use petgraph::visit::EdgeRef;

use crate::model::GrapheniumGraph;

pub use cohesion::CommunityStats;
pub use louvain::LouvainConfig;

// ── Public options ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ClusterOptions {
    /// RNG seed (default 42, matching Python version).
    pub seed: u64,
    /// Maximum Louvain aggregation levels (default 10).
    pub max_level: usize,
    /// Modularity improvement threshold to stop (default 1e-4).
    pub threshold: f64,
    /// Fraction of total nodes above which a community is considered oversized
    /// and eligible for splitting (default 0.25 = 25 %).
    pub split_fraction: f64,
    /// Minimum community size before splitting is attempted (default 10).
    pub split_min_size: usize,
}

impl Default for ClusterOptions {
    fn default() -> Self {
        Self {
            seed: 42,
            max_level: 10,
            threshold: 1e-4,
            split_fraction: 0.25,
            split_min_size: 10,
        }
    }
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Detect communities in `graph` using the Louvain algorithm, write the
/// community ID back onto each node, and return per-community statistics.
/// After this call, `node.community` is `Some(k)` for every node, where `k`
/// is 0-indexed and community 0 is the largest.
pub fn cluster(graph: &mut GrapheniumGraph, opts: &ClusterOptions) -> Vec<CommunityStats> {
    let n = graph.node_count();
    if n == 0 {
        return Vec::new();
    }

    // ── Build sequential index ────────────────────────────────────────────
    // petgraph node indices are not necessarily 0..n; build our own mapping.
    let node_indices: Vec<_> = graph.inner().node_indices().collect();
    let idx_to_seq: HashMap<_, usize> = node_indices
        .iter()
        .enumerate()
        .map(|(i, &ni)| (ni, i))
        .collect();

    // Collect node IDs now, before any mutable borrows.
    let node_ids: Vec<String> = node_indices
        .iter()
        .map(|&ni| graph.inner()[ni].id.clone())
        .collect();

    // ── Build edge list for Louvain ───────────────────────────────────────
    let edges: Vec<(usize, usize, f64)> = graph
        .inner()
        .edge_references()
        .map(|e| {
            let u = idx_to_seq[&e.source()];
            let v = idx_to_seq[&e.target()];
            let w = e.weight().weight;
            (u, v, w)
        })
        .collect();

    // ── Run Louvain ───────────────────────────────────────────────────────
    let louvain_cfg = LouvainConfig {
        seed: opts.seed,
        max_level: opts.max_level,
        threshold: opts.threshold,
    };
    let mut assignments = louvain::run(n, &edges, &louvain_cfg);

    // ── Split oversized communities ───────────────────────────────────────
    split::split_large(
        &mut assignments,
        n,
        &edges,
        opts.split_fraction,
        opts.split_min_size,
        &louvain_cfg,
    );

    // ── Write community IDs back to graph nodes ───────────────────────────
    for (i, &comm) in assignments.iter().enumerate() {
        if let Some(node) = graph.node_data_mut(&node_ids[i]) {
            node.community = Some(comm);
        }
    }

    // ── Compute and return cohesion statistics ───────────────────────────
    cohesion::community_stats(graph, &assignments, &node_ids)
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
    fn empty_graph_returns_no_stats() {
        let mut g = GrapheniumGraph::new();
        let stats = cluster(&mut g, &ClusterOptions::default());
        assert!(stats.is_empty());
    }

    #[test]
    fn all_nodes_assigned_a_community() {
        let mut g = build_graph(&["a", "b", "c"], &[("a", "b"), ("b", "c")]);
        cluster(&mut g, &ClusterOptions::default());
        for id in &["a", "b", "c"] {
            assert!(
                g.node_data(id).unwrap().community.is_some(),
                "node {id} should have a community"
            );
        }
    }

    #[test]
    fn two_cliques_separate_communities() {
        // Two triangles with a weak bridge — should form two communities.
        let mut g = build_graph(
            &["a", "b", "c", "d", "e", "f"],
            &[
                ("a", "b"),
                ("b", "c"),
                ("a", "c"), // clique 1
                ("d", "e"),
                ("e", "f"),
                ("d", "f"), // clique 2
                            // no bridge — disconnected, so definitely two communities
            ],
        );
        let stats = cluster(&mut g, &ClusterOptions::default());
        assert!(
            stats.len() >= 2,
            "expected ≥ 2 communities, got {}",
            stats.len()
        );

        let ca = g.node_data("a").unwrap().community.unwrap();
        let cd = g.node_data("d").unwrap().community.unwrap();
        assert_ne!(ca, cd, "cliques should be in different communities");
    }

    #[test]
    fn community_stats_sizes_match() {
        let mut g = build_graph(&["a", "b", "c"], &[("a", "b"), ("b", "c"), ("a", "c")]);
        let stats = cluster(&mut g, &ClusterOptions::default());
        let total_nodes: usize = stats.iter().map(|s| s.size).sum();
        assert_eq!(total_nodes, 3, "stats sizes should sum to total node count");
    }

    #[test]
    fn largest_community_is_first_in_stats() {
        // 3-node clique + 1 isolated node
        let mut g = build_graph(&["a", "b", "c", "x"], &[("a", "b"), ("b", "c"), ("a", "c")]);
        let stats = cluster(&mut g, &ClusterOptions::default());
        // Stats are sorted by size descending; first entry should be ≥ second.
        if stats.len() >= 2 {
            assert!(stats[0].size >= stats[1].size);
        }
    }
}
