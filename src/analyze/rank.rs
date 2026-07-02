//! Directed graph projection and ranking algorithms.
//!
//! Graphenium's underlying `petgraph` graph is undirected. Directionality is
//! preserved logically in each `Edge` via `src_original` and `tgt_original`.
//! This module projects directed views from the undirected graph for ranking
//! algorithms that require direction: PageRank, reverse reachability, dominators.

use std::collections::HashMap;

use crate::model::graph::GrapheniumGraph;

/// A directed edge for use in ranking algorithms.
#[derive(Debug, Clone)]
pub struct DirectedEdge {
    pub source: String,
    pub target: String,
    pub relation: String,
    pub weight: f64,
}

/// A directed graph projection.
/// This is a lightweight adjacency list built from the undirected
/// GrapheniumGraph using `Edge::src_original` / `Edge::tgt_original`
/// to recover direction.
#[derive(Debug, Clone)]
pub struct DirectedProjection {
    /// Adjacency list: source -> [(target, weight)]
    pub outgoing: HashMap<String, Vec<(String, DirectedEdge)>>,
    /// Reverse adjacency list: target -> [(source, weight)]
    pub incoming: HashMap<String, Vec<(String, DirectedEdge)>>,
    /// All node IDs in this projection.
    pub nodes: Vec<String>,
}

impl DirectedProjection {
    /// Build a directed projection from a GrapheniumGraph.
    ///
    /// Only includes edges whose relation matches `relation_filter` if
    /// provided. Weight is the edge's traversal weight.
    ///
    /// When `src_original` / `tgt_original` are empty (deprecated edges or
    /// edges that never had direction set), uses `source` / `target` as a
    /// best-effort fallback.
    pub fn from_graph(graph: &GrapheniumGraph, relation_filter: Option<&[&str]>) -> Self {
        let mut outgoing: HashMap<String, Vec<(String, DirectedEdge)>> = HashMap::new();
        let mut incoming: HashMap<String, Vec<(String, DirectedEdge)>> = HashMap::new();
        let mut node_set: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

        for edge in graph.edges_iter() {
            // Apply relation filter
            if let Some(filter) = relation_filter {
                if !filter.contains(&edge.relation.as_str()) {
                    continue;
                }
            }

            // Determine direction: prefer src_original/tgt_original, fallback to source/target
            let src = if !edge.src_original.is_empty() {
                &edge.src_original
            } else {
                &edge.source
            };
            let tgt = if !edge.tgt_original.is_empty() {
                &edge.tgt_original
            } else {
                &edge.target
            };

            let de = DirectedEdge {
                source: src.clone(),
                target: tgt.clone(),
                relation: edge.relation.clone(),
                weight: edge.weight,
            };

            outgoing
                .entry(src.clone())
                .or_default()
                .push((tgt.clone(), de.clone()));
            incoming
                .entry(tgt.clone())
                .or_default()
                .push((src.clone(), de));

            node_set.insert(src.clone());
            node_set.insert(tgt.clone());
        }

        let nodes: Vec<String> = node_set.into_iter().collect();

        Self {
            outgoing,
            incoming,
            nodes,
        }
    }

    /// Number of nodes in this projection.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Number of edges in this projection (summed across outgoing).
    pub fn edge_count(&self) -> usize {
        self.outgoing.values().map(|v| v.len()).sum()
    }

    /// Check if the projection has no nodes.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

/// Compute PageRank on a directed projection.
/// Returns a map from node ID to PageRank score. Uses the standard
/// PageRank algorithm with damping factor `d` (default 0.85).
pub fn page_rank(proj: &DirectedProjection, d: f64, max_iter: usize) -> HashMap<String, f64> {
    let n = proj.nodes.len();
    if n == 0 {
        return HashMap::new();
    }

    let mut ranks: HashMap<String, f64> = proj
        .nodes
        .iter()
        .map(|id| (id.clone(), 1.0 / n as f64))
        .collect();
    let dangling = 1.0 / n as f64;

    for _iter in 0..max_iter {
        let mut new_ranks: HashMap<String, f64> = HashMap::new();
        let mut dangling_sum = 0.0;

        for id in &proj.nodes {
            let rank = ranks.get(id).copied().unwrap_or(dangling);
            let out_edges = proj.outgoing.get(id);
            if let Some(edges) = out_edges {
                if edges.is_empty() {
                    dangling_sum += rank;
                } else {
                    let w_sum: f64 = edges.iter().map(|(_, e)| e.weight).sum();
                    for (tgt, edge) in edges {
                        let contribution = rank * (edge.weight / w_sum);
                        *new_ranks.entry(tgt.clone()).or_insert(0.0) += contribution;
                    }
                }
            } else {
                dangling_sum += rank;
            }
        }

        // Teleportation
        let teleport = (1.0 - d) / n as f64;
        for id in &proj.nodes {
            let pr = teleport
                + d * (new_ranks.get(id).copied().unwrap_or(0.0) + dangling_sum * dangling);
            ranks.insert(id.clone(), pr);
        }
    }

    ranks
}

/// Compute community boundary crossing scores.
/// For each edge that crosses a community boundary, compute a weighted score.
/// Returns a list of (source_id, target_id, score) tuples sorted by score descending.
pub fn community_boundary_crossings(
    graph: &GrapheniumGraph,
    min_confidence: Option<&[crate::model::Confidence]>,
) -> Vec<(String, String, f64)> {
    let mut scores: Vec<(String, String, f64)> = Vec::new();

    for edge in graph.edges_iter() {
        // Filter by confidence if requested
        if let Some(confs) = min_confidence {
            if !confs.contains(&edge.confidence) {
                continue;
            }
        }

        // Get source and target nodes
        let src_node = graph.node_data(&edge.source);
        let tgt_node = graph.node_data(&edge.target);

        if let (Some(src), Some(tgt)) = (src_node, tgt_node) {
            // Check if they are in different communities
            if let (Some(sc), Some(tc)) = (src.community, tgt.community) {
                if sc != tc {
                    // Score: edge weight * confidence score
                    let score = edge.weight * edge.confidence_score;
                    scores.push((edge.source.clone(), edge.target.clone(), score));
                }
            }
        }
    }

    // Sort by score descending
    scores.sort_unstable_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    scores
}

/// Compute rooted dominators for a directed projection from a given root node.
/// Returns a map from node ID to its immediate dominator ID (the node that
/// must be passed through to reach this node from the root). Uses a simple
/// iterative algorithm suitable for small to medium graphs.
/// The root node maps to itself as its own dominator.
pub fn rooted_dominators(proj: &DirectedProjection, root: &str) -> HashMap<String, Option<String>> {
    use std::collections::BTreeSet;

    // Topological order via BFS from root
    let reachable = reverse_reachable_inverse(proj, root);
    let mut order: Vec<&str> = Vec::new();
    let mut visited = BTreeSet::new();
    let mut queue = std::collections::VecDeque::new();

    if reachable.iter().any(|s| s == root) || root == proj.nodes.first().map_or("", |s| s) {
        // root may not be in reachable if graph is disconnected
    }

    if proj.outgoing.contains_key(root) {
        queue.push_back(root);
        visited.insert(root);
    }

    while let Some(current) = queue.pop_front() {
        order.push(current);
        if let Some(edges) = proj.outgoing.get(current) {
            for (tgt, _) in edges {
                if visited.insert(tgt.as_str()) {
                    queue.push_back(tgt);
                }
            }
        }
    }

    // If root isn't reachable through BFS, try adding it directly
    if !visited.contains(root) {
        order.insert(0, root);
        visited.insert(root);
    }

    // Initialize dominators: root dominates itself, others dominated by all
    let mut dom: HashMap<String, Option<String>> = HashMap::new();
    dom.insert(root.to_string(), Some(root.to_string()));

    for node in &proj.nodes {
        if node != root && visited.contains(node.as_str()) {
            // Initialize with None (will be set in iteration)
            dom.insert(node.clone(), None);
        }
    }

    // Iterative dominator computation: scale iterations by graph depth
    // BFS depth is bounded by the number of nodes in the reachable subgraph
    let max_iters = (order.len() * 2 / 3).max(10);
    for _iter in 0..max_iters {
        let mut changed = false;
        for node in &order {
            if *node == root {
                continue;
            }
            // Get all predecessors
            let preds: Vec<&String> = proj
                .incoming
                .get(*node)
                .map(|edges| edges.iter().map(|(s, _)| s).collect())
                .unwrap_or_default();

            if preds.is_empty() {
                continue;
            }

            // Intersect dominators of all predecessors
            let mut idom: Option<String> = None;
            for pred in preds {
                if let Some(Some(_)) = dom.get(pred) {
                    if idom.is_none() {
                        idom = dom.get(pred).cloned().flatten();
                    } else {
                        // Intersect: find LCA
                        idom = intersect(&dom, pred, &idom.unwrap());
                    }
                }
            }
            let current = dom.get(*node).cloned().flatten();
            if idom != current {
                dom.insert((node).to_string(), idom);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    dom
}

/// Helper for dominator intersection.
fn intersect(
    dom: &HashMap<String, Option<String>>,
    finger1: &str,
    finger2: &str,
) -> Option<String> {
    // Simple approach: walk up dominator tree from both fingers
    use std::collections::BTreeSet;
    let mut f1 = finger1.to_string();
    let mut f2 = finger2.to_string();

    let mut ancestors1 = BTreeSet::new();
    while let Some(parent) = dom.get(&f1).cloned().flatten() {
        if !ancestors1.insert(f1.clone()) {
            break;
        }
        f1 = parent;
    }
    ancestors1.insert(f1);

    while !ancestors1.contains(&f2) {
        if let Some(parent) = dom.get(&f2).cloned().flatten() {
            f2 = parent;
        } else {
            return Some(finger1.to_string());
        }
    }
    Some(f2)
}

/// BFS from root following outgoing edges (inverse of reverse_reachable).
fn reverse_reachable_inverse(proj: &DirectedProjection, root: &str) -> Vec<String> {
    let mut visited = std::collections::BTreeSet::<&str>::new();
    let mut queue = std::collections::VecDeque::new();
    queue.push_back(root);
    visited.insert(root);

    while let Some(current) = queue.pop_front() {
        if let Some(edges) = proj.outgoing.get(current) {
            for (tgt, _) in edges {
                if visited.insert(tgt) {
                    queue.push_back(tgt);
                }
            }
        }
    }

    visited.into_iter().map(|s| s.to_string()).collect()
}

/// Generate a chokepoints report combining multiple ranking signals.
/// Returns a ranked list of nodes with scores for each signal.
pub fn chokepoint_report(
    proj: &DirectedProjection,
    _graph: &GrapheniumGraph,
    page_rank_ranks: &HashMap<String, f64>,
) -> Vec<ChokepointEntry> {
    let mut entries: Vec<ChokepointEntry> = Vec::new();

    for node in &proj.nodes {
        let pr = page_rank_ranks.get(node).copied().unwrap_or(0.0);
        let out_degree = proj.outgoing.get(node).map(|e| e.len()).unwrap_or(0);
        let in_degree = proj.incoming.get(node).map(|e| e.len()).unwrap_or(0);

        entries.push(ChokepointEntry {
            node_id: node.clone(),
            page_rank: pr,
            out_degree,
            in_degree,
            combined_score: pr * (1.0 + (out_degree + in_degree) as f64 * 0.1),
        });
    }

    entries.sort_unstable_by(|a, b| {
        b.combined_score
            .partial_cmp(&a.combined_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    entries
}

/// A single entry in the chokepoint report.
#[derive(Debug, Clone)]
pub struct ChokepointEntry {
    pub node_id: String,
    pub page_rank: f64,
    pub out_degree: usize,
    pub in_degree: usize,
    pub combined_score: f64,
}

/// Compute reverse reachability: all nodes that can reach `target`.
/// Uses BFS over the reverse graph (following incoming edges).
pub fn reverse_reachable(proj: &DirectedProjection, target: &str) -> Vec<String> {
    let mut visited = std::collections::BTreeSet::<&str>::new();
    let mut queue = std::collections::VecDeque::new();
    queue.push_back(target);
    visited.insert(target);

    while let Some(current) = queue.pop_front() {
        if let Some(in_edges) = proj.incoming.get(current) {
            for (src, _) in in_edges {
                if visited.insert(src) {
                    queue.push_back(src);
                }
            }
        }
    }

    let mut result: Vec<String> = visited
        .into_iter()
        .filter(|id| *id != target)
        .map(|s| s.to_string())
        .collect();
    result.sort();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Edge, FileType, Node};

    fn make_graph() -> GrapheniumGraph {
        let mut g = GrapheniumGraph::new();
        g.upsert_node(Node::new("a", "A", FileType::Code, "src/a.rs"));
        g.upsert_node(Node::new("b", "B", FileType::Code, "src/b.rs"));
        g.upsert_node(Node::new("c", "C", FileType::Code, "src/c.rs"));
        g.upsert_node(Node::new("d", "D", FileType::Code, "src/d.rs"));

        // a -> b -> c (a calls b, b calls c)
        // a -> d (a imports d)
        let mut e1 = Edge::extracted("a", "b", "calls", "src/a.rs");
        e1.src_original = "a".into();
        e1.tgt_original = "b".into();
        g.add_edge(e1);

        let mut e2 = Edge::extracted("b", "c", "calls", "src/b.rs");
        e2.src_original = "b".into();
        e2.tgt_original = "c".into();
        g.add_edge(e2);

        let mut e3 = Edge::extracted("a", "d", "imports", "src/a.rs");
        e3.src_original = "a".into();
        e3.tgt_original = "d".into();
        g.add_edge(e3);

        g
    }

    #[test]
    fn projection_builds_from_graph() {
        let g = make_graph();
        let proj = DirectedProjection::from_graph(&g, None);
        assert_eq!(proj.node_count(), 4);
        assert!(proj.edge_count() >= 3);
    }

    #[test]
    fn projection_respects_relation_filter() {
        let g = make_graph();
        let proj = DirectedProjection::from_graph(&g, Some(&["calls"]));
        // Only 2 call edges (a->b, b->c) and 3 nodes (a, b, c)
        assert_eq!(proj.node_count(), 3);
        assert_eq!(proj.edge_count(), 2);
    }

    #[test]
    fn page_rank_ranks_nodes() {
        let g = make_graph();
        let proj = DirectedProjection::from_graph(&g, None);
        let ranks = page_rank(&proj, 0.85, 20);
        assert_eq!(ranks.len(), 4);
        // a is a source, c is a sink
        assert!(ranks["a"] > 0.0);
        assert!(ranks["c"] > 0.0);
    }

    #[test]
    fn reverse_reachable_from_sink() {
        let g = make_graph();
        let proj = DirectedProjection::from_graph(&g, Some(&["calls"]));
        let reachable = reverse_reachable(&proj, "c");
        assert!(reachable.contains(&"a".to_string()));
        assert!(reachable.contains(&"b".to_string()));
        assert_eq!(reachable.len(), 2);
    }

    #[test]
    fn reverse_reachable_from_source() {
        let g = make_graph();
        let proj = DirectedProjection::from_graph(&g, None);
        let reachable = reverse_reachable(&proj, "a");
        assert!(reachable.is_empty());
    }
}
