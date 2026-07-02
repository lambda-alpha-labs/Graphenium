//! Optional embedding-based retrieval for Graphenium.
//!
//! Provides two embedding approaches:
//! - **Text embeddings** (Phase 4.4): term-frequency vectors from node labels.
//! - **Node2Vec embeddings** (Phase 4.5): structural embeddings from random walks.
//!
//! Both are purely algorithmic with no external ML dependencies.

use std::collections::HashMap;

use crate::analyze::rank::DirectedProjection;
use crate::model::GrapheniumGraph;

// ── Text embedding (TF-IDF-like) ──────────────────────────────────────────────

/// A simple term-frequency vector for text similarity.
#[derive(Debug, Clone)]
pub struct TfVector {
    pub node_id: String,
    pub terms: HashMap<String, f64>,
}

/// Build term-frequency vectors for all nodes by tokenizing labels.
/// Terms are lowercased, split on non-alphanumeric characters, and weighted
/// by frequency within each node's label + qualified_label.
pub fn build_text_embeddings(graph: &GrapheniumGraph) -> Vec<TfVector> {
    let mut vectors = Vec::new();

    for node in graph.nodes() {
        let mut terms: HashMap<String, f64> = HashMap::new();
        let text = format!(
            "{} {} {}",
            node.label,
            node.qualified_label.as_deref().unwrap_or(""),
            node.source_file
        );

        for token in text.split(|c: char| !c.is_alphanumeric()) {
            let t = token.to_lowercase();
            if t.len() >= 2 && !is_stop_word(&t) {
                *terms.entry(t).or_insert(0.0) += 1.0;
            }
        }

        // Normalize by total term count (TF normalization)
        let total: f64 = terms.values().sum();
        if total > 0.0 {
            for v in terms.values_mut() {
                *v /= total;
            }
        }

        if !terms.is_empty() {
            vectors.push(TfVector {
                node_id: node.id.clone(),
                terms,
            });
        }
    }

    vectors
}

/// Compute cosine similarity between two TF vectors.
pub fn cosine_similarity(a: &TfVector, b: &TfVector) -> f64 {
    let mut dot = 0.0;
    let mut mag_a = 0.0;
    let mut mag_b = 0.0;

    for (term, weight_a) in &a.terms {
        mag_a += weight_a * weight_a;
        if let Some(weight_b) = b.terms.get(term) {
            dot += weight_a * weight_b;
        }
    }
    for (_, weight_b) in &b.terms {
        mag_b += weight_b * weight_b;
    }

    let denom = mag_a.sqrt() * mag_b.sqrt();
    if denom == 0.0 {
        0.0
    } else {
        dot / denom
    }
}

fn is_stop_word(word: &str) -> bool {
    matches!(
        word,
        "the"
            | "a"
            | "an"
            | "and"
            | "or"
            | "in"
            | "on"
            | "at"
            | "to"
            | "for"
            | "of"
            | "is"
            | "it"
            | "as"
            | "by"
            | "with"
            | "from"
            | "this"
            | "that"
            | "was"
            | "are"
            | "be"
            | "has"
            | "had"
            | "not"
            | "but"
            | "what"
            | "all"
            | "when"
            | "where"
            | "how"
            | "which"
    )
}

/// Search for nodes whose text embeddings are most similar to a query.
/// Returns node IDs sorted by similarity descending.
pub fn search_by_text(graph: &GrapheniumGraph, query: &str, top_k: usize) -> Vec<(String, f64)> {
    let vectors = build_text_embeddings(graph);

    // Build query vector
    let mut query_terms: HashMap<String, f64> = HashMap::new();
    for token in query.split(|c: char| !c.is_alphanumeric()) {
        let t = token.to_lowercase();
        if t.len() >= 2 && !is_stop_word(&t) {
            *query_terms.entry(t).or_insert(0.0) += 1.0;
        }
    }
    let total: f64 = query_terms.values().sum();
    if total > 0.0 {
        for v in query_terms.values_mut() {
            *v /= total;
        }
    }

    let query_vec = TfVector {
        node_id: String::new(),
        terms: query_terms,
    };

    let mut similarities: Vec<(String, f64)> = vectors
        .iter()
        .map(|v| (v.node_id.clone(), cosine_similarity(&query_vec, v)))
        .filter(|(_, score)| *score > 0.0)
        .collect();

    similarities
        .sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    similarities.truncate(top_k);
    similarities
}

// ── Node2Vec (structural embeddings) ──────────────────────────────────────────

/// A structural embedding vector for a node.
#[derive(Debug, Clone)]
pub struct Node2VecEmbedding {
    pub node_id: String,
    pub vector: Vec<f64>,
}

/// Generate random walks on the directed graph.
fn random_walks(
    proj: &DirectedProjection,
    walks_per_node: usize,
    walk_length: usize,
) -> Vec<Vec<String>> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut walks = Vec::new();

    for _ in 0..walks_per_node {
        for start in &proj.nodes {
            let mut walk = Vec::with_capacity(walk_length);
            walk.push(start.clone());
            let mut current = start.clone();

            for _step in 1..walk_length {
                if let Some(out_edges) = proj.outgoing.get(&current) {
                    if out_edges.is_empty() {
                        break;
                    }
                    let idx = rng.gen_range(0..out_edges.len());
                    let (next, _) = &out_edges[idx];
                    walk.push(next.clone());
                    current = next.clone();
                } else {
                    break;
                }
            }

            if walk.len() >= 2 {
                walks.push(walk);
            }
        }
    }

    walks
}

/// Train Node2Vec embeddings using random walks and co-occurrence counting.
/// This is a simplified approach: nodes that frequently co-appear in walks
/// get similar embeddings. The embedding dimension is controlled by `dim`.
pub fn train_node2vec(
    proj: &DirectedProjection,
    dim: usize,
    walks_per_node: usize,
    walk_length: usize,
    iterations: usize,
) -> Vec<Node2VecEmbedding> {
    let walks = random_walks(proj, walks_per_node, walk_length);

    // Build co-occurrence counts: how often each pair appears within distance 5
    let mut cooccur: HashMap<(String, String), f64> = HashMap::new();
    for walk in &walks {
        for (i, node) in walk.iter().enumerate() {
            let window = 3;
            for j in (i.saturating_sub(window))..=(i + window).min(walk.len() - 1) {
                if i != j {
                    *cooccur
                        .entry((node.clone(), walk[j].clone()))
                        .or_insert(0.0) += 1.0;
                }
            }
        }
    }

    // Initialize random vectors
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut vectors: HashMap<String, Vec<f64>> = proj
        .nodes
        .iter()
        .map(|n| {
            let v: Vec<f64> = (0..dim).map(|_| rng.gen_range(-0.5..0.5)).collect();
            (n.clone(), v)
        })
        .collect();

    // Simplified training: push vectors together for co-occurring pairs
    let learning_rate = 0.01;
    for _iter in 0..iterations {
        // Collect all updates first, then apply them
        let mut updates: Vec<(String, String, Vec<f64>)> = Vec::new();
        for ((n1, n2), count) in &cooccur {
            let weight = count.min(10.0);
            if let (Some(v1), Some(v2)) = (vectors.get(n1), vectors.get(n2)) {
                let diff: Vec<f64> = v1
                    .iter()
                    .zip(v2.iter())
                    .map(|(a, b)| (a - b) * learning_rate * weight)
                    .collect();
                updates.push((n1.to_string(), n2.to_string(), diff));
            }
        }

        // Apply updates
        for (n1, n2, diff) in &updates {
            if let Some(v1) = vectors.get_mut(n1) {
                for (v, d) in v1.iter_mut().zip(diff.iter()) {
                    *v -= d;
                }
            }
            if let Some(v2) = vectors.get_mut(n2) {
                for (v, d) in v2.iter_mut().zip(diff.iter()) {
                    *v += d;
                }
            }
        }
    }

    // Normalize vectors
    for (_id, vec) in vectors.iter_mut() {
        let mag: f64 = vec.iter().map(|v| v * v).sum::<f64>().sqrt();
        if mag > 0.0 {
            for v in vec.iter_mut() {
                *v /= mag;
            }
        }
    }

    vectors
        .into_iter()
        .map(|(id, vec)| Node2VecEmbedding {
            node_id: id.to_string(),
            vector: vec,
        })
        .collect()
}

/// Find nodes structurally similar to a target using Node2Vec embeddings.
pub fn find_similar_structural(
    embeddings: &[Node2VecEmbedding],
    target_id: &str,
    top_k: usize,
) -> Vec<(String, f64)> {
    let target = match embeddings.iter().find(|e| e.node_id == target_id) {
        Some(t) => t,
        None => return Vec::new(),
    };

    let mut similarities: Vec<(String, f64)> = embeddings
        .iter()
        .filter(|e| e.node_id != target_id)
        .map(|e| {
            let dot: f64 = target
                .vector
                .iter()
                .zip(e.vector.iter())
                .map(|(a, b)| a * b)
                .sum();
            (e.node_id.clone(), dot)
        })
        .collect();

    similarities
        .sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    similarities.truncate(top_k);
    similarities
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Edge, FileType, Node};

    fn make_graph() -> GrapheniumGraph {
        let mut g = GrapheniumGraph::new();
        g.upsert_node(Node::new(
            "auth_login",
            "AuthLogin",
            FileType::Code,
            "src/auth.rs",
        ));
        g.upsert_node(Node::new(
            "auth_logout",
            "AuthLogout",
            FileType::Code,
            "src/auth.rs",
        ));
        g.upsert_node(Node::new(
            "db_query",
            "DBQuery",
            FileType::Code,
            "src/db.rs",
        ));
        g.upsert_node(Node::new(
            "db_connect",
            "DBConnect",
            FileType::Code,
            "src/db.rs",
        ));
        g.add_edge(Edge::extracted(
            "auth_login",
            "auth_logout",
            "imports",
            "src/auth.rs",
        ));
        g.add_edge(Edge::extracted(
            "auth_login",
            "db_query",
            "calls",
            "src/auth.rs",
        ));
        g.add_edge(Edge::extracted(
            "db_query",
            "db_connect",
            "calls",
            "src/db.rs",
        ));
        g
    }

    #[test]
    fn text_embeddings_build_from_labels() {
        let g = make_graph();
        let vecs = build_text_embeddings(&g);
        assert_eq!(vecs.len(), 4);
        let auth = vecs.iter().find(|v| v.node_id == "auth_login").unwrap();
        assert!(auth.terms.contains_key("authlogin"));
    }

    #[test]
    fn text_search_finds_similar() {
        let g = make_graph();
        let results = search_by_text(&g, "auth login", 2);
        assert!(!results.is_empty());
    }

    #[test]
    fn node2vec_trains_and_finds_similar() {
        let g = make_graph();
        let proj = DirectedProjection::from_graph(&g, None);
        let embs = train_node2vec(&proj, 4, 2, 5, 5);
        assert!(!embs.is_empty());

        if let Some(target) = embs.first() {
            let similar = find_similar_structural(&embs, &target.node_id, 2);
            assert!(similar.len() <= 2);
        }
    }
}
