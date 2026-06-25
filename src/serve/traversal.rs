/// Graph traversal and formatting utilities for the MCP server.
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};

use crate::model::{GrapheniumGraph, Node};
use crate::ranking::{self, RankedNode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneratedCodeMode {
    Include,
    Exclude,
    Only,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathMode {
    Strict,
    Semantic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathResult {
    pub path: Vec<String>,
    pub hops: usize,
    pub mode: PathMode,
    pub total_cost_millis: u32,
}

// ── Node scoring ──────────────────────────────────────────────────────────────

/// Score every node by how many keywords from `query` appear in its label,
/// ID, or source_file (case-insensitive).
///
/// Returns `(node_id, score)` pairs sorted highest-score first.
/// If no node matches any keyword, falls back to the top-5 highest-degree nodes.
pub fn score_nodes(graph: &GrapheniumGraph, query: &str) -> Vec<(String, f64)> {
    ranking::score_query_nodes(graph, query)
}

/// Scoped variant of [`score_nodes`] that only considers nodes in `allowed`.
pub fn score_nodes_in_scope(
    graph: &GrapheniumGraph,
    query: &str,
    allowed: Option<&HashSet<String>>,
) -> Vec<(String, f64)> {
    ranking::score_query_nodes_in_scope(graph, query, allowed)
}

/// Scoped detailed ranking with query-match explanations.
pub fn score_nodes_detailed_in_scope(
    graph: &GrapheniumGraph,
    query: &str,
    allowed: Option<&HashSet<String>>,
) -> Vec<RankedNode> {
    ranking::score_query_nodes_detailed_in_scope(graph, query, allowed)
}

/// Return the set of node IDs matching the requested path scope.
///
/// Path matching is case-insensitive and slash-normalized. Both `path_prefix`
/// and `exclude_path` are treated as path fragments so callers can scope using
/// either an absolute prefix or a repo-relative subtree segment.
pub fn scoped_node_ids(
    graph: &GrapheniumGraph,
    path_prefix: Option<&str>,
    exclude_path: Option<&str>,
) -> Option<HashSet<String>> {
    filtered_node_ids(
        graph,
        path_prefix,
        exclude_path,
        None,
        GeneratedCodeMode::Include,
    )
}

/// Return the set of node IDs matching the requested path and node-type scope.
pub fn filtered_node_ids(
    graph: &GrapheniumGraph,
    path_prefix: Option<&str>,
    exclude_path: Option<&str>,
    node_types: Option<&[String]>,
    generated_code_mode: GeneratedCodeMode,
) -> Option<HashSet<String>> {
    let include = normalize_scope(path_prefix);
    let exclude = normalize_scope(exclude_path);
    let node_types = normalize_filters(node_types);

    if include.is_none()
        && exclude.is_none()
        && node_types.is_empty()
        && generated_code_mode == GeneratedCodeMode::Include
    {
        return None;
    }

    Some(
        graph
            .node_ids()
            .filter_map(|id| {
                let node = graph.node_data(id)?;
                (path_matches_scope(&node.source_file, include.as_deref(), exclude.as_deref())
                    && node_matches_type(node, &node_types)
                    && generated_code_matches(&node.source_file, generated_code_mode))
                .then(|| id.to_string())
            })
            .collect(),
    )
}

pub fn parse_generated_code_mode(mode: Option<&str>) -> Result<GeneratedCodeMode, String> {
    match mode.map(|m| m.trim().to_lowercase()) {
        None => Ok(GeneratedCodeMode::Include),
        Some(mode) if mode == "include" => Ok(GeneratedCodeMode::Include),
        Some(mode) if mode == "exclude" => Ok(GeneratedCodeMode::Exclude),
        Some(mode) if mode == "only" => Ok(GeneratedCodeMode::Only),
        Some(other) => Err(format!(
            "Unknown generated_code_mode '{other}'. Expected 'include', 'exclude', or 'only'."
        )),
    }
}

/// Normalize a list of filter strings to lowercase non-empty fragments.
pub fn normalize_filters(values: Option<&[String]>) -> Vec<String> {
    values
        .unwrap_or(&[])
        .iter()
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .collect()
}

/// True when an edge relation passes the include/exclude filter lists.
pub fn relation_matches(relation: &str, include: &[String], exclude: &[String]) -> bool {
    let relation = relation.to_lowercase();

    if !include.is_empty() && !include.iter().any(|needle| relation.contains(needle)) {
        return false;
    }

    !exclude.iter().any(|needle| relation.contains(needle))
}

fn normalize_scope(scope: Option<&str>) -> Option<String> {
    scope.map(normalize_path).filter(|scope| !scope.is_empty())
}

fn path_matches_scope(source_file: &str, include: Option<&str>, exclude: Option<&str>) -> bool {
    let normalized = normalize_path(source_file);

    if let Some(exclude) = exclude {
        if normalized.contains(exclude) {
            return false;
        }
    }

    include.is_none_or(|include| normalized.contains(include))
}

fn normalize_path(path: &str) -> String {
    path.trim().replace('\\', "/").to_lowercase()
}

fn generated_code_matches(source_file: &str, mode: GeneratedCodeMode) -> bool {
    let generated_like = is_generated_like_path(source_file);
    match mode {
        GeneratedCodeMode::Include => true,
        GeneratedCodeMode::Exclude => !generated_like,
        GeneratedCodeMode::Only => generated_like,
    }
}

pub fn is_generated_like_path(path: &str) -> bool {
    let normalized = normalize_path(path);
    let components: Vec<&str> = normalized
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();

    if components.iter().any(|segment| {
        matches!(
            *segment,
            "generated"
                | "gen"
                | "template"
                | "templates"
                | "vendor"
                | "third_party"
                | "third-party"
                | "obj"
                | "dist"
                | "build"
                | "target"
                | "node_modules"
                | "graphenium-out"
        )
    }) {
        return true;
    }

    let file_name = components.last().copied().unwrap_or_default();
    file_name.ends_with(".g.cs")
        || file_name.ends_with(".generated.cs")
        || file_name.ends_with(".designer.cs")
        || file_name.ends_with(".gen.cs")
        || file_name.ends_with(".generated.rs")
        || file_name.ends_with(".generated.ts")
        || file_name.ends_with(".g.ts")
        || file_name.contains(".generated.")
}

fn node_matches_type(node: &Node, allowed_types: &[String]) -> bool {
    allowed_types.is_empty()
        || allowed_types
            .iter()
            .any(|kind| node.file_type.to_string() == *kind)
}

// ── BFS / DFS ─────────────────────────────────────────────────────────────────

/// Breadth-first traversal from `seeds`.
///
/// Stops when `max_nodes` have been visited or no nodes within `max_depth`
/// hops of any seed remain.
pub fn bfs(
    graph: &GrapheniumGraph,
    seeds: &[String],
    max_nodes: usize,
    max_depth: usize,
) -> Vec<String> {
    bfs_in_scope(graph, seeds, max_nodes, max_depth, None)
}

/// Breadth-first traversal constrained to nodes in `allowed` when provided.
pub fn bfs_in_scope(
    graph: &GrapheniumGraph,
    seeds: &[String],
    max_nodes: usize,
    max_depth: usize,
    allowed: Option<&HashSet<String>>,
) -> Vec<String> {
    bfs_with_filters(graph, seeds, max_nodes, max_depth, allowed, &[], &[])
}

/// Breadth-first traversal constrained to both node scope and relation filters.
pub fn bfs_with_filters(
    graph: &GrapheniumGraph,
    seeds: &[String],
    max_nodes: usize,
    max_depth: usize,
    allowed: Option<&HashSet<String>>,
    include_relations: &[String],
    exclude_relations: &[String],
) -> Vec<String> {
    let mut visited: HashSet<String> = HashSet::new();
    let mut order: Vec<String> = Vec::new();
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();

    for seed in seeds {
        if graph.contains_node(seed)
            && allowed.is_none_or(|allowed| allowed.contains(seed))
            && visited.insert(seed.clone())
        {
            order.push(seed.clone());
            queue.push_back((seed.clone(), 0));
        }
    }

    while let Some((id, depth)) = queue.pop_front() {
        if order.len() >= max_nodes {
            break;
        }
        if depth >= max_depth {
            continue;
        }
        for neighbor in
            filtered_neighbors(graph, &id, allowed, include_relations, exclude_relations)
        {
            if visited.insert(neighbor.clone()) {
                order.push(neighbor.clone());
                queue.push_back((neighbor, depth + 1));
            }
        }
    }

    order
}

/// Depth-first traversal from `seeds`.
///
/// Stops when `max_nodes` have been visited or all reachable nodes within
/// `max_depth` hops have been explored.
pub fn dfs(
    graph: &GrapheniumGraph,
    seeds: &[String],
    max_nodes: usize,
    max_depth: usize,
) -> Vec<String> {
    dfs_in_scope(graph, seeds, max_nodes, max_depth, None)
}

/// Depth-first traversal constrained to nodes in `allowed` when provided.
pub fn dfs_in_scope(
    graph: &GrapheniumGraph,
    seeds: &[String],
    max_nodes: usize,
    max_depth: usize,
    allowed: Option<&HashSet<String>>,
) -> Vec<String> {
    dfs_with_filters(graph, seeds, max_nodes, max_depth, allowed, &[], &[])
}

/// Depth-first traversal constrained to both node scope and relation filters.
pub fn dfs_with_filters(
    graph: &GrapheniumGraph,
    seeds: &[String],
    max_nodes: usize,
    max_depth: usize,
    allowed: Option<&HashSet<String>>,
    include_relations: &[String],
    exclude_relations: &[String],
) -> Vec<String> {
    let mut visited: HashSet<String> = HashSet::new();
    let mut order: Vec<String> = Vec::new();

    for seed in seeds {
        dfs_helper(
            graph,
            seed,
            &mut visited,
            &mut order,
            max_nodes,
            max_depth,
            0,
            allowed,
            include_relations,
            exclude_relations,
        );
    }

    order
}

fn dfs_helper(
    graph: &GrapheniumGraph,
    id: &str,
    visited: &mut HashSet<String>,
    order: &mut Vec<String>,
    max_nodes: usize,
    max_depth: usize,
    depth: usize,
    allowed: Option<&HashSet<String>>,
    include_relations: &[String],
    exclude_relations: &[String],
) {
    if visited.contains(id) || order.len() >= max_nodes || depth > max_depth {
        return;
    }
    if !graph.contains_node(id) {
        return;
    }
    if allowed.is_some_and(|allowed| !allowed.contains(id)) {
        return;
    }
    visited.insert(id.to_string());
    order.push(id.to_string());

    for neighbor in filtered_neighbors(graph, id, allowed, include_relations, exclude_relations) {
        dfs_helper(
            graph,
            &neighbor,
            visited,
            order,
            max_nodes,
            max_depth,
            depth + 1,
            allowed,
            include_relations,
            exclude_relations,
        );
    }
}

// ── Text formatting ────────────────────────────────────────────────────────────

/// Format a subgraph (ordered list of node IDs) as Markdown within `budget_chars`.
///
/// Each node is rendered as a Markdown section with its label, type, community,
/// and connections to other nodes in the subgraph.
pub fn subgraph_to_text(
    graph: &GrapheniumGraph,
    node_ids: &[String],
    budget_chars: usize,
) -> String {
    subgraph_to_text_with_filters(graph, node_ids, budget_chars, &[], &[])
}

/// Format a subgraph while filtering rendered edges by relation.
pub fn subgraph_to_text_with_filters(
    graph: &GrapheniumGraph,
    node_ids: &[String],
    budget_chars: usize,
    include_relations: &[String],
    exclude_relations: &[String],
) -> String {
    subgraph_to_text_with_match_details(
        graph,
        node_ids,
        budget_chars,
        include_relations,
        exclude_relations,
        &[],
    )
}

/// Format a subgraph while including concise query-match explanations.
pub fn subgraph_to_text_with_match_details(
    graph: &GrapheniumGraph,
    node_ids: &[String],
    budget_chars: usize,
    include_relations: &[String],
    exclude_relations: &[String],
    ranked_nodes: &[RankedNode],
) -> String {
    if node_ids.is_empty() {
        return "No matching nodes found.".to_string();
    }

    let visited_set: HashSet<&str> = node_ids.iter().map(|s| s.as_str()).collect();
    let ranked_by_id: HashMap<&str, &RankedNode> = ranked_nodes
        .iter()
        .map(|ranked| (ranked.id.as_str(), ranked))
        .collect();
    let mut out = format!("Found {} node(s)\n\n", node_ids.len());

    for id in node_ids {
        if out.len() >= budget_chars {
            out.push_str("\n[... output truncated at budget]\n");
            break;
        }

        let Some(node) = graph.node_data(id) else {
            continue;
        };

        let comm_tag = node
            .community
            .map(|c| format!(" [community {c}]"))
            .unwrap_or_default();

        let mut entry = format!(
            "## {} ({}{})\nFile: {}\n",
            node.label, node.file_type, comm_tag, node.source_file
        );
        if !node.source_location.is_empty() {
            entry.push_str(&format!("Span: {}\n", node.source_location));
        }

        if let Some(ranked) = ranked_by_id.get(id.as_str()) {
            entry.push_str(&format!("Match: {}\n", format_rank_explanation(ranked)));
        } else {
            entry.push_str("Match: included via traversal from matched seed nodes\n");
        }

        let mut connections = String::new();
        let mut seen_connections: HashSet<(String, String)> = HashSet::new();
        for (neighbor_id, edge) in graph.node_edges(id) {
            if visited_set.contains(neighbor_id)
                && relation_matches(&edge.relation, include_relations, exclude_relations)
            {
                let key = (neighbor_id.to_string(), edge.relation.to_lowercase());
                if !seen_connections.insert(key) {
                    continue;
                }
                if let Some(nb) = graph.node_data(neighbor_id) {
                    let provenance = match (&edge.extractor, &edge.resolution_status) {
                        (Some(ext), Some(stat)) => format!(" [{ext}:{stat}]"),
                        (Some(ext), None) => format!(" [{ext}]"),
                        _ => String::new(),
                    };
                    connections.push_str(&format!(
                        "- {} `{}` {}{}\n",
                        node.label, edge.relation, nb.label, provenance
                    ));
                }
            }
        }
        if !connections.is_empty() {
            entry.push_str("\nConnections:\n");
            entry.push_str(&connections);
        }
        entry.push('\n');
        out.push_str(&entry);
    }

    out
}

fn format_rank_explanation(ranked: &RankedNode) -> String {
    if ranked.is_direct_match() {
        let fields = if ranked.matched_fields.is_empty() {
            "unknown fields".to_string()
        } else {
            ranked.matched_fields.join(", ")
        };
        let keywords = ranked
            .matched_keywords
            .iter()
            .map(|keyword| format!("`{keyword}`"))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "direct keyword match on {fields} for {keywords} (score {:.2})",
            ranked.score
        )
    } else if let Some(reason) = &ranked.fallback_reason {
        format!("{reason} (score {:.2})", ranked.score)
    } else {
        format!("ranked seed (score {:.2})", ranked.score)
    }
}

/// Find the shortest path while respecting node scope and relation filters.
pub fn shortest_path_with_filters(
    graph: &GrapheniumGraph,
    from_id: &str,
    to_id: &str,
    allowed: Option<&HashSet<String>>,
    include_relations: &[String],
    exclude_relations: &[String],
) -> Option<Vec<String>> {
    if allowed.is_some_and(|allowed| !allowed.contains(from_id) || !allowed.contains(to_id)) {
        return None;
    }

    let mut visited: HashSet<String> = HashSet::from([from_id.to_string()]);
    let mut queue: VecDeque<String> = VecDeque::from([from_id.to_string()]);
    let mut parents: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    while let Some(current) = queue.pop_front() {
        if current == to_id {
            let mut path = vec![to_id.to_string()];
            let mut cursor = to_id;
            while let Some(parent) = parents.get(cursor) {
                path.push(parent.clone());
                cursor = parent;
            }
            path.reverse();
            return Some(path);
        }

        for neighbor in filtered_neighbors(
            graph,
            &current,
            allowed,
            include_relations,
            exclude_relations,
        ) {
            if visited.insert(neighbor.clone()) {
                parents.insert(neighbor.clone(), current.clone());
                queue.push_back(neighbor);
            }
        }
    }

    None
}

fn filtered_neighbors(
    graph: &GrapheniumGraph,
    id: &str,
    allowed: Option<&HashSet<String>>,
    include_relations: &[String],
    exclude_relations: &[String],
) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut neighbors = Vec::new();

    for (neighbor_id, edge) in graph.node_edges(id) {
        if allowed.is_some_and(|allowed| !allowed.contains(neighbor_id)) {
            continue;
        }
        if !relation_matches(&edge.relation, include_relations, exclude_relations) {
            continue;
        }
        if seen.insert(neighbor_id.to_string()) {
            neighbors.push(neighbor_id.to_string());
        }
    }

    neighbors
}

pub fn semantic_path_with_filters(
    graph: &GrapheniumGraph,
    from_id: &str,
    to_id: &str,
    allowed: Option<&HashSet<String>>,
    include_relations: &[String],
    exclude_relations: &[String],
    mode: PathMode,
    exclude_framework_noise: bool,
) -> Option<PathResult> {
    if allowed.is_some_and(|allowed| !allowed.contains(from_id) || !allowed.contains(to_id)) {
        return None;
    }

    match mode {
        PathMode::Strict => shortest_path_with_filters(
            graph,
            from_id,
            to_id,
            allowed,
            include_relations,
            exclude_relations,
        )
        .map(|path| PathResult {
            hops: path.len().saturating_sub(1),
            total_cost_millis: path.len().saturating_sub(1) as u32 * 1000,
            path,
            mode: PathMode::Strict,
        }),
        PathMode::Semantic => semantic_weighted_path(
            graph,
            from_id,
            to_id,
            allowed,
            include_relations,
            exclude_relations,
            exclude_framework_noise,
        ),
    }
}

fn semantic_weighted_path(
    graph: &GrapheniumGraph,
    from_id: &str,
    to_id: &str,
    allowed: Option<&HashSet<String>>,
    include_relations: &[String],
    exclude_relations: &[String],
    exclude_framework_noise: bool,
) -> Option<PathResult> {
    let mut distances: HashMap<String, u32> = HashMap::from([(from_id.to_string(), 0)]);
    let mut hops: HashMap<String, usize> = HashMap::from([(from_id.to_string(), 0)]);
    let mut parents: HashMap<String, String> = HashMap::new();
    let mut heap: BinaryHeap<(Reverse<u32>, Reverse<usize>, String)> =
        BinaryHeap::from([(Reverse(0), Reverse(0), from_id.to_string())]);

    while let Some((Reverse(cost), Reverse(hop_count), current)) = heap.pop() {
        if current == to_id {
            return Some(PathResult {
                path: reconstruct_path(&parents, to_id),
                hops: hop_count,
                mode: PathMode::Semantic,
                total_cost_millis: cost,
            });
        }

        if distances.get(&current).is_some_and(|best| cost > *best) {
            continue;
        }

        for (neighbor, edge) in filtered_neighbor_edges_for_path(
            graph,
            &current,
            allowed,
            include_relations,
            exclude_relations,
            exclude_framework_noise,
            to_id,
        ) {
            let next_cost = cost.saturating_add(semantic_edge_cost(
                graph,
                &neighbor,
                edge,
                to_id,
                exclude_framework_noise,
            ));
            let next_hops = hop_count + 1;
            let best_cost = distances.get(&neighbor).copied();
            let best_hops = hops.get(&neighbor).copied();
            let should_update = match (best_cost, best_hops) {
                (Some(best_cost), Some(best_hops)) => {
                    next_cost < best_cost || (next_cost == best_cost && next_hops < best_hops)
                }
                _ => true,
            };

            if should_update {
                distances.insert(neighbor.clone(), next_cost);
                hops.insert(neighbor.clone(), next_hops);
                parents.insert(neighbor.clone(), current.clone());
                heap.push((Reverse(next_cost), Reverse(next_hops), neighbor));
            }
        }
    }

    None
}

fn reconstruct_path(parents: &HashMap<String, String>, to_id: &str) -> Vec<String> {
    let mut path = vec![to_id.to_string()];
    let mut cursor = to_id;
    while let Some(parent) = parents.get(cursor) {
        path.push(parent.clone());
        cursor = parent;
    }
    path.reverse();
    path
}

fn filtered_neighbor_edges_for_path<'a>(
    graph: &'a GrapheniumGraph,
    id: &str,
    allowed: Option<&HashSet<String>>,
    include_relations: &[String],
    exclude_relations: &[String],
    exclude_framework_noise: bool,
    destination_id: &str,
) -> Vec<(String, &'a crate::model::Edge)> {
    let mut seen = HashSet::new();
    let mut neighbors = Vec::new();

    for (neighbor_id, edge) in graph.node_edges(id) {
        if allowed.is_some_and(|allowed| !allowed.contains(neighbor_id)) {
            continue;
        }
        if !relation_matches(&edge.relation, include_relations, exclude_relations) {
            continue;
        }
        if exclude_framework_noise
            && neighbor_id != destination_id
            && graph
                .node_data(neighbor_id)
                .is_some_and(|node| ranking::is_framework_noise_node(graph, node))
        {
            continue;
        }
        if seen.insert((neighbor_id.to_string(), edge.relation.clone())) {
            neighbors.push((neighbor_id.to_string(), edge));
        }
    }

    neighbors
}

/// Rank a relation name for display ordering. Lower is "more interesting"
/// (behavioural → structural). Used by `get_neighbors` to sort output so that
/// `calls`/`uses` appear before `contains`/`imports`.
pub fn relation_rank(relation: &str) -> u32 {
    match relation {
        "calls" => 1000,
        "uses" => 1200,
        "method" => 1300,
        "contains" => 1400,
        "references" => 1700,
        "imports" => 6000,
        _ => 2200,
    }
}

fn semantic_edge_cost(
    graph: &GrapheniumGraph,
    neighbor_id: &str,
    edge: &crate::model::Edge,
    to_id: &str,
    exclude_framework_noise: bool,
) -> u32 {
    let relation_cost = relation_rank(edge.relation.as_str());

    let mut node_penalty = 0;
    if neighbor_id != to_id
        && graph
            .node_data(neighbor_id)
            .is_some_and(|node| ranking::is_framework_noise_node(graph, node))
    {
        node_penalty = if exclude_framework_noise {
            20_000
        } else {
            8_000
        };
    }

    relation_cost + node_penalty
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Edge, FileType, Node};

    fn make_graph() -> GrapheniumGraph {
        let mut g = GrapheniumGraph::new();
        let mut a = Node::new("a_foo", "Foo", FileType::Code, "a.py");
        a.community = Some(0);
        let mut b = Node::new("b_bar", "Bar", FileType::Code, "b.py");
        b.community = Some(0);
        let c = Node::new("c_baz", "Baz", FileType::Document, "c.md");
        g.upsert_node(a);
        g.upsert_node(b);
        g.upsert_node(c);
        g.add_edge(Edge::extracted("a_foo", "b_bar", "calls", "a.py"));
        g
    }

    #[test]
    fn score_nodes_keyword_match() {
        let g = make_graph();
        let scored = score_nodes(&g, "Foo");
        assert!(!scored.is_empty());
        assert_eq!(scored[0].0, "a_foo");
    }

    #[test]
    fn score_nodes_fallback_to_degree() {
        let g = make_graph();
        // "xyz" matches nothing — should fall back to high-degree nodes
        let scored = score_nodes(&g, "xyz");
        assert!(!scored.is_empty());
    }

    #[test]
    fn score_nodes_downranks_framework_noise() {
        let mut g = GrapheniumGraph::new();
        g.upsert_node(Node::new(
            "system",
            "System",
            FileType::Code,
            "tests/app.cs",
        ));
        g.upsert_node(Node::new(
            "service",
            "OrderService",
            FileType::Code,
            "src/OrderService.cs",
        ));
        g.upsert_node(Node::new("a", "A", FileType::Code, "src/A.cs"));
        g.upsert_node(Node::new("b", "B", FileType::Code, "src/B.cs"));
        g.add_edge(Edge::extracted("a", "system", "imports", "src/A.cs"));
        g.add_edge(Edge::extracted("b", "system", "imports", "src/B.cs"));
        g.add_edge(Edge::extracted("a", "service", "calls", "src/A.cs"));
        g.add_edge(Edge::extracted("b", "service", "calls", "src/B.cs"));

        let scored = score_nodes(&g, "system service");
        assert_eq!(scored[0].0, "service");
    }

    #[test]
    fn scoped_node_ids_filters_by_include_and_exclude_paths() {
        let g = make_graph();
        let scoped = scoped_node_ids(&g, Some("a.py"), Some("beta")).unwrap();
        assert!(scoped.contains("a_foo"));
        assert!(!scoped.contains("b_bar"));
        assert!(!scoped.contains("c_baz"));
    }

    #[test]
    fn filtered_node_ids_can_filter_by_node_type() {
        let g = make_graph();
        let allowed = vec!["document".to_string()];
        let filtered =
            filtered_node_ids(&g, None, None, Some(&allowed), GeneratedCodeMode::Include).unwrap();
        assert_eq!(filtered, HashSet::from(["c_baz".to_string()]));
    }

    #[test]
    fn filtered_node_ids_can_exclude_generated_like_paths() {
        let mut g = make_graph();
        g.upsert_node(Node::new(
            "template_node",
            "TemplateNode",
            FileType::Code,
            "Data/Templates/MainScreen.view.cs",
        ));

        let filtered = filtered_node_ids(&g, None, None, None, GeneratedCodeMode::Exclude).unwrap();
        assert!(!filtered.contains("template_node"));
        assert!(filtered.contains("a_foo"));

        let generated_only =
            filtered_node_ids(&g, None, None, None, GeneratedCodeMode::Only).unwrap();
        assert_eq!(generated_only, HashSet::from(["template_node".to_string()]));
    }

    #[test]
    fn generated_like_path_detects_template_and_vendor_patterns() {
        assert!(is_generated_like_path("Data/Templates/MainScreen.view.cs"));
        assert!(is_generated_like_path("vendor/pkg/index.ts"));
        assert!(is_generated_like_path("src/Form1.Designer.cs"));
        assert!(!is_generated_like_path("src/RealService.cs"));
    }

    #[test]
    fn bfs_in_scope_ignores_neighbors_outside_scope() {
        let g = make_graph();
        let allowed = HashSet::from(["a_foo".to_string()]);
        let visited = bfs_in_scope(&g, &["a_foo".to_string()], 10, 3, Some(&allowed));
        assert_eq!(visited, vec!["a_foo".to_string()]);
    }

    #[test]
    fn bfs_with_filters_respects_relation_filters() {
        let mut g = make_graph();
        g.add_edge(Edge::extracted("a_foo", "c_baz", "imports", "a.py"));
        let include = vec!["calls".to_string()];
        let visited = bfs_with_filters(&g, &["a_foo".to_string()], 10, 3, None, &include, &[]);
        assert!(visited.contains(&"b_bar".to_string()));
        assert!(!visited.contains(&"c_baz".to_string()));
    }

    #[test]
    fn bfs_visits_connected_nodes() {
        let g = make_graph();
        let visited = bfs(&g, &["a_foo".to_string()], 10, 3);
        assert!(visited.contains(&"a_foo".to_string()));
        assert!(visited.contains(&"b_bar".to_string()));
        // c_baz is disconnected
        assert!(!visited.contains(&"c_baz".to_string()));
    }

    #[test]
    fn bfs_respects_max_nodes() {
        let g = make_graph();
        let visited = bfs(&g, &["a_foo".to_string()], 1, 3);
        assert_eq!(visited.len(), 1);
    }

    #[test]
    fn dfs_visits_connected_nodes() {
        let g = make_graph();
        let visited = dfs(&g, &["a_foo".to_string()], 10, 3);
        assert!(visited.contains(&"a_foo".to_string()));
        assert!(visited.contains(&"b_bar".to_string()));
    }

    #[test]
    fn subgraph_to_text_includes_labels() {
        let g = make_graph();
        let ids = vec!["a_foo".to_string(), "b_bar".to_string()];
        let text = subgraph_to_text(&g, &ids, 10_000);
        assert!(text.contains("Foo"));
        assert!(text.contains("Bar"));
        assert!(text.contains("calls"));
    }

    #[test]
    fn subgraph_to_text_deduplicates_duplicate_connections() {
        let mut g = make_graph();
        let src_idx = g.id_index["a_foo"];
        let tgt_idx = g.id_index["b_bar"];
        g.inner.add_edge(
            src_idx,
            tgt_idx,
            Edge::extracted("a_foo", "b_bar", "calls", "a.py"),
        );

        let ids = vec!["a_foo".to_string(), "b_bar".to_string()];
        let text = subgraph_to_text(&g, &ids, 10_000);
        assert_eq!(text.matches("- Foo `calls` Bar").count(), 1);
    }

    #[test]
    fn subgraph_to_text_with_match_details_shows_direct_and_traversal_reasons() {
        let g = make_graph();
        let ranked = vec![RankedNode {
            id: "a_foo".to_string(),
            score: 2.0,
            matched_keywords: vec!["foo".to_string()],
            matched_fields: vec!["label".to_string()],
            fallback_reason: None,
        }];
        let ids = vec!["a_foo".to_string(), "b_bar".to_string()];
        let text = subgraph_to_text_with_match_details(&g, &ids, 10_000, &[], &[], &ranked);
        assert!(text.contains("direct keyword match on label"));
        assert!(text.contains("included via traversal from matched seed nodes"));
    }

    #[test]
    fn subgraph_to_text_with_filters_omits_excluded_relations() {
        let mut g = make_graph();
        g.add_edge(Edge::extracted("a_foo", "c_baz", "imports", "a.py"));

        let ids = vec![
            "a_foo".to_string(),
            "b_bar".to_string(),
            "c_baz".to_string(),
        ];
        let include = vec!["calls".to_string()];
        let text = subgraph_to_text_with_filters(&g, &ids, 10_000, &include, &[]);
        assert!(text.contains("- Foo `calls` Bar"));
        assert!(!text.contains("imports"));
    }

    #[test]
    fn shortest_path_with_filters_respects_relation_filters() {
        let mut g = make_graph();
        g.add_edge(Edge::extracted("b_bar", "c_baz", "imports", "b.py"));
        let exclude = vec!["imports".to_string()];
        let path = shortest_path_with_filters(&g, "a_foo", "c_baz", None, &[], &exclude);
        assert!(path.is_none());
    }

    #[test]
    fn semantic_path_prefers_meaningful_relations_over_shorter_import_bridge() {
        let mut g = GrapheniumGraph::new();
        g.upsert_node(Node::new("start", "Start", FileType::Code, "src/Start.cs"));
        g.upsert_node(Node::new("goal", "Goal", FileType::Code, "src/Goal.cs"));
        g.upsert_node(Node::new("mid_a", "MidA", FileType::Code, "src/MidA.cs"));
        g.upsert_node(Node::new("mid_b", "MidB", FileType::Code, "src/MidB.cs"));
        g.upsert_node(Node::new(
            "system",
            "System",
            FileType::Code,
            "src/FrameworkBridge.cs",
        ));

        g.add_edge(Edge::extracted(
            "start",
            "system",
            "imports",
            "src/Start.cs",
        ));
        g.add_edge(Edge::extracted(
            "system",
            "goal",
            "imports",
            "src/FrameworkBridge.cs",
        ));
        g.add_edge(Edge::extracted("start", "mid_a", "calls", "src/Start.cs"));
        g.add_edge(Edge::extracted("mid_a", "mid_b", "uses", "src/MidA.cs"));
        g.add_edge(Edge::extracted("mid_b", "goal", "calls", "src/MidB.cs"));

        let semantic = semantic_path_with_filters(
            &g,
            "start",
            "goal",
            None,
            &[],
            &[],
            PathMode::Semantic,
            false,
        )
        .expect("semantic path");
        let strict = semantic_path_with_filters(
            &g,
            "start",
            "goal",
            None,
            &[],
            &[],
            PathMode::Strict,
            false,
        )
        .expect("strict path");

        assert_eq!(semantic.path, vec!["start", "mid_a", "mid_b", "goal"]);
        assert_eq!(strict.path, vec!["start", "system", "goal"]);
        assert!(semantic.total_cost_millis > strict.total_cost_millis);
    }

    #[test]
    fn semantic_path_can_exclude_framework_noise_nodes() {
        let mut g = GrapheniumGraph::new();
        g.upsert_node(Node::new("start", "Start", FileType::Code, "src/Start.cs"));
        g.upsert_node(Node::new("goal", "Goal", FileType::Code, "src/Goal.cs"));
        g.upsert_node(Node::new(
            "system",
            "System",
            FileType::Code,
            "src/FrameworkBridge.cs",
        ));
        g.add_edge(Edge::extracted(
            "start",
            "system",
            "imports",
            "src/Start.cs",
        ));
        g.add_edge(Edge::extracted(
            "system",
            "goal",
            "imports",
            "src/FrameworkBridge.cs",
        ));

        let path = semantic_path_with_filters(
            &g,
            "start",
            "goal",
            None,
            &[],
            &[],
            PathMode::Semantic,
            true,
        );

        assert!(path.is_none());
    }

    #[test]
    fn subgraph_to_text_truncates_at_budget() {
        let g = make_graph();
        let ids = vec!["a_foo".to_string(), "b_bar".to_string()];
        // Tiny budget forces truncation
        let text = subgraph_to_text(&g, &ids, 20);
        assert!(text.contains("truncated") || text.len() <= 100);
    }

    #[test]
    fn empty_ids_returns_placeholder() {
        let g = make_graph();
        let text = subgraph_to_text(&g, &[], 10_000);
        assert!(text.contains("No matching"));
    }
}
