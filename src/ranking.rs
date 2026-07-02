use std::cmp::Ordering;
use std::collections::HashSet;
use std::path::Path;

use crate::analyze::rank::DirectedProjection;
use crate::model::{FileType, GrapheniumGraph, Node};

/// A namespace/import aggregation hub: reached only via `imports` edges and
/// fanned out wide enough that it is clearly a grouping node, not a real symbol.
/// Used by multiple fix phases to exclude namespace hubs from impact/structural tools.
pub fn is_namespace_aggregation_node(node: &Node, graph: &GrapheniumGraph) -> bool {
    if node.file_type != FileType::Code {
        return false;
    }
    let incident_edges: Vec<_> = graph
        .edges_iter()
        .filter(|e| e.source == node.id || e.target == node.id)
        .collect();
    if incident_edges.len() <= 1 {
        return false;
    }
    incident_edges.iter().all(|e| e.relation == "imports")
}

/// Query ranking mode for hybrid retrieval.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum QueryMode {
    /// Standard keyword-based scoring (current default, Phase 4.1).
    Lexical,
    /// Graph distance weighted scoring: rank by topological proximity.
    Structural,
    /// Combined lexical + structural scoring.
    Hybrid,
}

impl QueryMode {
    pub fn from_str(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "structural" => QueryMode::Structural,
            "hybrid" => QueryMode::Hybrid,
            _ => QueryMode::Lexical,
        }
    }
}

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
    // .NET / C#
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
    // Python Standard Library Modules
    "os",
    "sys",
    "re",
    "codecs",
    "warnings",
    "time",
    "math",
    "datetime",
    "subprocess",
    "shutil",
    "logging",
    "argparse",
    "copy",
    "weakref",
    "types",
    "multiprocessing",
    "socket",
    "select",
    "asyncio",
    "urllib",
    "html",
    "csv",
    "ast",
    "parser",
    "pydoc",
    "unittest",
    "mock",
    "ctypes",
    "struct",
    "pickle",
    "sqlite3",
    "tempfile",
    "contextlib",
    "inspect",
    "importlib",
    "pkgutil",
    "zipfile",
    "tarfile",
    "fixer_util",
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

// ── Mode-aware query scoring (Phase 4.1-4.3) ──────────────────────────────────

/// Score nodes using the specified query mode.
pub fn score_query_nodes_with_mode(
    graph: &GrapheniumGraph,
    query: &str,
    mode: QueryMode,
    allowed: Option<&HashSet<String>>,
) -> Vec<RankedNode> {
    match mode {
        QueryMode::Lexical => score_query_nodes_detailed_in_scope(graph, query, allowed),
        QueryMode::Structural => score_structural(graph, query, allowed),
        QueryMode::Hybrid => score_hybrid(graph, query, allowed),
    }
}

fn score_structural(
    graph: &GrapheniumGraph,
    query: &str,
    allowed: Option<&HashSet<String>>,
) -> Vec<RankedNode> {
    let seeded = score_query_nodes_detailed_in_scope(graph, query, allowed);
    let seed_ids: HashSet<String> = seeded.iter().map(|n| n.id.clone()).collect();

    if seeded.is_empty() {
        return seeded;
    }

    let proj = DirectedProjection::from_graph(graph, None);
    let mut distance_scores: std::collections::HashMap<String, f64> =
        std::collections::HashMap::new();

    for seed in &seeded {
        let mut visited = HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back((seed.id.clone(), 0.0));
        visited.insert(seed.id.clone());

        while let Some((current, dist)) = queue.pop_front() {
            let weight = 1.0 / (1.0 + dist * 0.5);
            *distance_scores.entry(current.clone()).or_insert(0.0) += weight;
            if dist >= 3.0 {
                continue;
            }
            if let Some(edges) = proj.outgoing.get(&current) {
                for (tgt, _) in edges {
                    if visited.insert(tgt.clone()) {
                        queue.push_back((tgt.clone(), dist + 1.0));
                    }
                }
            }
        }
    }

    let mut scored: Vec<RankedNode> = Vec::new();
    for (id, score) in distance_scores {
        if let Some(node) = graph.node_data(&id) {
            let matched = if seed_ids.contains(&id) {
                seeded
                    .iter()
                    .find(|n| n.id == id)
                    .map(|n| n.matched_keywords.clone())
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            scored.push(RankedNode {
                id: id.clone(),
                score: score * query_rank_multiplier(graph, node),
                matched_keywords: matched,
                matched_fields: if seed_ids.contains(&id) {
                    vec!["structural".to_string()]
                } else {
                    Vec::new()
                },
                fallback_reason: if seed_ids.contains(&id) {
                    None
                } else {
                    Some(format!("structural distance {score:.2}"))
                },
            });
        }
    }

    scored.sort_unstable_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored
}

fn score_hybrid(
    graph: &GrapheniumGraph,
    query: &str,
    allowed: Option<&HashSet<String>>,
) -> Vec<RankedNode> {
    let lexical = score_query_nodes_detailed_in_scope(graph, query, allowed);
    let structural = score_structural(graph, query, allowed);
    let mut combined: std::collections::HashMap<String, (f64, RankedNode)> =
        std::collections::HashMap::new();

    for node in lexical {
        combined.insert(node.id.clone(), (node.score * 0.6, node));
    }
    for node in structural {
        let entry = combined
            .entry(node.id.clone())
            .or_insert((0.0, node.clone()));
        entry.0 += node.score * 0.4;
    }

    let mut result: Vec<RankedNode> = combined
        .into_values()
        .map(|(score, mut node)| {
            node.score = score;
            node
        })
        .collect();

    result.sort_unstable_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    result
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
    fn test_is_namespace_aggregation_node_detects_import_only_hubs() {
        let mut g = GrapheniumGraph::new();
        let hub = Node::new("app_hub", "App.Hub", FileType::Code, "src/app/hub.rs");
        g.upsert_node(hub.clone());

        // Add callers that import this hub
        for i in 0..10 {
            let caller = Node::new(
                &format!("caller_{}", i),
                &format!("Caller{}", i),
                FileType::Code,
                &format!("src/mod{}/file.rs", i),
            );
            g.upsert_node(caller.clone());
            g.add_edge(Edge::new(
                &format!("caller_{}", i),
                "app_hub",
                "imports",
                crate::model::Confidence::Extracted,
                "src/app/hub.rs",
            ));
        }

        // Hub should be detected as namespace aggregation node
        let hub_node = g.node_data("app_hub").unwrap();
        assert!(is_namespace_aggregation_node(hub_node, &g));

        // A normal function with calls should NOT be
        let normal = Node::new("normal_fn", "normalFn", FileType::Code, "src/doer.rs");
        g.upsert_node(normal.clone());
        g.add_edge(Edge::new(
            "caller_0",
            "normal_fn",
            "calls",
            crate::model::Confidence::Extracted,
            "src/doer.rs",
        ));
        let normal_node = g.node_data("normal_fn").unwrap();
        assert!(!is_namespace_aggregation_node(normal_node, &g));
    }
}
