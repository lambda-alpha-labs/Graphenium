/// Community cohesion scoring.
///
/// Cohesion measures how densely connected a community is internally:
///
/// ```text
/// cohesion = actual_internal_edges / (n * (n-1) / 2)
/// ```
///
/// where `n` is the number of nodes in the community and the denominator is
/// the number of edges in a complete undirected graph with `n` nodes.
///
/// A score of 1.0 = all nodes fully connected (clique).
/// A score of 0.0 = no internal edges.
use std::collections::HashMap;

use crate::cluster::focus;
use crate::model::GrapheniumGraph;

// ── Public types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CommunityStats {
    pub community_id: usize,
    /// Number of nodes in this community.
    pub size: usize,
    /// Number of edges with both endpoints in this community.
    pub internal_edges: usize,
    /// `internal_edges / (n*(n-1)/2)`.  Zero for singleton communities.
    pub cohesion: f64,
    /// Node IDs belonging to this community (sorted for determinism).
    pub members: Vec<String>,
    /// Human-readable path focus (longest shared directory prefix, or top
    /// file names when members are scattered). `None` for communities with
    /// no coherent focus.
    pub focus: Option<String>,
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Compute `CommunityStats` for each community present in `graph`.
///
/// `assignments[i]` is the community ID for `node_ids[i]`.
pub fn community_stats(
    graph: &GrapheniumGraph,
    assignments: &[usize],
    node_ids: &[String],
) -> Vec<CommunityStats> {
    // Group nodes by community.
    let mut comm_members: HashMap<usize, Vec<String>> = HashMap::new();
    for (i, id) in node_ids.iter().enumerate() {
        if let Some(&c) = assignments.get(i) {
            comm_members.entry(c).or_default().push(id.clone());
        }
    }

    // Build a set for quick membership testing.
    let member_set: HashMap<&str, usize> = node_ids
        .iter()
        .enumerate()
        .filter_map(|(i, id)| assignments.get(i).map(|&c| (id.as_str(), c)))
        .collect();

    // Count internal edges per community.
    let mut internal_counts: HashMap<usize, usize> = HashMap::new();
    for (src, tgt, _edge) in graph.edges_with_endpoints() {
        let cs = member_set.get(src).copied();
        let ct = member_set.get(tgt).copied();
        if let (Some(a), Some(b)) = (cs, ct) {
            if a == b {
                *internal_counts.entry(a).or_insert(0) += 1;
            }
        }
    }

    // Build stats, one entry per community, sorted by size descending.
    let mut stats: Vec<CommunityStats> = comm_members
        .into_iter()
        .map(|(id, mut members)| {
            members.sort();
            let n = members.len();
            let ie = internal_counts.get(&id).copied().unwrap_or(0);
            let max_edges = if n >= 2 { n * (n - 1) / 2 } else { 0 };
            let cohesion = if max_edges > 0 {
                ie as f64 / max_edges as f64
            } else {
                0.0
            };
            let paths: Vec<String> = members
                .iter()
                .filter_map(|mid| graph.node_data(mid).map(|n| n.source_file.clone()))
                .collect();
            let focus = focus::focus_label(&paths);
            CommunityStats {
                community_id: id,
                size: n,
                internal_edges: ie,
                cohesion,
                members,
                focus,
            }
        })
        .collect();

    stats.sort_unstable_by(|a, b| {
        b.size
            .cmp(&a.size)
            .then(a.community_id.cmp(&b.community_id))
    });
    stats
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Edge, FileType, Node};

    fn make_graph_with_edges(node_ids: &[&str], edges: &[(&str, &str)]) -> GrapheniumGraph {
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
    fn clique_has_cohesion_one() {
        // Three nodes fully connected → cohesion = 1.0
        let g = make_graph_with_edges(&["a", "b", "c"], &[("a", "b"), ("b", "c"), ("a", "c")]);
        let node_ids = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let assignments = vec![0, 0, 0]; // all in community 0

        let stats = community_stats(&g, &assignments, &node_ids);
        assert_eq!(stats.len(), 1);
        assert!((stats[0].cohesion - 1.0).abs() < 1e-9);
        assert_eq!(stats[0].internal_edges, 3);
    }

    #[test]
    fn no_internal_edges_cohesion_zero() {
        let g = make_graph_with_edges(&["a", "b", "c", "d"], &[("a", "c"), ("b", "d")]);
        // communities: {a,b} = 0, {c,d} = 1
        // edge (a,c) is cross-community, (b,d) is cross-community
        let node_ids = vec![
            "a".to_string(),
            "b".to_string(),
            "c".to_string(),
            "d".to_string(),
        ];
        let assignments = vec![0, 0, 1, 1];

        let stats = community_stats(&g, &assignments, &node_ids);
        for s in &stats {
            assert_eq!(s.internal_edges, 0);
            assert_eq!(s.cohesion, 0.0);
        }
    }

    #[test]
    fn partial_connectivity() {
        // 4-node community with 4 of 6 possible edges → cohesion = 4/6
        let g = make_graph_with_edges(
            &["a", "b", "c", "d"],
            &[("a", "b"), ("a", "c"), ("a", "d"), ("b", "c")],
        );
        let node_ids = vec![
            "a".to_string(),
            "b".to_string(),
            "c".to_string(),
            "d".to_string(),
        ];
        let assignments = vec![0, 0, 0, 0];

        let stats = community_stats(&g, &assignments, &node_ids);
        assert_eq!(stats[0].internal_edges, 4);
        assert!((stats[0].cohesion - 4.0 / 6.0).abs() < 1e-9);
    }

    #[test]
    fn singleton_cohesion_zero() {
        let g = make_graph_with_edges(&["a"], &[]);
        let node_ids = vec!["a".to_string()];
        let assignments = vec![0];

        let stats = community_stats(&g, &assignments, &node_ids);
        assert_eq!(stats[0].cohesion, 0.0);
    }

    #[test]
    fn two_communities_reported() {
        let g = make_graph_with_edges(&["a", "b", "c", "d"], &[("a", "b"), ("c", "d")]);
        let node_ids = vec![
            "a".to_string(),
            "b".to_string(),
            "c".to_string(),
            "d".to_string(),
        ];
        let assignments = vec![0, 0, 1, 1];

        let stats = community_stats(&g, &assignments, &node_ids);
        assert_eq!(stats.len(), 2);
    }
}
