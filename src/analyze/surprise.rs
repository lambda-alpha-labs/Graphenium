/// Surprise-scoring for edges.
/// An edge is "surprising" when it connects entities that we would not
/// normally expect to be related.  The score is:
/// ```text
/// base  = conf_bonus                      // AMBIGUOUS=3, INFERRED=2, EXTRACTED=1
///       + (cross_file_type  ? 2 : 0)      // code ↔ paper is more surprising
///       + (cross_repo       ? 2 : 0)      // different top-level directory
///       + (cross_community  ? 1 : 0)      // different Louvain community
/// score = base × (1.5 if semantically_similar_to else 1.0)
///       + (peripheral_to_hub ? 1 : 0)     // low-degree node → high-degree node
/// ```
use std::path::Path;

use crate::model::graph::GrapheniumGraph;

// ── Public types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SurprisingEdge {
    pub source: String,
    pub target: String,
    pub relation: String,
    pub score: f64,
    /// Human-readable explanation of which factors contributed.
    pub reasons: Vec<String>,
}

// ── Helpers ────────────────────────────────────────────────────────────────────

/// Returns the first path component, used to group files by top-level directory.
fn top_dir(source_file: &str) -> &str {
    Path::new(source_file)
        .components()
        .next()
        .and_then(|c| c.as_os_str().to_str())
        .unwrap_or("")
}

// ── Public API ─────────────────────────────────────────────────────────────────

/// Score every edge in the graph and return the `top_n` most surprising ones,
/// sorted by score descending.
pub fn surprising_connections(graph: &GrapheniumGraph, top_n: usize) -> Vec<SurprisingEdge> {
    let mut scored: Vec<SurprisingEdge> = graph
        .edges_with_endpoints()
        .filter_map(|(src_id, tgt_id, edge)| {
            let src = graph.node_data(src_id)?;
            let tgt = graph.node_data(tgt_id)?;
            let mut reasons = Vec::new();

            let conf_bonus = edge.confidence.surprise_bonus() as f64;
            if conf_bonus > 1.0 {
                reasons.push(format!("{} confidence", edge.confidence));
            }

            let cross_ft = src.file_type != tgt.file_type;
            if cross_ft {
                reasons.push(format!("{} ↔ {}", src.file_type, tgt.file_type));
            }

            let cross_repo = top_dir(&src.source_file) != top_dir(&tgt.source_file);
            if cross_repo {
                reasons.push("cross-repo".to_string());
            }

            let cross_comm = matches!((src.community, tgt.community), (Some(a), Some(b)) if a != b);
            if cross_comm {
                reasons.push("cross-community".to_string());
            }

            let is_semantic = edge.relation == "semantically_similar_to";
            if is_semantic {
                reasons.push("semantic similarity".to_string());
            }

            let src_deg = graph.degree(src_id);
            let tgt_deg = graph.degree(tgt_id);
            let periph = src_deg.min(tgt_deg) <= 2 && src_deg.max(tgt_deg) >= 5;
            if periph {
                reasons.push("peripheral→hub".to_string());
            }

            let base = conf_bonus
                + if cross_ft { 2.0 } else { 0.0 }
                + if cross_repo { 2.0 } else { 0.0 }
                + if cross_comm { 1.0 } else { 0.0 };
            let score =
                base * (if is_semantic { 1.5 } else { 1.0 }) + if periph { 1.0 } else { 0.0 };

            Some(SurprisingEdge {
                source: src_id.to_string(),
                target: tgt_id.to_string(),
                relation: edge.relation.clone(),
                score,
                reasons,
            })
        })
        .collect();

    scored.sort_unstable_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored.truncate(top_n);
    scored
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Confidence, Edge, FileType, Node};

    fn make_graph() -> GrapheniumGraph {
        let mut g = GrapheniumGraph::new();
        let mut a = Node::new("src_a", "A", FileType::Code, "src/a.rs");
        a.community = Some(0);
        g.upsert_node(a);
        let mut b = Node::new("papers_b", "B", FileType::Paper, "papers/b.pdf");
        b.community = Some(1);
        g.upsert_node(b);
        let mut c = Node::new("src_c", "C", FileType::Code, "src/c.rs");
        c.community = Some(0);
        g.upsert_node(c);
        g
    }

    #[test]
    fn cross_type_repo_community_scores_high() {
        let mut g = make_graph();
        // AMBIGUOUS(3) + cross_ft(2) + cross_repo(2) + cross_comm(1) = 8.0
        g.add_edge(Edge::new(
            "src_a",
            "papers_b",
            "references",
            Confidence::Ambiguous,
            "src/a.rs",
        ));
        let results = surprising_connections(&g, 10);
        assert!(!results.is_empty());
        let top = &results[0];
        assert!((top.score - 8.0).abs() < 1e-9);
        assert!(top.reasons.contains(&"cross-repo".to_string()));
        assert!(top.reasons.contains(&"cross-community".to_string()));
    }

    #[test]
    fn extracted_same_community_baseline() {
        let mut g = make_graph();
        // EXTRACTED(1), no other factors → score = 1.0
        g.add_edge(Edge::extracted("src_a", "src_c", "calls", "src/a.rs"));
        let results = surprising_connections(&g, 10);
        let e = results.iter().find(|e| e.source == "src_a").unwrap();
        assert_eq!(e.score, 1.0);
    }

    #[test]
    fn semantic_similarity_multiplies_score() {
        let mut g = make_graph();
        // INFERRED(2) + cross_comm(1) = 3 × 1.5 = 4.5
        let mut edge = Edge::new(
            "src_a",
            "papers_b",
            "semantically_similar_to",
            Confidence::Inferred,
            "src/a.rs",
        );
        edge.relation = "semantically_similar_to".to_string();
        g.add_edge(edge);
        let results = surprising_connections(&g, 10);
        let top = &results[0];
        // INFERRED(2) + cross_ft(2) + cross_repo(2) + cross_comm(1) = 7 × 1.5 = 10.5
        assert!(top.score > 10.0);
    }

    #[test]
    fn empty_graph_returns_empty() {
        let g = GrapheniumGraph::new();
        assert!(surprising_connections(&g, 10).is_empty());
    }
}
