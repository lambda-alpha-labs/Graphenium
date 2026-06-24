/// Suggested-question generation.
///
/// We generate questions from five sources:
///
/// 1. **AMBIGUOUS edges** — the relationship needs manual review.
/// 2. **Bridge nodes** — nodes with high betweenness centrality (Brandes'
///    algorithm, capped at 5 000 nodes).
/// 3. **God nodes with INFERRED edges** — high-degree nodes whose connections
///    were not explicitly stated in source.
/// 4. **Isolated nodes** — nodes with no edges at all.
/// 5. **Low-cohesion communities** — communities with cohesion < 0.15.
use std::collections::{HashMap, VecDeque};

use crate::cluster::CommunityStats;
use crate::model::graph::GrapheniumGraph;
use crate::model::Confidence;

// ── Public types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SuggestedQuestion {
    pub question: String,
    pub reason: String,
    pub node_ids: Vec<String>,
}

// ── Public API ────────────────────────────────────────────────────────────────

pub fn suggest_questions(
    graph: &GrapheniumGraph,
    community_stats: &[CommunityStats],
) -> Vec<SuggestedQuestion> {
    let mut questions = Vec::new();
    ambiguous_edge_questions(graph, &mut questions);
    bridge_node_questions(graph, &mut questions);
    god_node_inferred_questions(graph, &mut questions);
    isolated_node_questions(graph, &mut questions);
    low_cohesion_questions(community_stats, &mut questions);
    questions
}

// ── Question sources ──────────────────────────────────────────────────────────

fn ambiguous_edge_questions(graph: &GrapheniumGraph, out: &mut Vec<SuggestedQuestion>) {
    let mut count = 0;
    for (src_id, tgt_id, edge) in graph.edges_with_endpoints() {
        if count >= 5 {
            break;
        }
        if edge.confidence != Confidence::Ambiguous {
            continue;
        }
        let src_label = graph.node_data(src_id).map_or(src_id, |n| n.label.as_str());
        let tgt_label = graph.node_data(tgt_id).map_or(tgt_id, |n| n.label.as_str());
        out.push(SuggestedQuestion {
            question: format!("What is the relationship between `{src_label}` and `{tgt_label}`?"),
            reason: format!(
                "Edge `{}` is AMBIGUOUS — manual review needed.",
                edge.relation
            ),
            node_ids: vec![src_id.to_string(), tgt_id.to_string()],
        });
        count += 1;
    }
}

fn bridge_node_questions(graph: &GrapheniumGraph, out: &mut Vec<SuggestedQuestion>) {
    let mut centrality: Vec<(String, f64)> = betweenness_centrality(graph).into_iter().collect();
    centrality.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    for (id, score) in centrality.iter().take(3) {
        if *score < 0.01 {
            break;
        }
        let label = graph
            .node_data(id)
            .map_or(id.as_str(), |n| n.label.as_str());
        out.push(SuggestedQuestion {
            question: format!(
                "Is `{label}` a bottleneck? Removing it could disconnect parts of the graph."
            ),
            reason: format!("High betweenness centrality ({score:.3})."),
            node_ids: vec![id.clone()],
        });
    }
}

fn god_node_inferred_questions(graph: &GrapheniumGraph, out: &mut Vec<SuggestedQuestion>) {
    let mut by_degree: Vec<(&str, usize)> =
        graph.node_ids().map(|id| (id, graph.degree(id))).collect();
    by_degree.sort_unstable_by(|a, b| b.1.cmp(&a.1));

    let mut count = 0;
    for &(id, _deg) in by_degree.iter().take(20) {
        if count >= 3 {
            break;
        }
        let inferred_neighbors: Vec<String> = graph
            .node_edges(id)
            .into_iter()
            .filter(|(_, e)| e.confidence == Confidence::Inferred)
            .map(|(nbr, _)| {
                graph
                    .node_data(nbr)
                    .map_or_else(|| nbr.to_string(), |n| n.label.clone())
            })
            .take(2)
            .collect();
        if inferred_neighbors.is_empty() {
            continue;
        }
        let label = graph.node_data(id).map_or(id, |n| n.label.as_str());
        out.push(SuggestedQuestion {
            question: format!(
                "How does `{label}` interact with {}?",
                inferred_neighbors.join(", ")
            ),
            reason: format!(
                "`{label}` is highly connected with INFERRED relationships that weren't \
                 explicitly stated in source."
            ),
            node_ids: vec![id.to_string()],
        });
        count += 1;
    }
}

fn isolated_node_questions(graph: &GrapheniumGraph, out: &mut Vec<SuggestedQuestion>) {
    let mut count = 0;
    for node in graph.nodes() {
        if count >= 3 {
            break;
        }
        if graph.degree(&node.id) == 0 {
            out.push(SuggestedQuestion {
                question: format!(
                    "Why is `{}` isolated? Does it connect to anything outside this corpus?",
                    node.label
                ),
                reason: "Node has no edges.".to_string(),
                node_ids: vec![node.id.clone()],
            });
            count += 1;
        }
    }
}

fn low_cohesion_questions(stats: &[CommunityStats], out: &mut Vec<SuggestedQuestion>) {
    for s in stats.iter().filter(|s| s.size >= 3 && s.cohesion < 0.15) {
        out.push(SuggestedQuestion {
            question: format!(
                "Community {} has low internal connectivity ({:.0}%). \
                 Should its {} members be reorganized?",
                s.community_id,
                s.cohesion * 100.0,
                s.size,
            ),
            reason: format!("Cohesion {:.3} < 0.15 threshold.", s.cohesion),
            node_ids: s.members.clone(),
        });
    }
}

// ── Brandes betweenness centrality ────────────────────────────────────────────

/// Compute normalized betweenness centrality for all nodes using Brandes'
/// algorithm (O(V·E)).  For graphs larger than 5 000 nodes, only the first
/// 5 000 (by insertion order) are included.
fn betweenness_centrality(graph: &GrapheniumGraph) -> HashMap<String, f64> {
    let all_ids: Vec<String> = graph.node_ids().map(|s| s.to_string()).collect();
    let ids: &[String] = if all_ids.len() > 5000 {
        &all_ids[..5000]
    } else {
        &all_ids
    };
    let n = ids.len();
    if n < 3 {
        return ids.iter().map(|id| (id.clone(), 0.0)).collect();
    }

    let id_to_idx: HashMap<&str, usize> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), i))
        .collect();

    // Build adjacency by index for fast BFS.
    let adj: Vec<Vec<usize>> = ids
        .iter()
        .map(|id| {
            graph
                .neighbor_ids(id)
                .into_iter()
                .filter_map(|nbr| id_to_idx.get(nbr.as_str()).copied())
                .collect()
        })
        .collect();

    let mut betweenness = vec![0.0f64; n];

    for s in 0..n {
        let mut stack = Vec::<usize>::new();
        let mut pred = vec![Vec::<usize>::new(); n];
        let mut sigma = vec![0.0f64; n];
        sigma[s] = 1.0;
        let mut dist = vec![-1i64; n];
        dist[s] = 0;
        let mut queue = VecDeque::new();
        queue.push_back(s);

        // BFS
        while let Some(v) = queue.pop_front() {
            stack.push(v);
            for &w in &adj[v] {
                if dist[w] < 0 {
                    queue.push_back(w);
                    dist[w] = dist[v] + 1;
                }
                if dist[w] == dist[v] + 1 {
                    sigma[w] += sigma[v];
                    pred[w].push(v);
                }
            }
        }

        // Back-propagation
        let mut delta = vec![0.0f64; n];
        while let Some(w) = stack.pop() {
            for &v in &pred[w] {
                if sigma[w] > 0.0 {
                    delta[v] += (sigma[v] / sigma[w]) * (1.0 + delta[w]);
                }
            }
            if w != s {
                betweenness[w] += delta[w];
            }
        }
    }

    // Normalize by (n-1)(n-2)/2 for undirected graphs.
    let norm = ((n - 1) * (n - 2)) as f64 / 2.0;
    ids.iter()
        .enumerate()
        .map(|(i, id)| (id.clone(), betweenness[i] / norm))
        .collect()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Confidence, Edge, FileType, Node};

    fn chain_graph() -> GrapheniumGraph {
        // a – b – c – d  (b and c are bridges)
        let mut g = GrapheniumGraph::new();
        for id in &["a", "b", "c", "d"] {
            g.upsert_node(Node::new(*id, *id, FileType::Code, "f.rs"));
        }
        g.add_edge(Edge::extracted("a", "b", "calls", "f.rs"));
        g.add_edge(Edge::extracted("b", "c", "calls", "f.rs"));
        g.add_edge(Edge::extracted("c", "d", "calls", "f.rs"));
        g
    }

    #[test]
    fn bridge_nodes_detected() {
        let g = chain_graph();
        let mut qs = Vec::new();
        bridge_node_questions(&g, &mut qs);
        assert!(!qs.is_empty(), "should find at least one bridge");
    }

    #[test]
    fn isolated_node_detected() {
        let mut g = chain_graph();
        g.upsert_node(Node::new("x", "X", FileType::Code, "f.rs"));
        let mut qs = Vec::new();
        isolated_node_questions(&g, &mut qs);
        assert!(qs.iter().any(|q| q.node_ids.contains(&"x".to_string())));
    }

    #[test]
    fn ambiguous_edge_generates_question() {
        let mut g = chain_graph();
        g.add_edge(Edge::new("a", "d", "uses", Confidence::Ambiguous, "f.rs"));
        let mut qs = Vec::new();
        ambiguous_edge_questions(&g, &mut qs);
        assert!(!qs.is_empty());
        assert!(qs[0].reason.contains("AMBIGUOUS"));
    }

    #[test]
    fn low_cohesion_generates_question() {
        let stats = vec![CommunityStats {
            community_id: 0,
            size: 5,
            internal_edges: 0,
            cohesion: 0.0,
            members: vec!["a".into(), "b".into(), "c".into(), "d".into(), "e".into()],
            focus: None,
        }];
        let mut qs = Vec::new();
        low_cohesion_questions(&stats, &mut qs);
        assert_eq!(qs.len(), 1);
    }

    #[test]
    fn suggest_questions_does_not_panic() {
        let g = chain_graph();
        let _ = suggest_questions(&g, &[]);
    }
}
