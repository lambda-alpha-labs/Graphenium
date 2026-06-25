/// MCP server tools exposing the knowledge graph.
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use petgraph::visit::EdgeRef;

use arc_swap::ArcSwap;
use rmcp::{
    model::{ServerCapabilities, ServerInfo},
    tool, ServerHandler,
};

use crate::model::GrapheniumGraph;

use super::traversal;

// ── Server struct ─────────────────────────────────────────────────────────────

/// MCP server backed by a loaded `GrapheniumGraph`.
///
/// The graph lives behind an `ArcSwap` so the `reload_graph` tool can
/// atomically swap it without restarting the server process. Readers
/// (tool handlers) call `self.graph()` to take a snapshot `Arc` for the
/// duration of a single request — lock-free and cheap.
#[derive(Clone)]
pub struct GrapheniumServer {
    graph_store: Arc<ArcSwap<GrapheniumGraph>>,
    /// Path the server was launched with, so `reload_graph` has a default
    /// source when no explicit path is supplied. Wrapped in `Mutex` so
    /// `reload_graph` can also update it when a new path is provided.
    default_path: Arc<Mutex<Option<PathBuf>>>,
}

#[derive(Debug, Clone)]
struct CommunityOverview {
    community_id: usize,
    size: usize,
    focus: Option<String>,
    top_node_label: String,
    top_node_degree: usize,
    internal_edges: usize,
    boundary_edges: usize,
    external_communities: usize,
}

// ── Constructor + helpers (plain impl, no tool_box) ──────────────────────────

impl GrapheniumServer {
    pub fn new(graph: GrapheniumGraph) -> Self {
        Self {
            graph_store: Arc::new(ArcSwap::from_pointee(graph)),
            default_path: Arc::new(Mutex::new(None)),
        }
    }

    /// Construct a server that remembers the path it was launched from so
    /// `reload_graph` can default to reloading from the same source.
    pub fn with_path(graph: GrapheniumGraph, path: PathBuf) -> Self {
        Self {
            graph_store: Arc::new(ArcSwap::from_pointee(graph)),
            default_path: Arc::new(Mutex::new(Some(path))),
        }
    }

    /// Take a lock-free snapshot of the current graph. Cheap — one atomic
    /// fetch-add on the inner `Arc`'s refcount.
    fn graph(&self) -> Arc<GrapheniumGraph> {
        self.graph_store.load_full()
    }

    /// Resolve a node by exact ID, or by case-insensitive label match.
    fn resolve_id(&self, id_or_label: &str) -> Option<String> {
        let graph = self.graph();
        if graph.contains_node(id_or_label) {
            return Some(id_or_label.to_string());
        }
        let lower = id_or_label.to_lowercase();
        let resolved = graph
            .nodes()
            .find(|n| n.label.to_lowercase() == lower)
            .map(|n| n.id.clone());
        resolved
    }

    fn filtered_node_ids(
        &self,
        path_prefix: Option<&str>,
        exclude_path: Option<&str>,
        node_types: Option<&[String]>,
        generated_code_mode: traversal::GeneratedCodeMode,
    ) -> Option<HashSet<String>> {
        traversal::filtered_node_ids(
            &self.graph(),
            path_prefix,
            exclude_path,
            node_types,
            generated_code_mode,
        )
    }

    fn empty_scope_message(scoped: bool) -> String {
        if scoped {
            "No nodes found in the selected filter scope.".to_string()
        } else {
            "No nodes found in the graph.".to_string()
        }
    }

    fn generated_mode_header(mode: traversal::GeneratedCodeMode) -> Option<&'static str> {
        match mode {
            traversal::GeneratedCodeMode::Include => None,
            traversal::GeneratedCodeMode::Exclude => {
                Some("Filter: generated/template/vendor paths excluded\n\n")
            }
            traversal::GeneratedCodeMode::Only => {
                Some("Filter: only generated/template/vendor paths included\n\n")
            }
        }
    }

    fn ast_only_tuning_enabled(&self, explicit: Option<bool>) -> bool {
        explicit.unwrap_or_else(|| self.graph().is_ast_only())
    }

    fn resolve_generated_code_mode(
        &self,
        generated_code_mode: Option<&str>,
        ast_only_tuning: bool,
    ) -> Result<traversal::GeneratedCodeMode, String> {
        if generated_code_mode.is_none() && ast_only_tuning {
            Ok(traversal::GeneratedCodeMode::Exclude)
        } else {
            traversal::parse_generated_code_mode(generated_code_mode)
        }
    }

    fn ast_only_tuning_header(enabled: bool) -> Option<&'static str> {
        if enabled {
            Some("AST-only tuning active: suppressing common import/generated-code noise by default\n\n")
        } else {
            None
        }
    }

    fn summarize_community(&self, community_id: usize, include_members: bool) -> String {
        let graph = self.graph();
        let members: Vec<_> = graph
            .nodes()
            .filter(|n| n.community == Some(community_id))
            .collect();

        if members.is_empty() {
            return format!("Community {community_id} not found or empty.");
        }

        let member_ids: HashSet<&str> = members.iter().map(|n| n.id.as_str()).collect();
        let mut file_types: HashMap<String, usize> = HashMap::new();
        let mut files: HashMap<String, usize> = HashMap::new();
        let mut internal_relations: HashMap<String, usize> = HashMap::new();
        let mut boundary_relations: HashMap<String, usize> = HashMap::new();

        for member in &members {
            *file_types.entry(member.file_type.to_string()).or_default() += 1;
            *files.entry(member.source_file.clone()).or_default() += 1;
        }

        for (src, tgt, edge) in graph.edges_with_endpoints() {
            let src_in = member_ids.contains(src);
            let tgt_in = member_ids.contains(tgt);
            if src_in && tgt_in {
                *internal_relations.entry(edge.relation.clone()).or_default() += 1;
            } else if src_in || tgt_in {
                *boundary_relations.entry(edge.relation.clone()).or_default() += 1;
            }
        }

        let mut representative_nodes: Vec<_> = members
            .iter()
            .map(|node| (*node, graph.degree(&node.id)))
            .collect();
        representative_nodes.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then_with(|| a.0.label.cmp(&b.0.label))
                .then_with(|| a.0.id.cmp(&b.0.id))
        });

        let mut out = format!("# Community {community_id}\n\n");
        if let Some(label) = community_focus_label(&members, &files) {
            out.push_str(&format!("Likely focus: {label}\n\n"));
        }
        out.push_str(&format!("- Members: {}\n", members.len()));
        out.push_str(&format!(
            "- Internal edges: {}\n",
            internal_relations.values().sum::<usize>()
        ));
        out.push_str(&format!(
            "- Boundary edges: {}\n",
            boundary_relations.values().sum::<usize>()
        ));

        let type_summary = top_counts(&file_types, 5)
            .into_iter()
            .map(|(kind, count)| format!("{kind}={count}"))
            .collect::<Vec<_>>()
            .join(", ");
        if !type_summary.is_empty() {
            out.push_str(&format!("- Node types: {type_summary}\n"));
        }

        out.push_str("\n## Representative Nodes\n");
        for (node, degree) in representative_nodes.iter().take(5) {
            out.push_str(&format!(
                "- **{}** ({}, degree={}) — {}\n",
                node.display_label(),
                node.file_type,
                degree,
                node.source_file
            ));
        }

        let top_files = top_counts(&files, 5);
        if !top_files.is_empty() {
            out.push_str("\n## Representative Files\n");
            for (file, count) in top_files {
                out.push_str(&format!("- {file} ({count} node(s))\n"));
            }
        }

        let top_internal = top_counts(&internal_relations, 5);
        if !top_internal.is_empty() {
            out.push_str("\n## Dominant Internal Relations\n");
            for (relation, count) in top_internal {
                out.push_str(&format!("- `{relation}`: {count}\n"));
            }
        }

        let top_boundary = top_counts(&boundary_relations, 5);
        if !top_boundary.is_empty() {
            out.push_str("\n## Dominant Boundary Relations\n");
            for (relation, count) in top_boundary {
                out.push_str(&format!("- `{relation}`: {count}\n"));
            }
        }

        if include_members {
            out.push_str("\n## Members\n");
            for (node, degree) in representative_nodes {
                out.push_str(&format!(
                    "- **{}** ({}, degree={}) — {}\n",
                    node.display_label(),
                    node.file_type,
                    degree,
                    node.source_file
                ));
            }
        }

        out
    }

    fn render_architecture_summary(
        &self,
        scoped: Option<&HashSet<String>>,
        generated_code_mode: traversal::GeneratedCodeMode,
        max_communities: usize,
    ) -> String {
        let graph = self.graph();
        let in_scope = |id: &str| scoped.is_none_or(|allowed| allowed.contains(id));
        let nodes: Vec<_> = graph.nodes().filter(|n| in_scope(&n.id)).collect();
        if nodes.is_empty() {
            return Self::empty_scope_message(true);
        }

        let overviews = community_overviews(&graph, &nodes, scoped);
        let hotspots = crate::analyze::god_nodes_in_scope(&graph, 5, scoped);
        let connectors = top_bridge_nodes(&graph, &nodes, scoped, 5);

        let mut out = String::new();
        if let Some(header) = Self::generated_mode_header(generated_code_mode) {
            out.push_str(header);
        }
        out.push_str("# Architecture Summary\n\n");
        out.push_str(&format!("- Nodes in scope: {}\n", nodes.len()));
        out.push_str(&format!("- Communities in scope: {}\n", overviews.len()));
        out.push_str(&format!("- Hotspots listed: {}\n", hotspots.len()));
        out.push_str(&format!("- Connectors listed: {}\n", connectors.len()));

        if !overviews.is_empty() {
            out.push_str("\n## Largest Communities\n");
            for ov in overviews.iter().take(max_communities) {
                let focus = ov.focus.as_deref().unwrap_or("mixed/unclear focus");
                out.push_str(&format!(
                    "- Community {}: {} node(s), focus `{}`, top node **{}** (degree {}), boundary edges {}, external communities {}\n",
                    ov.community_id,
                    ov.size,
                    focus,
                    ov.top_node_label,
                    ov.top_node_degree,
                    ov.boundary_edges,
                    ov.external_communities,
                ));
            }
        }

        if !connectors.is_empty() {
            out.push_str("\n## Cross-Community Connectors\n");
            for (label, file, cross_edges, external_communities, degree) in connectors {
                out.push_str(&format!(
                    "- **{}** bridges {} cross-community edge(s) across {} community(s) (degree {}) — {}\n",
                    label, cross_edges, external_communities, degree, file
                ));
            }
        }

        if !hotspots.is_empty() {
            out.push_str("\n## Architectural Hotspots\n");
            for gn in hotspots.iter().take(5) {
                out.push_str(&format!(
                    "- **{}** (degree {}) — {}\n",
                    gn.display_label(),
                    gn.degree,
                    gn.source_file
                ));
            }
        }

        let mut boundary_heavy = overviews
            .iter()
            .filter(|ov| ov.boundary_edges > 0)
            .collect::<Vec<_>>();
        boundary_heavy.sort_by(|a, b| {
            b.boundary_edges
                .cmp(&a.boundary_edges)
                .then_with(|| b.external_communities.cmp(&a.external_communities))
                .then_with(|| a.community_id.cmp(&b.community_id))
        });
        if !boundary_heavy.is_empty() {
            out.push_str("\n## Boundary-Heavy Communities\n");
            for ov in boundary_heavy.into_iter().take(3) {
                let focus = ov.focus.as_deref().unwrap_or("mixed/unclear focus");
                out.push_str(&format!(
                    "- Community {} ({}) has {} boundary edge(s) vs {} internal edge(s)\n",
                    ov.community_id, focus, ov.boundary_edges, ov.internal_edges,
                ));
            }
        }

        out
    }
}

fn top_counts(map: &HashMap<String, usize>, limit: usize) -> Vec<(String, usize)> {
    let mut entries: Vec<_> = map.iter().map(|(k, v)| (k.clone(), *v)).collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    entries.truncate(limit);
    entries
}

fn community_focus_label(
    members: &[&crate::model::Node],
    files: &HashMap<String, usize>,
) -> Option<String> {
    let path_focus = common_path_focus(members);
    if path_focus.is_some() {
        return path_focus;
    }

    let top_files = top_counts(files, 2);
    if top_files.is_empty() {
        None
    } else {
        Some(
            top_files
                .into_iter()
                .map(|(file, _)| file)
                .collect::<Vec<_>>()
                .join(" | "),
        )
    }
}

fn common_path_focus(members: &[&crate::model::Node]) -> Option<String> {
    let mut paths = members
        .iter()
        .map(|node| normalize_display_path(&node.source_file))
        .collect::<Vec<_>>();
    if paths.is_empty() {
        return None;
    }

    let mut prefix: Vec<String> = paths
        .remove(0)
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(|segment| segment.to_string())
        .collect();

    for path in paths {
        let segments: Vec<String> = path
            .split('/')
            .filter(|segment| !segment.is_empty())
            .map(|segment| segment.to_string())
            .collect();
        let common_len = prefix
            .iter()
            .zip(segments.iter())
            .take_while(|(a, b)| a.eq_ignore_ascii_case(b))
            .count();
        prefix.truncate(common_len);
        if prefix.is_empty() {
            return None;
        }
    }

    if prefix.last().is_some_and(|segment| segment.contains('.')) {
        prefix.pop();
    }

    if prefix.len() <= 1 {
        None
    } else {
        Some(prefix.join("/"))
    }
}

fn normalize_display_path(path: &str) -> String {
    path.strip_prefix("\\\\?\\")
        .unwrap_or(path)
        .replace('\\', "/")
        .trim_start_matches('/')
        .to_string()
}

fn community_overviews(
    graph: &GrapheniumGraph,
    nodes: &[&crate::model::Node],
    scoped: Option<&HashSet<String>>,
) -> Vec<CommunityOverview> {
    let mut grouped: HashMap<usize, Vec<&crate::model::Node>> = HashMap::new();
    for node in nodes.iter().copied() {
        if let Some(cid) = node.community {
            grouped.entry(cid).or_default().push(node);
        }
    }

    let mut overviews = grouped
        .into_iter()
        .map(|(community_id, members)| {
            let member_ids: HashSet<&str> = members.iter().map(|n| n.id.as_str()).collect();
            let mut files: HashMap<String, usize> = HashMap::new();
            for member in &members {
                *files.entry(member.source_file.clone()).or_default() += 1;
            }

            let mut internal_edges = 0;
            let mut boundary_edges = 0;
            let mut external_communities = HashSet::new();
            for member in &members {
                for (neighbor_id, _edge) in graph.node_edges(&member.id) {
                    if scoped.is_some_and(|allowed| !allowed.contains(neighbor_id)) {
                        continue;
                    }
                    if member_ids.contains(neighbor_id) {
                        internal_edges += 1;
                    } else {
                        boundary_edges += 1;
                        if let Some(neighbor) = graph.node_data(neighbor_id) {
                            if let Some(other_comm) = neighbor.community {
                                external_communities.insert(other_comm);
                            }
                        }
                    }
                }
            }

            internal_edges /= 2;

            let (top_node_label, top_node_degree) = members
                .iter()
                .filter(|node| !crate::ranking::is_framework_noise_node(graph, node))
                .map(|node| (node.display_label().to_string(), graph.degree(&node.id)))
                .max_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)))
                .or_else(|| {
                    members
                        .iter()
                        .map(|node| (node.display_label().to_string(), graph.degree(&node.id)))
                        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)))
                })
                .unwrap_or_else(|| ("unknown".to_string(), 0));

            CommunityOverview {
                community_id,
                size: members.len(),
                focus: community_focus_label(&members, &files),
                top_node_label,
                top_node_degree,
                internal_edges,
                boundary_edges,
                external_communities: external_communities.len(),
            }
        })
        .collect::<Vec<_>>();

    overviews.sort_by(|a, b| {
        b.size
            .cmp(&a.size)
            .then_with(|| b.boundary_edges.cmp(&a.boundary_edges))
            .then_with(|| a.community_id.cmp(&b.community_id))
    });
    overviews
}

fn top_bridge_nodes(
    graph: &GrapheniumGraph,
    nodes: &[&crate::model::Node],
    scoped: Option<&HashSet<String>>,
    limit: usize,
) -> Vec<(String, String, usize, usize, usize)> {
    let mut bridges = nodes
        .iter()
        .filter_map(|node| {
            if crate::ranking::is_framework_noise_node(graph, node) {
                return None;
            }
            let mut cross_edges = 0;
            let mut external_communities = HashSet::new();
            let node_comm = node.community?;
            for (neighbor_id, _edge) in graph.node_edges(&node.id) {
                if scoped.is_some_and(|allowed| !allowed.contains(neighbor_id)) {
                    continue;
                }
                let Some(neighbor) = graph.node_data(neighbor_id) else {
                    continue;
                };
                if let Some(other_comm) = neighbor.community {
                    if other_comm != node_comm {
                        cross_edges += 1;
                        external_communities.insert(other_comm);
                    }
                }
            }

            if cross_edges == 0 {
                return None;
            }

            Some((
                node.display_label().to_string(),
                node.source_file.clone(),
                cross_edges,
                external_communities.len(),
                graph.degree(&node.id),
            ))
        })
        .collect::<Vec<_>>();

    bridges.sort_by(|a, b| {
        b.2.cmp(&a.2)
            .then_with(|| b.3.cmp(&a.3))
            .then_with(|| b.4.cmp(&a.4))
            .then_with(|| a.0.cmp(&b.0))
    });
    bridges.truncate(limit);
    bridges
}

// ── Tool definitions ──────────────────────────────────────────────────────────

#[tool(tool_box)]
impl GrapheniumServer {
    /// Query the knowledge graph with keywords via BFS or DFS traversal.
    #[tool(description = "Query the knowledge graph with keywords. \
        Scores nodes by keyword match and traverses the graph via BFS (default) or DFS. \
        Returns matching nodes and their connections formatted as Markdown.")]
    fn query_graph(
        &self,
        #[tool(param)]
        #[schemars(description = "Keywords or phrase to search for (space-separated)")]
        keywords: String,
        #[tool(param)]
        #[schemars(description = "Traversal depth (1–6, default 3)")]
        depth: Option<i32>,
        #[tool(param)]
        #[schemars(description = "Approximate output token budget (default 2000)")]
        budget: Option<i32>,
        #[tool(param)]
        #[schemars(description = "Use depth-first search instead of BFS (default false)")]
        dfs: Option<bool>,
        #[tool(param)]
        #[schemars(
            description = "Optional path prefix or path fragment to include (case-insensitive)"
        )]
        path_prefix: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Optional path prefix or path fragment to exclude (case-insensitive)"
        )]
        exclude_path: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Optional file types to include, e.g. ['code', 'document', 'rationale']"
        )]
        node_types: Option<Vec<String>>,
        #[tool(param)]
        #[schemars(
            description = "Optional relation filters to include during traversal/rendering, e.g. ['calls', 'uses']"
        )]
        include_relations: Option<Vec<String>>,
        #[tool(param)]
        #[schemars(
            description = "Optional relation filters to exclude during traversal/rendering, e.g. ['imports']"
        )]
        exclude_relations: Option<Vec<String>>,
        #[tool(param)]
        #[schemars(
            description = "Generated-like code filter: 'include' (default), 'exclude', or 'only'"
        )]
        generated_code_mode: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "AST-only tuning: true enables AST-only noise suppression, false disables it, omitted = auto"
        )]
        ast_only_tuning: Option<bool>,
    ) -> String {
        let depth = (depth.unwrap_or(3) as usize).clamp(1, 6);
        let budget = (budget.unwrap_or(2000) as usize).max(200);
        let use_dfs = dfs.unwrap_or(false);
        let max_nodes = (budget / 40).max(5).min(200);
        let ast_only_tuning = self.ast_only_tuning_enabled(ast_only_tuning);
        let generated_code_mode = match self
            .resolve_generated_code_mode(generated_code_mode.as_deref(), ast_only_tuning)
        {
            Ok(mode) => mode,
            Err(err) => return err,
        };
        let scoped = self.filtered_node_ids(
            path_prefix.as_deref(),
            exclude_path.as_deref(),
            node_types.as_deref(),
            generated_code_mode,
        );
        let include_relations = traversal::normalize_filters(include_relations.as_deref());
        let mut exclude_relations = traversal::normalize_filters(exclude_relations.as_deref());
        if ast_only_tuning && exclude_relations.is_empty() {
            exclude_relations.push("imports".to_string());
        }

        if scoped.as_ref().is_some_and(HashSet::is_empty) {
            return Self::empty_scope_message(true);
        }

        let graph = self.graph();
        let ranked = traversal::score_nodes_detailed_in_scope(&graph, &keywords, scoped.as_ref());
        let seeds: Vec<String> = ranked.iter().take(5).map(|node| node.id.clone()).collect();

        if seeds.is_empty() {
            return Self::empty_scope_message(scoped.is_some());
        }

        let visited = if use_dfs {
            traversal::dfs_with_filters(
                &graph,
                &seeds,
                max_nodes,
                depth,
                scoped.as_ref(),
                &include_relations,
                &exclude_relations,
            )
        } else {
            traversal::bfs_with_filters(
                &graph,
                &seeds,
                max_nodes,
                depth,
                scoped.as_ref(),
                &include_relations,
                &exclude_relations,
            )
        };

        let mut out = String::new();
        if let Some(header) = Self::ast_only_tuning_header(ast_only_tuning) {
            out.push_str(header);
        }
        if let Some(header) = Self::generated_mode_header(generated_code_mode) {
            out.push_str(header);
        }
        out.push_str(&traversal::subgraph_to_text_with_match_details(
            &graph,
            &visited,
            budget * 4,
            &include_relations,
            &exclude_relations,
            &ranked,
        ));
        out
    }

    /// Get full details for a node by ID or label.
    #[tool(
        description = "Get full details for a node by ID or label (case-insensitive). \
        Returns the node's label, file type, source file, source span/location, \
        community assignment, and degree."
    )]
    fn get_node(
        &self,
        #[tool(param)]
        #[schemars(description = "Node ID or label to look up")]
        id: String,
    ) -> String {
        let resolved = match self.resolve_id(&id) {
            Some(r) => r,
            None => return format!("Node '{id}' not found."),
        };
        let graph = self.graph();
        let node = graph.node_data(&resolved).unwrap();
        let degree = graph.degree(&resolved);
        let comm = node
            .community
            .map(|c| c.to_string())
            .unwrap_or_else(|| "none".to_string());
        let loc = if node.source_location.is_empty() {
            "unknown"
        } else {
            &node.source_location
        };
        let display = node.display_label();
        let name_suffix = if display != node.label {
            format!(" (short name: {})", node.label)
        } else {
            String::new()
        };

        format!(
            "**{display}**{name_suffix} ({ft})\n\
             ID: {id}\n\
             File: {sf}\n\
             Span: {loc}\n\
             Community: {comm}\n\
             Degree: {degree}",
            ft = node.file_type,
            id = node.id,
            sf = node.source_file,
        )
    }

    /// Get direct neighbors of a node with edge details.
    #[tool(
        description = "Get all direct neighbors of a node, including edge relation types, \
        confidence levels, and scores. An optional relation filter narrows results to \
        edges whose relation name contains the given substring."
    )]
    fn get_neighbors(
        &self,
        #[tool(param)]
        #[schemars(description = "Node ID or label to query")]
        node_id: String,
        #[tool(param)]
        #[schemars(
            description = "Optional substring to filter by relation type (e.g. 'calls', 'imports')"
        )]
        relation: Option<String>,
    ) -> String {
        let resolved = match self.resolve_id(&node_id) {
            Some(r) => r,
            None => return format!("Node '{node_id}' not found."),
        };

        let graph = self.graph();
        let node = graph.node_data(&resolved).unwrap();
        let mut out = format!("# Neighbors of {}\n\n", node.display_label());
        let mut seen = HashSet::new();

        let mut entries: Vec<_> = graph
            .node_edges(&resolved)
            .into_iter()
            .filter(|(_, edge)| match relation.as_deref() {
                Some(filter) => edge
                    .relation
                    .to_lowercase()
                    .contains(&filter.to_lowercase()),
                None => true,
            })
            .filter(|(neighbor_id, edge)| {
                seen.insert((
                    neighbor_id.to_string(),
                    edge.relation.to_lowercase(),
                    edge.confidence.to_string(),
                ))
            })
            .filter_map(|(neighbor_id, edge)| {
                graph
                    .node_data(neighbor_id)
                    .map(|nb| (neighbor_id, edge, nb))
            })
            .collect();

        // Rank behavioural edges (calls, uses, inherits) before structural
        // ones (contains, imports) so agents see the "interesting" neighbours
        // first when their budget is limited.
        entries.sort_by_key(|(_, edge, nb)| {
            (
                super::traversal::relation_rank(edge.relation.as_str()),
                nb.label.to_lowercase(),
            )
        });

        let count = entries.len();
        for (_, edge, nb) in &entries {
            out.push_str(&format!(
                "- **{}** via `{}` ({}, score={:.2}) — {}\n",
                nb.display_label(),
                edge.relation,
                edge.confidence,
                edge.confidence_score,
                nb.source_file
            ));
        }

        if count == 0 {
            out.push_str("No neighbors found");
            if relation.is_some() {
                out.push_str(" with the given relation filter");
            }
            out.push('.');
        } else {
            out.push_str(&format!("\nTotal: {count} neighbor(s)"));
        }

        out
    }

    /// Get a concise summary of a community by ID.
    #[tool(description = "Summarize a community by its integer community ID. \
        Returns representative nodes, files, and dominant relations. \
        Set `include_members` to true to append the full member list.")]
    fn get_community(
        &self,
        #[tool(param)]
        #[schemars(description = "Integer community ID (0-indexed, community 0 is the largest)")]
        community_id: i32,
        #[tool(param)]
        #[schemars(description = "Append the full member list after the summary (default false)")]
        include_members: Option<bool>,
    ) -> String {
        self.summarize_community(community_id as usize, include_members.unwrap_or(false))
    }

    /// Return the top N most-connected hub nodes.
    #[tool(
        description = "Return the top N most connected nodes ('god nodes' or hubs) in the graph. \
        File-level hubs and stub nodes (degree ≤ 1) are filtered out. \
        Useful for finding architectural hotspots."
    )]
    fn god_nodes(
        &self,
        #[tool(param)]
        #[schemars(description = "Number of hub nodes to return (default 10, max 50)")]
        n: Option<i32>,
        #[tool(param)]
        #[schemars(
            description = "Optional path prefix or path fragment to include (case-insensitive)"
        )]
        path_prefix: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Optional path prefix or path fragment to exclude (case-insensitive)"
        )]
        exclude_path: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Optional file types to include, e.g. ['code', 'document', 'rationale']"
        )]
        node_types: Option<Vec<String>>,
        #[tool(param)]
        #[schemars(
            description = "Generated-like code filter: 'include' (default), 'exclude', or 'only'"
        )]
        generated_code_mode: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "AST-only tuning: true enables AST-only noise suppression, false disables it, omitted = auto"
        )]
        ast_only_tuning: Option<bool>,
    ) -> String {
        let top_n = n.map(|v| (v as usize).clamp(1, 50)).unwrap_or(10);
        let ast_only_tuning = self.ast_only_tuning_enabled(ast_only_tuning);
        let generated_code_mode = match self
            .resolve_generated_code_mode(generated_code_mode.as_deref(), ast_only_tuning)
        {
            Ok(mode) => mode,
            Err(err) => return err,
        };
        let scoped = self.filtered_node_ids(
            path_prefix.as_deref(),
            exclude_path.as_deref(),
            node_types.as_deref(),
            generated_code_mode,
        );

        if scoped.as_ref().is_some_and(HashSet::is_empty) {
            return "No hub nodes found in the selected filter scope.".to_string();
        }

        let result = crate::analyze::god_nodes_in_scope(&self.graph(), top_n, scoped.as_ref());

        if result.is_empty() {
            return if scoped.is_some() {
                "No hub nodes found in the selected filter scope.".to_string()
            } else {
                "No hub nodes found (graph may be too small or disconnected).".to_string()
            };
        }

        let mut out = String::new();
        if let Some(header) = Self::ast_only_tuning_header(ast_only_tuning) {
            out.push_str(header);
        }
        if let Some(header) = Self::generated_mode_header(generated_code_mode) {
            out.push_str(header);
        }
        out.push_str(&format!("# Top {} Hub Nodes\n\n", result.len()));
        for gn in &result {
            let comm = gn
                .community
                .map(|c| format!(" [community {c}]"))
                .unwrap_or_default();
            out.push_str(&format!(
                "- **{}**{} — degree {}, {}\n",
                gn.display_label(),
                comm,
                gn.degree,
                gn.source_file
            ));
        }
        out
    }

    /// Return summary statistics for the knowledge graph.
    #[tool(
        description = "Return summary statistics for the loaded knowledge graph: \
        node/edge/hyperedge counts, number of communities, node-type breakdown, \
        and edge confidence distribution."
    )]
    fn graph_stats(
        &self,
        #[tool(param)]
        #[schemars(
            description = "Optional path prefix or path fragment to include (case-insensitive)"
        )]
        path_prefix: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Optional path prefix or path fragment to exclude (case-insensitive)"
        )]
        exclude_path: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Optional file types to include, e.g. ['code', 'document', 'rationale']"
        )]
        node_types: Option<Vec<String>>,
        #[tool(param)]
        #[schemars(
            description = "Generated-like code filter: 'include' (default), 'exclude', or 'only'"
        )]
        generated_code_mode: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "AST-only tuning: true enables AST-only noise suppression, false disables it, omitted = auto"
        )]
        ast_only_tuning: Option<bool>,
    ) -> String {
        let g = self.graph();
        let g = &*g;
        let ast_only_tuning = self.ast_only_tuning_enabled(ast_only_tuning);
        let generated_code_mode = match self
            .resolve_generated_code_mode(generated_code_mode.as_deref(), ast_only_tuning)
        {
            Ok(mode) => mode,
            Err(err) => return err,
        };
        let scoped = self.filtered_node_ids(
            path_prefix.as_deref(),
            exclude_path.as_deref(),
            node_types.as_deref(),
            generated_code_mode,
        );
        let in_scope = |id: &str| scoped.as_ref().is_none_or(|allowed| allowed.contains(id));

        let nodes: Vec<_> = g.nodes().filter(|n| in_scope(&n.id)).collect();
        let communities: std::collections::HashSet<usize> =
            nodes.iter().filter_map(|n| n.community).collect();

        let extracted = g
            .edges_with_endpoints()
            .filter(|(src, tgt, e)| {
                in_scope(src)
                    && in_scope(tgt)
                    && matches!(e.confidence, crate::model::Confidence::Extracted)
            })
            .count();
        let inferred = g
            .edges_with_endpoints()
            .filter(|(src, tgt, e)| {
                in_scope(src)
                    && in_scope(tgt)
                    && matches!(e.confidence, crate::model::Confidence::Inferred)
            })
            .count();
        let ambiguous = g
            .edges_with_endpoints()
            .filter(|(src, tgt, e)| {
                in_scope(src)
                    && in_scope(tgt)
                    && matches!(e.confidence, crate::model::Confidence::Ambiguous)
            })
            .count();

        let edge_count = extracted + inferred + ambiguous;
        let hyperedge_count = g
            .hyperedges
            .iter()
            .filter(|h| h.nodes.iter().all(|id| in_scope(id)))
            .count();

        // Count extractors for provenance breakdown.
        let extractor_counts: std::collections::BTreeMap<&str, usize> = {
            let mut m: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
            for e in g.edges_iter() {
                if let Some(ref ext) = e.extractor {
                    *m.entry(ext.as_str()).or_default() += 1;
                }
            }
            m
        };
        let extractor_rows: String = extractor_counts
            .iter()
            .map(|(k, v)| format!("  {k}: {v}"))
            .collect::<Vec<_>>()
            .join("\n");

        let type_rows: String = ["code", "document", "paper", "image", "rationale"]
            .iter()
            .filter_map(|t| {
                let count = nodes
                    .iter()
                    .filter(|n| n.file_type.to_string() == *t)
                    .count();
                if count > 0 {
                    Some(format!("  {t}: {count}"))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        let mut out = String::new();
        if let Some(header) = Self::ast_only_tuning_header(ast_only_tuning) {
            out.push_str(header);
        }
        if let Some(header) = Self::generated_mode_header(generated_code_mode) {
            out.push_str(header);
        }

        // Graph metadata lines.
        let meta_lines: Vec<String> = {
            let mut lines = Vec::new();
            if let Some(ref v) = g.metadata.schema_version {
                lines.push(format!("  Schema version: {v}"));
            }
            if let Some(ref modes) = g.metadata.extraction_modes {
                lines.push(format!("  Extraction modes: {}", modes.join(", ")));
            }
            if let Some(ref langs) = g.metadata.languages {
                lines.push(format!("  Languages: {}", langs.join(", ")));
            }
            lines
        };
        let meta_block = if meta_lines.is_empty() {
            String::new()
        } else {
            format!("# Graph Metadata\n{}\n\n", meta_lines.join("\n"))
        };

        out.push_str(&format!(
            "# Graph Statistics\n\n\
             - Nodes: {n}\n\
             - Edges: {e}\n\
             - Hyperedges: {h}\n\
             - Communities: {c}\n\n\
             ## Node Types\n{type_rows}\n\n\
             ## Edge Confidence\n\
             - EXTRACTED: {extracted}\n\
             - INFERRED: {inferred}\n\
             - AMBIGUOUS: {ambiguous}\n\
             \n## Edge Provenance\n\
             {extractor_rows}",
            n = nodes.len(),
            e = edge_count,
            h = hyperedge_count,
            c = communities.len(),
        ));
        out = meta_block + &out;
        out
    }

    /// Return higher-level architectural highlights for the graph.
    #[tool(
        description = "Return a repository-level architectural summary with major communities, \
        cross-community connectors, and architectural hotspots. Useful for orienting on a codebase \
        before digging into specific files."
    )]
    fn architecture_summary(
        &self,
        #[tool(param)]
        #[schemars(
            description = "Optional path prefix or path fragment to include (case-insensitive)"
        )]
        path_prefix: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Optional path prefix or path fragment to exclude (case-insensitive)"
        )]
        exclude_path: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Optional file types to include, e.g. ['code', 'document', 'rationale']"
        )]
        node_types: Option<Vec<String>>,
        #[tool(param)]
        #[schemars(
            description = "Generated-like code filter: 'include' (default), 'exclude', or 'only'"
        )]
        generated_code_mode: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "AST-only tuning: true enables AST-only noise suppression, false disables it, omitted = auto"
        )]
        ast_only_tuning: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Maximum number of communities to summarize (default 5, max 10)")]
        max_communities: Option<i32>,
    ) -> String {
        let ast_only_tuning = self.ast_only_tuning_enabled(ast_only_tuning);
        let generated_code_mode = match self
            .resolve_generated_code_mode(generated_code_mode.as_deref(), ast_only_tuning)
        {
            Ok(mode) => mode,
            Err(err) => return err,
        };
        let scoped = self.filtered_node_ids(
            path_prefix.as_deref(),
            exclude_path.as_deref(),
            node_types.as_deref(),
            generated_code_mode,
        );
        if scoped.as_ref().is_some_and(HashSet::is_empty) {
            return Self::empty_scope_message(true);
        }

        let mut out = String::new();
        if let Some(header) = Self::ast_only_tuning_header(ast_only_tuning) {
            out.push_str(header);
        }
        out.push_str(
            &self.render_architecture_summary(
                scoped.as_ref(),
                generated_code_mode,
                max_communities
                    .map(|n| (n as usize).clamp(1, 10))
                    .unwrap_or(5),
            ),
        );
        out
    }

    /// Find the shortest path between two nodes.
    #[tool(
        description = "Find a path between two nodes. By default, semantic mode prefers \
        calls/uses/contains-style relationships over imports. Set mode='strict' for \
        the exact fewest-hop path. Accepts node IDs or labels (case-insensitive)."
    )]
    fn shortest_path(
        &self,
        #[tool(param)]
        #[schemars(description = "Starting node ID or label")]
        from: String,
        #[tool(param)]
        #[schemars(description = "Destination node ID or label")]
        to: String,
        #[tool(param)]
        #[schemars(
            description = "Optional path prefix or path fragment to include (case-insensitive)"
        )]
        path_prefix: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Optional path prefix or path fragment to exclude (case-insensitive)"
        )]
        exclude_path: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Optional file types to include, e.g. ['code', 'document', 'rationale']"
        )]
        node_types: Option<Vec<String>>,
        #[tool(param)]
        #[schemars(
            description = "Optional relation filters to include in the path, e.g. ['calls', 'uses']"
        )]
        include_relations: Option<Vec<String>>,
        #[tool(param)]
        #[schemars(
            description = "Optional relation filters to exclude from the path, e.g. ['imports']"
        )]
        exclude_relations: Option<Vec<String>>,
        #[tool(param)]
        #[schemars(
            description = "Path mode: 'semantic' (default) prefers meaningful relations; 'strict' uses exact hop count"
        )]
        mode: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Exclude framework/import-noise bridge nodes from the path search (default false)"
        )]
        exclude_framework_noise: Option<bool>,
        #[tool(param)]
        #[schemars(
            description = "Generated-like code filter: 'include' (default), 'exclude', or 'only'"
        )]
        generated_code_mode: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "AST-only tuning: true enables AST-only noise suppression, false disables it, omitted = auto"
        )]
        ast_only_tuning: Option<bool>,
    ) -> String {
        let from_id = match self.resolve_id(&from) {
            Some(id) => id,
            None => return format!("Node '{from}' not found."),
        };
        let to_id = match self.resolve_id(&to) {
            Some(id) => id,
            None => return format!("Node '{to}' not found."),
        };

        let graph = self.graph();
        if from_id == to_id {
            let node = graph.node_data(&from_id).unwrap();
            return format!(
                "'{}' and '{}' refer to the same node: {}.",
                from,
                to,
                node.display_label()
            );
        }

        let ast_only_tuning = self.ast_only_tuning_enabled(ast_only_tuning);
        let generated_code_mode = match self
            .resolve_generated_code_mode(generated_code_mode.as_deref(), ast_only_tuning)
        {
            Ok(mode) => mode,
            Err(err) => return err,
        };
        let scoped = self.filtered_node_ids(
            path_prefix.as_deref(),
            exclude_path.as_deref(),
            node_types.as_deref(),
            generated_code_mode,
        );
        let include_relations = traversal::normalize_filters(include_relations.as_deref());
        let exclude_relations = traversal::normalize_filters(exclude_relations.as_deref());
        let exclude_framework_noise = exclude_framework_noise.unwrap_or(ast_only_tuning);
        let mode = match mode.as_deref().map(|m| m.trim().to_lowercase()) {
            None => traversal::PathMode::Semantic,
            Some(m) if m == "semantic" => traversal::PathMode::Semantic,
            Some(m) if m == "strict" => traversal::PathMode::Strict,
            Some(other) => {
                return format!("Unknown path mode '{other}'. Expected 'semantic' or 'strict'.")
            }
        };

        let result = traversal::semantic_path_with_filters(
            &graph,
            &from_id,
            &to_id,
            scoped.as_ref(),
            &include_relations,
            &exclude_relations,
            mode,
            exclude_framework_noise,
        );

        match result {
            Some(result) => {
                let labels: Vec<String> = result
                    .path
                    .iter()
                    .filter_map(|id| graph.node_data(id))
                    .map(|node| node.display_label().to_string())
                    .collect();
                let mode_label = match result.mode {
                    traversal::PathMode::Semantic => "Semantic path",
                    traversal::PathMode::Strict => "Strict shortest path",
                };
                let mut out = String::new();
                if let Some(header) = Self::ast_only_tuning_header(ast_only_tuning) {
                    out.push_str(header);
                }
                if let Some(header) = Self::generated_mode_header(generated_code_mode) {
                    out.push_str(header);
                }
                out.push_str(&format!(
                    "{mode_label} ({hops} hop(s), cost {cost:.2}): {path}",
                    hops = result.hops,
                    cost = result.total_cost_millis as f64 / 1000.0,
                    path = labels.join(" → "),
                ));
                out
            }
            None => format!("No path found between '{from}' and '{to}'."),
        }
    }

    /// Summarize all graph nodes extracted from a particular file.
    ///
    /// A common agent question is "what's in this file?" — answering it via
    /// `query_graph` requires knowing at least one symbol name. This tool lets
    /// an agent orient on a file by its path alone. Path matching is
    /// case-insensitive suffix matching after normalization (Windows verbatim
    /// prefixes stripped, backslashes → forward), so both
    /// `src/foo/bar.rs` and `bar.rs` resolve to the same file if unambiguous.
    #[tool(
        description = "List all graph symbols extracted from a given source file. \
        The path is matched case-insensitively as a suffix against node source files, \
        so you can pass either a full path or just the filename. \
        Symbols are grouped by node kind (default) or by community. \
        Useful for answering \"what's in this file?\" without reading the file itself."
    )]
    fn summarize_file(
        &self,
        #[tool(param)]
        #[schemars(description = "File path (full or suffix). Case-insensitive. \
            Backslashes and forward slashes are equivalent.")]
        path: String,
        #[tool(param)]
        #[schemars(description = "Grouping: 'kind' (default) groups by file type, \
            'community' groups by community ID.")]
        group_by: Option<String>,
    ) -> String {
        let graph = self.graph();
        let needle = normalize_display_path(&path).to_lowercase();
        if needle.is_empty() {
            return "Empty path provided.".to_string();
        }

        let matches: Vec<&crate::model::Node> = graph
            .nodes()
            .filter(|n| {
                let norm = normalize_display_path(&n.source_file).to_lowercase();
                norm == needle || norm.ends_with(&format!("/{needle}"))
            })
            .collect();

        if matches.is_empty() {
            return format!("No nodes found matching file path '{path}'.");
        }

        let group_kind = group_by.as_deref().unwrap_or("kind").to_lowercase();
        let mut out = format!(
            "# File Summary: {}\n\n- Total symbols: {}\n",
            path,
            matches.len()
        );

        // Deduplicate the distinct source files matched (e.g. if `bar.rs`
        // appears in multiple directories, list them so the agent can narrow).
        let mut distinct_files: Vec<String> = matches
            .iter()
            .map(|n| normalize_display_path(&n.source_file))
            .collect();
        distinct_files.sort();
        distinct_files.dedup();
        if distinct_files.len() > 1 {
            out.push_str(&format!(
                "- Matched {} distinct files:\n",
                distinct_files.len()
            ));
            for f in &distinct_files {
                out.push_str(&format!("  - {f}\n"));
            }
        } else if let Some(f) = distinct_files.first() {
            out.push_str(&format!("- File: {f}\n"));
        }

        let mut grouped: HashMap<String, Vec<&crate::model::Node>> = HashMap::new();
        for node in &matches {
            let key = match group_kind.as_str() {
                "community" => node
                    .community
                    .map(|c| format!("Community {c}"))
                    .unwrap_or_else(|| "Community: none".to_string()),
                _ => node.file_type.to_string(),
            };
            grouped.entry(key).or_default().push(node);
        }

        let mut group_keys: Vec<_> = grouped.keys().cloned().collect();
        group_keys.sort();

        for key in group_keys {
            let nodes = grouped.get(&key).unwrap();
            out.push_str(&format!("\n## {} ({} symbol(s))\n", key, nodes.len()));
            let mut rows: Vec<(&crate::model::Node, usize)> =
                nodes.iter().map(|n| (*n, graph.degree(&n.id))).collect();
            rows.sort_by(|a, b| {
                b.1.cmp(&a.1)
                    .then_with(|| a.0.label.cmp(&b.0.label))
                    .then_with(|| a.0.id.cmp(&b.0.id))
            });
            for (n, degree) in rows {
                let loc = if n.source_location.is_empty() {
                    "unknown".to_string()
                } else {
                    n.source_location.clone()
                };
                let comm = n
                    .community
                    .map(|c| format!(" [community {c}]"))
                    .unwrap_or_default();
                out.push_str(&format!(
                    "- **{label}**{comm} — id `{id}`, span {loc}, degree {degree}\n",
                    label = n.display_label(),
                    id = n.id,
                ));
            }
        }

        out
    }

    // ── Write-back tools ────────────────────────────────────────────────────

    /// Write the current in-memory graph to disk so mutations survive restarts.
    fn persist_graph(&self) -> Result<String, String> {
        let path = {
            let guard = self.default_path.lock().map_err(|e| e.to_string())?;
            guard
                .clone()
                .ok_or_else(|| "No graph path configured — cannot persist.".to_string())?
        };

        let graph = self.graph();
        let json = crate::export::json::to_json(&graph).map_err(|e| e.to_string())?;

        // Atomic write: temp file → rename
        let tmp_path = path.with_extension("json.tmp");
        std::fs::write(&tmp_path, &json).map_err(|e| format!("write failed: {e}"))?;
        std::fs::rename(&tmp_path, &path).map_err(|e| format!("rename failed: {e}"))?;

        Ok(format!("Graph persisted to '{}'.", path.display()))
    }

    /// Parse a file_type string into a `FileType` variant.
    fn parse_file_type(s: &str) -> Result<crate::model::FileType, String> {
        match s.trim().to_lowercase().as_str() {
            "code" => Ok(crate::model::FileType::Code),
            "document" => Ok(crate::model::FileType::Document),
            "paper" => Ok(crate::model::FileType::Paper),
            "image" => Ok(crate::model::FileType::Image),
            "rationale" => Ok(crate::model::FileType::Rationale),
            other => Err(format!(
                "Unknown file_type '{other}'. Expected one of: code, document, paper, image, rationale."
            )),
        }
    }

    /// Parse a confidence string into a `Confidence` variant.
    fn parse_confidence(s: &str) -> Result<crate::model::Confidence, String> {
        let upper = s.trim().to_uppercase();
        match upper.as_str() {
            "EXTRACTED" => Ok(crate::model::Confidence::Extracted),
            "INFERRED" => Ok(crate::model::Confidence::Inferred),
            "AMBIGUOUS" => Ok(crate::model::Confidence::Ambiguous),
            _ => Err(format!(
                "Unknown confidence '{s}'. Expected EXTRACTED, INFERRED, or AMBIGUOUS."
            )),
        }
    }

    /// Add or update a node in the knowledge graph.
    #[tool(description = "Add or update a node in the knowledge graph. \
        Use this to register architectural concepts, rationale nodes, or \
        other logical entities the AST extractor does not capture. \
        If a node with the given ID already exists, it is updated in place. \
        The graph is persisted to disk immediately.")]
    fn add_node(
        &self,
        #[tool(param)]
        #[schemars(description = "Stable node identifier (normalized automatically)")]
        id: String,
        #[tool(param)]
        #[schemars(description = "Human-readable display name")]
        label: String,
        #[tool(param)]
        #[schemars(
            description = "Node type: 'code', 'document', 'paper', 'image', or 'rationale'"
        )]
        file_type: String,
        #[tool(param)]
        #[schemars(description = "Relative source file path to associate with this node")]
        source_file: String,
        #[tool(param)]
        #[schemars(description = "Optional source location hint, e.g. 'L42'")]
        source_location: Option<String>,
        #[tool(param)]
        #[schemars(description = "Optional qualified (scope-prefixed) label for disambiguation")]
        qualified_label: Option<String>,
    ) -> String {
        let ft = match Self::parse_file_type(&file_type) {
            Ok(ft) => ft,
            Err(e) => return e,
        };

        let mut node = crate::model::Node::new(&id, &label, ft, &source_file);
        if let Some(loc) = source_location {
            node = node.with_source_location(loc);
        }
        if let Some(ql) = qualified_label {
            node = node.with_qualified_label(ql);
        }

        let mut graph = (*self.graph()).clone();
        let was_update = graph.contains_node(&node.id);
        graph.upsert_node(node.clone());

        // Swap the mutated graph into place before persisting.
        self.graph_store.store(Arc::new(graph));

        let persist_msg = self
            .persist_graph()
            .unwrap_or_else(|e| format!("(persist warning: {e})"));

        let action = if was_update { "Updated" } else { "Added" };
        format!(
            "{action} node '{}' (id: {}, file_type: {}). {persist_msg}",
            node.display_label(),
            node.id,
            node.file_type
        )
    }

    /// Add an edge between two nodes in the knowledge graph.
    #[tool(description = "Add a directed edge between two existing nodes. \
        Resolves endpoints by ID first, then by case-insensitive label match. \
        Use this when you have confirmed a relationship through code inspection \
        — set confidence to EXTRACTED for relationships you have verified. \
        The graph is persisted to disk immediately.")]
    fn add_edge(
        &self,
        #[tool(param)]
        #[schemars(description = "Source node ID or label")]
        source: String,
        #[tool(param)]
        #[schemars(description = "Target node ID or label")]
        target: String,
        #[tool(param)]
        #[schemars(
            description = "Relation type, e.g. 'calls', 'uses', 'delegates_to', 'rationale_for'"
        )]
        relation: String,
        #[tool(param)]
        #[schemars(description = "Confidence: 'EXTRACTED' (verified by inspection), \
            'INFERRED' (strong hint), or 'AMBIGUOUS' (uncertain)")]
        confidence: String,
        #[tool(param)]
        #[schemars(description = "Source file where this relationship was observed")]
        source_file: String,
        #[tool(param)]
        #[schemars(description = "Optional source location hint, e.g. 'L72'")]
        source_location: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Optional traversal weight override (defaults to confidence-based)"
        )]
        weight: Option<f64>,
    ) -> String {
        let src_id = match self.resolve_id(&source) {
            Some(id) => id,
            None => return format!("Source node '{source}' not found."),
        };
        let tgt_id = match self.resolve_id(&target) {
            Some(id) => id,
            None => return format!("Target node '{target}' not found."),
        };

        let conf = match Self::parse_confidence(&confidence) {
            Ok(c) => c,
            Err(e) => return e,
        };

        let mut edge =
            crate::model::Edge::new(&src_id, &tgt_id, &relation, conf.clone(), &source_file);
        if let Some(loc) = source_location {
            edge.source_location = Some(loc);
        }
        if let Some(w) = weight {
            edge.weight = w.clamp(0.0, 1.0);
        }

        let mut graph = (*self.graph()).clone();
        let added = graph.add_edge(edge);
        if !added {
            return format!(
                "Edge not added: a logically equivalent edge already exists between '{}' and '{}'.",
                src_id, tgt_id
            );
        }

        self.graph_store.store(Arc::new(graph));
        let persist_msg = self
            .persist_graph()
            .unwrap_or_else(|e| format!("(persist warning: {e})"));

        format!(
            "Added edge: {src} --[{relation}]--> {tgt} (confidence: {conf}). {persist_msg}",
            src = src_id,
            tgt = tgt_id,
            relation = relation.to_lowercase(),
            conf = conf,
        )
    }

    /// Remove edges matching the given criteria from the knowledge graph.
    #[tool(description = "Remove edges between two nodes. \
        If a relation filter is provided, only matching edges are removed; \
        otherwise all edges between the two nodes are removed. \
        Resolves endpoints by ID first, then by case-insensitive label match. \
        Use this to correct false positives or remove stale relationships. \
        The graph is persisted to disk immediately.")]
    fn remove_edge(
        &self,
        #[tool(param)]
        #[schemars(description = "Source node ID or label")]
        source: String,
        #[tool(param)]
        #[schemars(description = "Target node ID or label")]
        target: String,
        #[tool(param)]
        #[schemars(
            description = "Optional relation filter. If omitted, all edges between source and target are removed."
        )]
        relation: Option<String>,
    ) -> String {
        let src_id = match self.resolve_id(&source) {
            Some(id) => id,
            None => return format!("Source node '{source}' not found."),
        };
        let tgt_id = match self.resolve_id(&target) {
            Some(id) => id,
            None => return format!("Target node '{target}' not found."),
        };

        let mut graph = (*self.graph()).clone();
        let src_idx = match graph.node_index(&src_id) {
            Some(idx) => idx,
            None => return format!("Source node '{src_id}' not found in graph after resolve."),
        };
        let tgt_idx = match graph.node_index(&tgt_id) {
            Some(idx) => idx,
            None => return format!("Target node '{tgt_id}' not found in graph after resolve."),
        };

        // Collect edge indices matching the (optional) relation filter.
        let edge_indices: Vec<_> = graph
            .inner()
            .edges_connecting(src_idx, tgt_idx)
            .filter(|eref| {
                if let Some(ref rel) = relation {
                    eref.weight().relation.to_lowercase() == rel.trim().to_lowercase()
                } else {
                    true
                }
            })
            .map(|eref| eref.id())
            .collect();

        let count = edge_indices.len();
        if count == 0 {
            return format!(
                "No matching edges found between '{}' and '{}'.",
                src_id, tgt_id
            );
        }

        for ei in edge_indices {
            graph.inner.remove_edge(ei);
        }

        self.graph_store.store(Arc::new(graph));
        let persist_msg = self
            .persist_graph()
            .unwrap_or_else(|e| format!("(persist warning: {e})"));

        let rel_desc = relation
            .as_deref()
            .map(|r| format!(" (relation: {r})"))
            .unwrap_or_default();
        format!(
            "Removed {count} edge(s) between '{src_id}' and '{tgt_id}'{rel_desc}. {persist_msg}",
            src_id = src_id,
            tgt_id = tgt_id,
        )
    }

    /// Hot-reload the graph backing this server.
    ///
    /// Reads `graph_path` from disk (falling back to the path the server was
    /// launched with) and atomically swaps it in. Active tool handlers that
    /// already took a snapshot via `self.graph()` keep seeing the old graph
    /// until their request finishes — swaps are lock-free on the read side.
    #[tool(
        description = "Reload the knowledge graph from a graph.json file without restarting the \
        MCP server. If no path is given, reloads from the path the server was launched with. \
        Use this to point a running server at a different repository's graph, or to pick up \
        changes after re-running `gm run`."
    )]
    fn reload_graph(
        &self,
        #[tool(param)]
        #[schemars(
            description = "Optional filesystem path to a graph.json file. If omitted, \
            reloads from the path the server was originally launched with."
        )]
        graph_path: Option<String>,
    ) -> String {
        use std::path::Path;

        let resolved_path: PathBuf = match graph_path.as_deref() {
            Some(explicit) => PathBuf::from(explicit),
            None => {
                let guard = match self.default_path.lock() {
                    Ok(g) => g,
                    Err(poisoned) => poisoned.into_inner(),
                };
                match guard.as_ref() {
                    Some(p) => p.clone(),
                    None => {
                        return "No graph path provided and no default path was recorded at \
                                server launch."
                            .to_string();
                    }
                }
            }
        };

        let new_graph = match crate::export::json::load_graph(Path::new(&resolved_path)) {
            Ok(g) => g,
            Err(e) => {
                return format!(
                    "Failed to load graph from '{}': {}",
                    resolved_path.display(),
                    e
                );
            }
        };

        let n = new_graph.node_count();
        let e = new_graph.edge_count();
        self.graph_store.store(Arc::new(new_graph));

        if let Ok(mut guard) = self.default_path.lock() {
            *guard = Some(resolved_path.clone());
        }

        format!(
            "Reloaded graph from '{}': {} nodes, {} edges.",
            resolved_path.display(),
            n,
            e
        )
    }
}

// ── ServerHandler ─────────────────────────────────────────────────────────────

#[tool(tool_box)]
impl ServerHandler for GrapheniumServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Graphenium knowledge graph server. \
                 Use query_graph to explore by keywords, get_node/get_neighbors for details, \
                 get_community to inspect clusters, god_nodes for hotspots, \
                 graph_stats for counts, architecture_summary for repo-level structure, \
                 shortest_path for connectivity, summarize_file to list all symbols in a \
                 source file by path, and reload_graph to hot-swap the underlying graph. \
                 Write tools: add_node to register new entities, add_edge to record \
                 relationships you have confirmed through code inspection (use EXTRACTED \
                 confidence when verified), and remove_edge to correct false positives \
                 or remove stale relationships. All writes persist to disk immediately."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Edge, FileType, Node};

    fn make_server() -> GrapheniumServer {
        let mut g = GrapheniumGraph::new();
        let mut a = Node::new("src_alpha", "Alpha", FileType::Code, "src/alpha.rs")
            .with_source_location("L10:C1-L24:C2");
        a.community = Some(0);
        let mut b = Node::new("src_beta", "Beta", FileType::Code, "src/beta.rs");
        b.community = Some(0);
        let c = Node::new("docs_guide", "Guide", FileType::Document, "docs/guide.md");
        g.upsert_node(a);
        g.upsert_node(b);
        g.upsert_node(c);
        g.add_edge(Edge::extracted(
            "src_alpha",
            "src_beta",
            "calls",
            "src/alpha.rs",
        ));
        GrapheniumServer::new(g)
    }

    fn make_scoped_server() -> GrapheniumServer {
        let mut g = GrapheniumGraph::new();
        g.upsert_node(Node::new(
            "src_controller",
            "Controller",
            FileType::Code,
            "src/Controller.cs",
        ));
        g.upsert_node(Node::new(
            "src_service",
            "OrderService",
            FileType::Code,
            "src/Services.cs",
        ));
        g.upsert_node(Node::new(
            "src_worker",
            "Worker",
            FileType::Code,
            "src/Worker.cs",
        ));
        g.upsert_node(Node::new(
            "tests_helper",
            "TestHelper",
            FileType::Code,
            "tests/TestHelper.cs",
        ));
        g.add_edge(Edge::extracted(
            "src_controller",
            "src_service",
            "calls",
            "src/Controller.cs",
        ));
        g.add_edge(Edge::extracted(
            "src_worker",
            "src_service",
            "calls",
            "src/Worker.cs",
        ));
        g.add_edge(Edge::extracted(
            "tests_helper",
            "src_service",
            "calls",
            "tests/TestHelper.cs",
        ));
        GrapheniumServer::new(g)
    }

    fn make_generated_server() -> GrapheniumServer {
        let mut g = GrapheniumGraph::new();
        g.upsert_node(Node::new(
            "template_screen",
            "TemplateScreen",
            FileType::Code,
            "Data/Templates/MainScreen.view.cs",
        ));
        g.upsert_node(Node::new(
            "real_service",
            "RealService",
            FileType::Code,
            "src/RealService.cs",
        ));
        g.add_edge(Edge::extracted(
            "template_screen",
            "real_service",
            "calls",
            "Data/Templates/MainScreen.view.cs",
        ));
        GrapheniumServer::new(g)
    }

    fn make_ast_only_generated_server() -> GrapheniumServer {
        let mut g = GrapheniumGraph::new();
        g.set_ast_only(true);
        g.upsert_node(Node::new(
            "template_screen",
            "TemplateScreen",
            FileType::Code,
            "Data/Templates/MainScreen.view.cs",
        ));
        g.upsert_node(Node::new(
            "system",
            "System",
            FileType::Code,
            "src/FrameworkBridge.cs",
        ));
        g.upsert_node(Node::new(
            "real_service",
            "RealService",
            FileType::Code,
            "src/RealService.cs",
        ));
        g.add_edge(Edge::extracted(
            "template_screen",
            "system",
            "imports",
            "Data/Templates/MainScreen.view.cs",
        ));
        g.add_edge(Edge::extracted(
            "system",
            "real_service",
            "imports",
            "src/FrameworkBridge.cs",
        ));
        GrapheniumServer::new(g)
    }

    fn make_architecture_server() -> GrapheniumServer {
        let mut g = GrapheniumGraph::new();

        let mut api = Node::new(
            "api_controller",
            "ApiController",
            FileType::Code,
            "src/api/Controller.cs",
        );
        api.community = Some(0);
        let mut api_service = Node::new(
            "api_service",
            "ApiService",
            FileType::Code,
            "src/api/Service.cs",
        );
        api_service.community = Some(0);
        let mut data_repo = Node::new(
            "data_repo",
            "DataRepository",
            FileType::Code,
            "src/data/Repository.cs",
        );
        data_repo.community = Some(1);
        let mut data_model = Node::new(
            "data_model",
            "DataModel",
            FileType::Code,
            "src/data/Model.cs",
        );
        data_model.community = Some(1);
        let mut gateway = Node::new(
            "gateway",
            "Gateway",
            FileType::Code,
            "src/shared/Gateway.cs",
        );
        gateway.community = Some(2);

        for node in [api, api_service, data_repo, data_model, gateway] {
            g.upsert_node(node);
        }

        g.add_edge(Edge::extracted(
            "api_controller",
            "api_service",
            "calls",
            "src/api/Controller.cs",
        ));
        g.add_edge(Edge::extracted(
            "data_repo",
            "data_model",
            "contains",
            "src/data/Repository.cs",
        ));
        g.add_edge(Edge::extracted(
            "api_service",
            "gateway",
            "uses",
            "src/api/Service.cs",
        ));
        g.add_edge(Edge::extracted(
            "gateway",
            "data_repo",
            "uses",
            "src/shared/Gateway.cs",
        ));

        GrapheniumServer::new(g)
    }

    #[test]
    fn query_graph_returns_text() {
        let s = make_server();
        let result = s.query_graph(
            "Alpha".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert!(result.contains("Alpha"));
        assert!(result.contains("Match: direct keyword match"));
    }

    #[test]
    fn query_graph_honors_path_scope() {
        let s = make_scoped_server();
        let result = s.query_graph(
            "service".to_string(),
            None,
            None,
            None,
            Some("tests/".to_string()),
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert!(result.contains("TestHelper"));
        assert!(!result.contains("Controller"));
    }

    #[test]
    fn query_graph_honors_node_type_and_relation_filters() {
        let s = make_server();
        let result = s.query_graph(
            "Guide".to_string(),
            None,
            None,
            None,
            None,
            None,
            Some(vec!["document".to_string()]),
            None,
            None,
            None,
            None,
        );
        assert!(result.contains("Guide"));

        let relation_filtered = s.query_graph(
            "Alpha".to_string(),
            None,
            None,
            None,
            None,
            None,
            Some(vec!["code".to_string()]),
            Some(vec!["imports".to_string()]),
            None,
            None,
            None,
        );
        assert!(!relation_filtered.contains("`calls`"));
        assert!(relation_filtered.contains("Match: direct keyword match"));
    }

    #[test]
    fn query_graph_honors_generated_code_mode() {
        let s = make_generated_server();
        let excluded = s.query_graph(
            "template service".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some("exclude".to_string()),
            None,
        );
        assert!(excluded.contains("generated/template/vendor paths excluded"));
        assert!(excluded.contains("RealService"));
        assert!(!excluded.contains("TemplateScreen"));

        let only = s.query_graph(
            "template service".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some("only".to_string()),
            None,
        );
        assert!(only.contains("only generated/template/vendor paths included"));
        assert!(only.contains("TemplateScreen"));
        assert!(!only.contains("RealService"));
    }

    #[test]
    fn query_graph_auto_tunes_for_ast_only_graphs() {
        let s = make_ast_only_generated_server();
        let result = s.query_graph(
            "template service system".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert!(result.contains("AST-only tuning active"));
        assert!(result.contains("generated/template/vendor paths excluded"));
        assert!(!result.contains("TemplateScreen"));
    }

    #[test]
    fn get_node_by_id() {
        let s = make_server();
        let result = s.get_node("src_alpha".to_string());
        assert!(result.contains("Alpha"));
        assert!(result.contains("src/alpha.rs"));
        assert!(result.contains("Span: L10:C1-L24:C2"));
    }

    #[test]
    fn get_node_by_label() {
        let s = make_server();
        let result = s.get_node("Beta".to_string());
        assert!(result.contains("src_beta"));
    }

    #[test]
    fn get_node_not_found() {
        let s = make_server();
        let result = s.get_node("NoSuchNode".to_string());
        assert!(result.contains("not found"));
    }

    #[test]
    fn get_neighbors_returns_connected() {
        let s = make_server();
        let result = s.get_neighbors("src_alpha".to_string(), None);
        assert!(result.contains("Beta"));
        assert!(result.contains("calls"));
    }

    #[test]
    fn get_neighbors_relation_filter() {
        let s = make_server();
        let result = s.get_neighbors("src_alpha".to_string(), Some("imports".to_string()));
        assert!(result.contains("No neighbors found"));
    }

    #[test]
    fn get_neighbors_deduplicates_duplicate_rows() {
        let mut g = GrapheniumGraph::new();
        g.upsert_node(Node::new(
            "src_alpha",
            "Alpha",
            FileType::Code,
            "src/alpha.rs",
        ));
        g.upsert_node(Node::new("src_beta", "Beta", FileType::Code, "src/beta.rs"));
        g.add_edge(Edge::extracted(
            "src_alpha",
            "src_beta",
            "calls",
            "src/alpha.rs",
        ));

        let src_idx = g.id_index["src_alpha"];
        let tgt_idx = g.id_index["src_beta"];
        g.inner.add_edge(
            src_idx,
            tgt_idx,
            Edge::extracted("src_alpha", "src_beta", "calls", "src/alpha.rs"),
        );

        let s = GrapheniumServer::new(g);
        let result = s.get_neighbors("src_alpha".to_string(), None);
        assert_eq!(result.matches("**Beta** via `calls`").count(), 1);
        assert!(result.contains("Total: 1 neighbor(s)"));
    }

    #[test]
    fn get_community_returns_members() {
        let s = make_server();
        let result = s.get_community(0, None);
        assert!(result.contains("# Community 0"));
        assert!(result.contains("## Representative Nodes"));
        assert!(result.contains("## Representative Files"));
        assert!(result.contains("## Dominant Internal Relations"));
        assert!(result.contains("Alpha"));
    }

    #[test]
    fn get_community_can_include_full_member_list() {
        let s = make_server();
        let result = s.get_community(0, Some(true));
        assert!(result.contains("## Members"));
        assert!(result.contains("Alpha"));
        assert!(result.contains("Beta"));
    }

    #[test]
    fn get_community_not_found() {
        let s = make_server();
        let result = s.get_community(99, None);
        assert!(result.contains("not found"));
    }

    #[test]
    fn god_nodes_returns_hubs() {
        let s = make_server();
        let result = s.god_nodes(Some(5), None, None, None, None, None);
        // Alpha has degree 1 (connected to Beta), which may be filtered as stub
        // At minimum, we should not panic
        assert!(!result.is_empty());
    }

    #[test]
    fn god_nodes_honors_path_scope() {
        let s = make_scoped_server();
        let result = s.god_nodes(Some(5), Some("tests/".to_string()), None, None, None, None);
        assert!(result.contains("selected filter scope"));

        let src_result = s.god_nodes(Some(5), Some("src/".to_string()), None, None, None, None);
        assert!(src_result.contains("OrderService"));
    }

    #[test]
    fn god_nodes_honors_node_type_filter() {
        let s = make_server();
        let result = s.god_nodes(
            Some(5),
            None,
            None,
            Some(vec!["document".to_string()]),
            None,
            None,
        );
        assert!(result.contains("selected filter scope") || result.contains("too small"));
    }

    #[test]
    fn graph_stats_has_counts() {
        let s = make_server();
        let result = s.graph_stats(None, None, None, None, None);
        assert!(result.contains("Nodes: 3"));
        assert!(result.contains("Edges: 1"));
    }

    #[test]
    fn graph_stats_honors_path_scope() {
        let s = make_scoped_server();
        let result = s.graph_stats(Some("tests/".to_string()), None, None, None, None);
        assert!(result.contains("Nodes: 1"));
        assert!(result.contains("Edges: 0"));
    }

    #[test]
    fn graph_stats_honors_node_type_filter() {
        let s = make_server();
        let result = s.graph_stats(None, None, Some(vec!["document".to_string()]), None, None);
        assert!(result.contains("Nodes: 1"));
        assert!(result.contains("document: 1"));
    }

    #[test]
    fn architecture_summary_reports_major_sections() {
        let s = make_architecture_server();
        let result = s.architecture_summary(None, None, None, None, None, Some(3));
        assert!(result.contains("# Architecture Summary"));
        assert!(result.contains("## Largest Communities"));
        assert!(result.contains("## Cross-Community Connectors"));
        assert!(result.contains("## Architectural Hotspots"));
        assert!(result.contains("ApiController") || result.contains("ApiService"));
    }

    #[test]
    fn architecture_summary_honors_generated_code_mode() {
        let s = make_generated_server();
        let result = s.architecture_summary(None, None, None, Some("only".to_string()), None, None);
        assert!(result.contains("only generated/template/vendor paths included"));
    }

    #[test]
    fn architecture_summary_auto_tunes_for_ast_only_graphs() {
        let s = make_ast_only_generated_server();
        let result = s.architecture_summary(None, None, None, None, None, None);
        assert!(result.contains("AST-only tuning active"));
        assert!(result.contains("generated/template/vendor paths excluded"));
    }

    #[test]
    fn shortest_path_found() {
        let s = make_server();
        let result = s.shortest_path(
            "src_alpha".to_string(),
            "src_beta".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert!(result.contains("Semantic path"));
        assert!(result.contains("hop"));
        assert!(result.contains("Alpha"));
        assert!(result.contains("Beta"));
    }

    #[test]
    fn shortest_path_no_path() {
        let s = make_server();
        // Guide is disconnected
        let result = s.shortest_path(
            "src_alpha".to_string(),
            "docs_guide".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert!(result.contains("No path"));
    }

    #[test]
    fn shortest_path_same_node() {
        let s = make_server();
        let result = s.shortest_path(
            "src_alpha".to_string(),
            "Alpha".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert!(result.contains("same node"));
    }

    #[test]
    fn shortest_path_honors_relation_filters() {
        let s = make_server();
        let result = s.shortest_path(
            "src_alpha".to_string(),
            "src_beta".to_string(),
            None,
            None,
            None,
            Some(vec!["imports".to_string()]),
            None,
            None,
            None,
            None,
            None,
        );
        assert!(result.contains("No path"));
    }

    #[test]
    fn shortest_path_rejects_unknown_mode() {
        let s = make_server();
        let result = s.shortest_path(
            "src_alpha".to_string(),
            "src_beta".to_string(),
            None,
            None,
            None,
            None,
            None,
            Some("weird".to_string()),
            None,
            None,
            None,
        );
        assert!(result.contains("Unknown path mode"));
    }

    #[test]
    fn shortest_path_auto_tunes_for_ast_only_graphs() {
        let s = make_ast_only_generated_server();
        let result = s.shortest_path(
            "TemplateScreen".to_string(),
            "RealService".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert!(result.contains("No path"));
    }

    #[test]
    fn get_info_has_tool_capabilities() {
        let s = make_server();
        let info = s.get_info();
        assert!(info.instructions.is_some());
        assert!(info.capabilities.tools.is_some());
    }

    #[test]
    fn reload_graph_swaps_state() {
        use std::io::Write;

        // Build a second graph and write it to a temp file.
        let mut g2 = GrapheniumGraph::new();
        g2.upsert_node(Node::new("x_one", "One", FileType::Code, "x/one.rs"));
        g2.upsert_node(Node::new("x_two", "Two", FileType::Code, "x/two.rs"));
        g2.upsert_node(Node::new("x_three", "Three", FileType::Code, "x/three.rs"));
        g2.add_edge(Edge::extracted("x_one", "x_two", "calls", "x/one.rs"));

        let json = crate::export::json::to_json(&g2).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("reload.json");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(json.as_bytes()).unwrap();
        drop(f);

        // Initial server has 3 original nodes.
        let s = make_server();
        assert!(s.graph().contains_node("src_alpha"));
        assert!(!s.graph().contains_node("x_one"));

        let msg = s.reload_graph(Some(path.to_string_lossy().into_owned()));
        assert!(msg.contains("Reloaded"), "unexpected message: {msg}");
        assert!(msg.contains("3 nodes"), "unexpected message: {msg}");

        // After reload, new graph is in effect.
        assert!(s.graph().contains_node("x_one"));
        assert!(!s.graph().contains_node("src_alpha"));
    }

    #[test]
    fn reload_graph_missing_path_without_default() {
        let s = make_server();
        let msg = s.reload_graph(None);
        assert!(msg.contains("No graph path"), "unexpected message: {msg}");
    }

    #[test]
    fn summarize_file_lists_symbols_matching_suffix() {
        let s = make_server();
        // make_server has src/alpha.rs (Alpha), src/beta.rs (Beta), docs/guide.md (Guide)
        let out = s.summarize_file("alpha.rs".to_string(), None);
        assert!(out.contains("Alpha"), "expected Alpha symbol: {out}");
        assert!(!out.contains("Beta"), "beta.rs should not match: {out}");
    }

    #[test]
    fn summarize_file_handles_backslash_paths() {
        let s = make_server();
        // Windows-style input should normalize the same as forward slashes.
        let out = s.summarize_file(r"src\beta.rs".to_string(), None);
        assert!(out.contains("Beta"), "expected Beta symbol: {out}");
    }

    #[test]
    fn summarize_file_no_match_returns_clean_message() {
        let s = make_server();
        let out = s.summarize_file("does_not_exist.rs".to_string(), None);
        assert!(out.contains("No nodes found"));
    }

    #[test]
    fn summarize_file_group_by_community() {
        let s = make_server();
        let out = s.summarize_file("alpha.rs".to_string(), Some("community".to_string()));
        // Alpha is in community 0 in make_server.
        assert!(
            out.contains("Community 0"),
            "expected community grouping: {out}"
        );
    }

    #[test]
    fn reload_graph_reports_parse_error() {
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bogus.json");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"{ not valid json").unwrap();
        drop(f);

        let s = make_server();
        let msg = s.reload_graph(Some(path.to_string_lossy().into_owned()));
        assert!(
            msg.to_lowercase().contains("failed to load"),
            "unexpected message: {msg}"
        );
    }
}
