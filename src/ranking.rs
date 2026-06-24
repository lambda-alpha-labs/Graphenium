use std::cmp::Ordering;
use std::collections::HashSet;
use std::path::Path;

use crate::model::{GrapheniumGraph, Node};

#[derive(Debug, Clone, PartialEq)]
pub struct RankedNode {
    pub id: String,
    pub score: f64,
    pub matched_keywords: Vec<String>,
    pub matched_fields: Vec<String>,
    pub fallback_reason: Option<String>,
}

impl RankedNode {
    pub fn is_direct_match(&self) -> bool {
        !self.matched_keywords.is_empty()
    }
}

const FRAMEWORK_LABELS: &[&str] = &[
    "collections",
    "componentmodel",
    "configuration",
    "diagnostics",
    "drawing",
    "forms",
    "generic",
    "http",
    "io",
    "json",
    "linq",
    "net",
    "runtime",
    "security",
    "serialization",
    "system",
    "tasks",
    "text",
    "threading",
    "web",
    "windows",
    "xml",
];

/// Score every node by how many keywords from `query` appear in its label,
/// ID, or source_file (case-insensitive), while down-ranking generic
/// framework/import noise.
pub fn score_query_nodes(graph: &GrapheniumGraph, query: &str) -> Vec<(String, f64)> {
    score_query_nodes_detailed(graph, query)
        .into_iter()
        .map(|node| (node.id, node.score))
        .collect()
}

/// Detailed variant of [`score_query_nodes`] with match explanations.
pub fn score_query_nodes_detailed(graph: &GrapheniumGraph, query: &str) -> Vec<RankedNode> {
    score_query_nodes_detailed_in_scope(graph, query, None)
}

/// Scoped variant of [`score_query_nodes`] that only considers nodes contained
/// in `allowed` when provided.
pub fn score_query_nodes_in_scope(
    graph: &GrapheniumGraph,
    query: &str,
    allowed: Option<&HashSet<String>>,
) -> Vec<(String, f64)> {
    score_query_nodes_detailed_in_scope(graph, query, allowed)
        .into_iter()
        .map(|node| (node.id, node.score))
        .collect()
}

/// Scoped detailed ranking with concise match explanations.
pub fn score_query_nodes_detailed_in_scope(
    graph: &GrapheniumGraph,
    query: &str,
    allowed: Option<&HashSet<String>>,
) -> Vec<RankedNode> {
    let keywords = parse_keywords(query);

    let mut scored: Vec<RankedNode> = graph
        .node_ids()
        .filter_map(|id| {
            if !node_allowed(id, allowed) {
                return None;
            }
            let node = graph.node_data(id)?;
            let label = node.label.to_lowercase();
            let node_id = id.to_lowercase();
            let source_file = node.source_file.to_lowercase();

            let label_matches = collect_matches(&keywords, &label);
            let id_matches = collect_matches(&keywords, &node_id);
            let path_matches = collect_matches(&keywords, &source_file);
            let matched_keywords = merge_keywords([&label_matches, &id_matches, &path_matches]);
            let raw_score = matched_keywords.len() as f64;

            if raw_score <= 0.0 {
                return None;
            }

            Some(RankedNode {
                id: id.to_string(),
                score: raw_score * query_rank_multiplier(graph, node),
                matched_keywords,
                matched_fields: matched_fields(&label_matches, &id_matches, &path_matches),
                fallback_reason: None,
            })
        })
        .collect();

    sort_ranked_nodes(graph, allowed, &mut scored);

    if scored.is_empty() {
        return top_degree_nodes_detailed_in_scope(graph, 5, allowed);
    }

    scored
}

/// Return the top nodes by degree, suppressing framework/import noise by default.
pub fn top_degree_nodes(graph: &GrapheniumGraph, limit: usize) -> Vec<(String, f64)> {
    top_degree_nodes_detailed(graph, limit)
        .into_iter()
        .map(|node| (node.id, node.score))
        .collect()
}

/// Detailed variant of [`top_degree_nodes`].
pub fn top_degree_nodes_detailed(graph: &GrapheniumGraph, limit: usize) -> Vec<RankedNode> {
    top_degree_nodes_detailed_in_scope(graph, limit, None)
}

/// Scoped variant of [`top_degree_nodes`] that only considers nodes contained
/// in `allowed` when provided.
pub fn top_degree_nodes_in_scope(
    graph: &GrapheniumGraph,
    limit: usize,
    allowed: Option<&HashSet<String>>,
) -> Vec<(String, f64)> {
    top_degree_nodes_detailed_in_scope(graph, limit, allowed)
        .into_iter()
        .map(|node| (node.id, node.score))
        .collect()
}

/// Scoped detailed hotspot ranking used as a fallback when no query terms match.
pub fn top_degree_nodes_detailed_in_scope(
    graph: &GrapheniumGraph,
    limit: usize,
    allowed: Option<&HashSet<String>>,
) -> Vec<RankedNode> {
    let mut ranked: Vec<RankedNode> = graph
        .node_ids()
        .filter_map(|id| {
            if !node_allowed(id, allowed) {
                return None;
            }
            let node = graph.node_data(id)?;
            if is_framework_noise_node(graph, node) {
                return None;
            }
            let degree = degree_in_scope(graph, id, allowed);
            Some((
                id.to_string(),
                degree as f64 * query_rank_multiplier(graph, node),
                degree,
            ))
        })
        .map(|(id, score, degree)| RankedNode {
            id,
            score,
            matched_keywords: Vec::new(),
            matched_fields: Vec::new(),
            fallback_reason: Some(format!("fallback hotspot seed (degree {degree})")),
        })
        .collect();

    if ranked.is_empty() {
        ranked = graph
            .node_ids()
            .filter(|id| node_allowed(id, allowed))
            .map(|id| {
                let degree = degree_in_scope(graph, id, allowed);
                RankedNode {
                    id: id.to_string(),
                    score: degree as f64,
                    matched_keywords: Vec::new(),
                    matched_fields: Vec::new(),
                    fallback_reason: Some(format!("fallback hotspot seed (degree {degree})")),
                }
            })
            .collect();
    }

    sort_ranked_nodes(graph, allowed, &mut ranked);
    ranked.truncate(limit);
    ranked
}

/// Count edges incident to `id` whose other endpoint is also inside `allowed`
/// when a scope is provided.
pub fn degree_in_scope(
    graph: &GrapheniumGraph,
    id: &str,
    allowed: Option<&HashSet<String>>,
) -> usize {
    match allowed {
        Some(allowed) => {
            if !allowed.contains(id) {
                return 0;
            }
            graph
                .node_edges(id)
                .into_iter()
                .filter(|(neighbor_id, _)| allowed.contains(*neighbor_id))
                .count()
        }
        None => graph.degree(id),
    }
}

/// True when the node behaves like a framework namespace or import fragment
/// instead of a repo-specific symbol.
pub fn is_framework_noise_node(graph: &GrapheniumGraph, node: &Node) -> bool {
    if !is_import_only_node(graph, node) {
        return false;
    }

    let file_stem = Path::new(&node.source_file)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    if node.label.eq_ignore_ascii_case(file_stem) {
        return false;
    }

    is_framework_label(&node.label)
}

fn sort_ranked_nodes(
    graph: &GrapheniumGraph,
    allowed: Option<&HashSet<String>>,
    nodes: &mut [RankedNode],
) {
    nodes.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| {
                degree_in_scope(graph, &b.id, allowed).cmp(&degree_in_scope(graph, &a.id, allowed))
            })
            .then_with(|| a.id.cmp(&b.id))
    });
}

fn parse_keywords(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .map(|w| {
            w.to_lowercase()
                .trim_matches(|c: char| !c.is_alphanumeric())
                .to_string()
        })
        .filter(|w| w.len() > 2)
        .collect()
}

fn collect_matches(keywords: &[String], haystack: &str) -> Vec<String> {
    keywords
        .iter()
        .filter(|kw| haystack.contains(kw.as_str()))
        .cloned()
        .collect()
}

fn merge_keywords<'a>(groups: impl IntoIterator<Item = &'a Vec<String>>) -> Vec<String> {
    let mut merged = Vec::new();
    for group in groups {
        for keyword in group {
            if !merged.contains(keyword) {
                merged.push(keyword.clone());
            }
        }
    }
    merged
}

fn matched_fields(label: &[String], id: &[String], path: &[String]) -> Vec<String> {
    let mut fields = Vec::new();
    if !label.is_empty() {
        fields.push("label".to_string());
    }
    if !id.is_empty() {
        fields.push("id".to_string());
    }
    if !path.is_empty() {
        fields.push("path".to_string());
    }
    fields
}

fn node_allowed(id: &str, allowed: Option<&HashSet<String>>) -> bool {
    allowed.is_none_or(|allowed| allowed.contains(id))
}

fn query_rank_multiplier(graph: &GrapheniumGraph, node: &Node) -> f64 {
    if is_framework_noise_node(graph, node) {
        0.2
    } else if is_import_only_node(graph, node) {
        0.5
    } else {
        1.0
    }
}

fn is_import_only_node(graph: &GrapheniumGraph, node: &Node) -> bool {
    let edges = graph.node_edges(&node.id);
    !edges.is_empty()
        && edges
            .iter()
            .all(|(_, edge)| edge.relation.eq_ignore_ascii_case("imports"))
}

fn is_framework_label(label: &str) -> bool {
    let normalized = label.trim().to_lowercase();
    if normalized.is_empty() {
        return false;
    }

    if FRAMEWORK_LABELS.contains(&normalized.as_str()) {
        return true;
    }

    let mut saw_segment = false;
    for segment in normalized.split(['.', ':']) {
        if segment.is_empty() {
            continue;
        }
        saw_segment = true;
        if !FRAMEWORK_LABELS.contains(&segment) {
            return false;
        }
    }

    saw_segment
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Edge, FileType, Node};

    fn build_graph() -> GrapheniumGraph {
        let mut g = GrapheniumGraph::new();
        g.upsert_node(Node::new(
            "system",
            "System",
            FileType::Code,
            "tests/app.cs",
        ));
        g.upsert_node(Node::new(
            "orderservice",
            "OrderService",
            FileType::Code,
            "src/OrderService.cs",
        ));
        g.upsert_node(Node::new(
            "controller",
            "Controller",
            FileType::Code,
            "src/Controller.cs",
        ));
        g.upsert_node(Node::new(
            "worker",
            "Worker",
            FileType::Code,
            "src/Worker.cs",
        ));
        g.upsert_node(Node::new(
            "helper",
            "Helper",
            FileType::Code,
            "src/Helper.cs",
        ));

        g.add_edge(Edge::extracted(
            "controller",
            "system",
            "imports",
            "src/Controller.cs",
        ));
        g.add_edge(Edge::extracted(
            "worker",
            "system",
            "imports",
            "src/Worker.cs",
        ));
        g.add_edge(Edge::extracted(
            "helper",
            "system",
            "imports",
            "src/Helper.cs",
        ));
        g.add_edge(Edge::extracted(
            "controller",
            "orderservice",
            "calls",
            "src/Controller.cs",
        ));
        g.add_edge(Edge::extracted(
            "worker",
            "orderservice",
            "calls",
            "src/Worker.cs",
        ));
        g.add_edge(Edge::extracted(
            "helper",
            "orderservice",
            "uses",
            "src/Helper.cs",
        ));
        g
    }

    #[test]
    fn detects_framework_import_noise() {
        let g = build_graph();
        let node = g.node_data("system").unwrap();
        assert!(is_framework_noise_node(&g, node));
        let service = g.node_data("orderservice").unwrap();
        assert!(!is_framework_noise_node(&g, service));
    }

    #[test]
    fn fallback_degree_ranking_skips_framework_noise() {
        let g = build_graph();
        let ranked = top_degree_nodes(&g, 2);
        assert_eq!(ranked[0].0, "orderservice");
        assert!(ranked.iter().all(|(id, _)| id != "system"));
    }

    #[test]
    fn query_scoring_downranks_framework_matches() {
        let g = build_graph();
        let ranked = score_query_nodes(&g, "system service");
        assert_eq!(ranked[0].0, "orderservice");
    }

    #[test]
    fn detailed_query_scoring_records_match_fields_and_keywords() {
        let g = build_graph();
        let ranked = score_query_nodes_detailed(&g, "service order");
        assert_eq!(ranked[0].id, "orderservice");
        assert!(ranked[0].matched_keywords.contains(&"service".to_string()));
        assert!(ranked[0].matched_fields.contains(&"label".to_string()));
        assert!(ranked[0].is_direct_match());
    }

    #[test]
    fn detailed_query_scoring_uses_fallback_reason_when_no_keywords_match() {
        let g = build_graph();
        let ranked = score_query_nodes_detailed(&g, "xyz");
        assert!(ranked[0].fallback_reason.is_some());
        assert!(!ranked[0].is_direct_match());
    }
}
