/// MCP server tools exposing the relational index.
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use petgraph::visit::EdgeRef;

use arc_swap::ArcSwap;
use rmcp::{
    handler::server::tool::ToolCallContext,
    model::{
        CallToolRequestParam, CallToolResult, ListToolsResult, PaginatedRequestParam,
        ServerCapabilities, ServerInfo,
    },
    service::RequestContext,
    tool, Error as McpError, ServerHandler,
};

use crate::model::GrapheniumGraph;

use super::traversal;

// ── Server struct ─────────────────────────────────────────────────────────────

/// MCP server backed by a loaded `GrapheniumGraph`.
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

    /// Reload the graph from a file path. Used by the auto-reload watcher.
    pub fn reload_from_file(
        graph_path: &Path,
        server: &Self,
    ) -> Result<(), crate::GrapheniumError> {
        let new_graph = crate::export::json::load_graph(graph_path)?;
        let n = new_graph.node_count();
        let e = new_graph.edge_count();
        server.graph_store.store(Arc::new(new_graph));
        if let Ok(mut guard) = server.default_path.lock() {
            *guard = Some(graph_path.to_path_buf());
        }
        eprintln!("[graphenium] Graph file changed, reloaded: {n} nodes, {e} edges");
        Ok(())
    }

    /// Take a lock-free snapshot of the current graph. Cheap — one atomic
    /// fetch-add on the inner `Arc`'s refcount.
    fn graph(&self) -> Arc<GrapheniumGraph> {
        self.graph_store.load_full()
    }

    fn default_graph_path(&self) -> Option<PathBuf> {
        self.default_path
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
    }

    fn staleness_warning(&self) -> Option<String> {
        let graph_path = self.default_graph_path()?;
        let graph = self.graph();
        let project_root = graph.metadata.project_root.as_deref().map(Path::new);
        super::freshness::check_staleness(&graph_path, project_root).warning_message()
    }

    fn load_arch_policy(&self) -> crate::policy::ArchPolicyConfig {
        let graph = self.graph();
        let Some(project_root) = graph.metadata.project_root.as_deref() else {
            return crate::policy::ArchPolicyConfig::default();
        };
        crate::policy::ArchPolicyConfig::load_for_project(Path::new(project_root))
            .unwrap_or_default()
    }

    fn validate_plan_preflight(&self, plan_id: &str) -> crate::harness::PreFlightReport {
        let graph = self.graph();
        let policy = self.load_arch_policy();
        crate::harness::validate_plan_preflight(&graph, plan_id, &policy.rules)
    }

    fn format_preflight_violation(
        plan_id: &str,
        report: &crate::harness::PreFlightReport,
    ) -> String {
        let violations = serde_json::to_string(&report.violations).unwrap_or_else(|_| "[]".into());
        format!(
            "PRE_FLIGHT_VIOLATION: The proposed plan '{}' violates repository architecture policies.\n\
             Violations: {violations}\n\
             Fix the plan structure before writing code.",
            plan_id
        )
    }

    /// Resolve a node by exact ID, or by case-insensitive label match.
    fn resolve_id(&self, id_or_label: &str) -> Option<String> {
        let graph = self.graph();
        if graph.contains_node(id_or_label) {
            return Some(id_or_label.to_string());
        }
        let lower = id_or_label.to_lowercase();
        let candidates: Vec<_> = graph
            .nodes()
            .filter(|n| n.label.to_lowercase() == lower)
            .collect();
        // When multiple nodes share the same label, pick the one with highest degree
        // (most likely the real implementation, not a namespace hub or stub)
        let resolved = candidates
            .iter()
            .max_by(|a, b| {
                graph
                    .degree(&a.id)
                    .cmp(&graph.degree(&b.id))
                    .then_with(|| b.id.len().cmp(&a.id.len()))
                    .then_with(|| a.id.cmp(&b.id))
            })
            .map(|n| n.id.clone());
        resolved
    }

    fn filtered_node_ids(
        &self,
        path_prefix: Option<&str>,
        exclude_path: Option<&str>,
        node_types: Option<&[String]>,
        generated_code_mode: traversal::GeneratedCodeMode,
        include_tests: bool,
    ) -> Option<HashSet<String>> {
        traversal::filtered_node_ids(
            &self.graph(),
            path_prefix,
            exclude_path,
            node_types,
            generated_code_mode,
            include_tests,
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

    /// Check if a node belongs to a module by path OR namespace/qualified_label.
    fn is_node_in_module(node: &crate::model::Node, fragment_lower: &str) -> bool {
        if fragment_lower.is_empty() {
            return false;
        }
        let source_file_normalized = node.source_file.replace('\\', "/").to_lowercase();
        if source_file_normalized.contains(fragment_lower) {
            return true;
        }
        if let Some(ref qualified) = node.qualified_label {
            if qualified.to_lowercase().contains(fragment_lower) {
                return true;
            }
        }
        if node.label.to_lowercase().contains(fragment_lower) {
            return true;
        }
        false
    }

    /// Resolve a comma-separated input string to node IDs, supporting
    /// exact ID match, then case-insensitive label search fallback.
    fn resolve_symbols_to_ids(&self, input: &str) -> (Vec<String>, Vec<String>) {
        let graph = self.graph_store.load();
        let mut resolved_ids = Vec::new();
        let mut unresolved_terms = Vec::new();

        let terms: Vec<&str> = input
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        for term in terms {
            if graph.contains_node(term) {
                resolved_ids.push(term.to_string());
                continue;
            }
            let matches: Vec<String> = graph
                .nodes()
                .filter(|n| n.label.eq_ignore_ascii_case(term))
                .map(|n| n.id.clone())
                .collect();
            if !matches.is_empty() {
                resolved_ids.extend(matches);
            } else {
                unresolved_terms.push(term.to_string());
            }
        }
        (resolved_ids, unresolved_terms)
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

    fn ast_only_tuning_header(_enabled: bool) -> Option<&'static str> {
        // Banner suppressed: the filter message already conveys AST-only tuning.
        // Re-enable by returning Some("...") when enabled.
        None
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
            let member_cap = 200;
            let total_members = representative_nodes.len();
            let display_limit = member_cap.min(total_members);
            for (node, degree) in representative_nodes.iter().take(display_limit) {
                out.push_str(&format!(
                    "- **{}** ({}, degree={}) — {}\n",
                    node.display_label(),
                    node.file_type,
                    degree,
                    node.source_file
                ));
            }
            if total_members > member_cap {
                out.push_str(&format!(
                    "\n*and {} more members (showing {} of {})*\n",
                    total_members - member_cap,
                    member_cap,
                    total_members
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
            let self_g = self.graph();
            let root = self_g.metadata.project_root.as_deref();
            for gn in hotspots.iter().take(5) {
                out.push_str(&format!(
                    "- **{}** (degree {}) — {}\n",
                    gn.display_label(),
                    gn.degree,
                    relative_path(&gn.source_file, root)
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

/// Format a plan verification report for MCP/CLI output.
fn format_plan_verification(report: &crate::harness::PlanVerificationReport) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "- Compliance: **{}**\n",
        if report.passes_compliance {
            "PASS"
        } else {
            "FAIL"
        }
    ));
    if !report.implemented_nodes.is_empty() {
        out.push_str(&format!(
            "- Implemented nodes: {}\n",
            report.implemented_nodes.join(", ")
        ));
    }
    if !report.missing_nodes.is_empty() {
        out.push_str(&format!(
            "- Missing nodes: {}\n",
            report.missing_nodes.join(", ")
        ));
    }
    if !report.implemented_edges.is_empty() {
        out.push_str(&format!(
            "- Implemented edges: {}\n",
            report.implemented_edges.join("; ")
        ));
    }
    if !report.missing_edges.is_empty() {
        out.push_str(&format!(
            "- Missing edges: {}\n",
            report.missing_edges.join("; ")
        ));
    }
    if !report.unplanned_modified_files.is_empty() {
        out.push_str(&format!(
            "- Unplanned modified files: {}\n",
            report.unplanned_modified_files.join(", ")
        ));
    }
    if !report.policy_violations.is_empty() {
        out.push_str(&format!(
            "- Physical policy violations: {}\n",
            report.policy_violations.join("; ")
        ));
    }
    out
}

/// Strip the project root prefix from an absolute path to produce a relative path.
/// If the path is already relative or the root is unknown, returns as-is.
fn relative_path(path: &str, root: Option<&str>) -> String {
    match root {
        Some(root) => {
            let normalized = path.replace('\\', "/");
            let root_norm = root.replace('\\', "/");
            let root_norm = root_norm.trim_end_matches('/');
            if let Some(rel) = normalized.strip_prefix(root_norm) {
                let rel = rel.trim_start_matches('/');
                if rel.is_empty() { "." } else { rel }.to_string()
            } else {
                path.to_string()
            }
        }
        None => path.to_string(),
    }
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

// ── Backend renderers (plain methods, not MCP tools) ──────────────────────────

/// Backend implementations backing the consolidated MCP tool surface.
/// These methods are intentionally kept out of the `#[tool(tool_box)]` block so
/// they are never registered as separate MCP endpoints.
impl GrapheniumServer {
    /// Get full details for a node by ID or label.
    fn get_node(&self, id: String) -> String {
        let resolved = match self.resolve_id(&id) {
            Some(r) => r,
            None => return format!("Node '{id}' not found."),
        };
        let graph = self.graph();
        let node = graph.node_data(&resolved).unwrap();
        let degree = graph.degree(&resolved);
        let root_str = graph.metadata.project_root.clone();
        let root = root_str.as_deref();

        // Check for duplicate labels
        let duplicate_count = graph
            .nodes()
            .filter(|n| n.label.to_lowercase() == id.to_lowercase() && n.id != resolved)
            .count();
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

        let dup_warning = if duplicate_count > 0 {
            format!("\n⚠ WARNING: {duplicate_count} other node(s) share this label. Use the ID above for exact matching.")
        } else {
            String::new()
        };

        format!(
            "**{display}**{name_suffix} ({ft})\n\
             ID: {id}\n\
             File: {sf}\n\
             Span: {loc}\n\
             Community: {comm}\n\
             Degree: {degree}\
             {dup_warning}",
            ft = node.file_type,
            id = node.id,
            sf = relative_path(&node.source_file, root),
        )
    }

    /// Get direct neighbors of a node with edge details.
    fn get_neighbors(
        &self,
        node_id: String,
        relation: Option<String>,
        max_neighbors: Option<i32>,
        extracted_only: Option<bool>,
    ) -> String {
        let extracted_only = extracted_only.unwrap_or(false);
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
            .filter(|(_, edge)| {
                if extracted_only && edge.confidence != crate::model::Confidence::Extracted {
                    return false;
                }
                match relation.as_deref() {
                    Some(filter) => edge
                        .relation
                        .to_lowercase()
                        .contains(&filter.to_lowercase()),
                    None => true,
                }
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

        let cap = max_neighbors.unwrap_or(50).max(1) as usize;
        let total = entries.len();
        if cap < total {
            entries.truncate(cap);
        }
        let displayed = entries.len();
        let self_g = self.graph();
        let root = self_g.metadata.project_root.as_deref();
        for (_, edge, nb) in &entries {
            out.push_str(&format!(
                "- **{}** via `{}` ({}, score={:.2}) — {}\n",
                nb.display_label(),
                edge.relation,
                edge.confidence,
                edge.confidence_score,
                relative_path(&nb.source_file, root)
            ));
        }

        if total == 0 {
            out.push_str("No neighbors found");
            if relation.is_some() {
                out.push_str(" with the given relation filter");
            }
            out.push('.');
        } else if cap < total {
            out.push_str(&format!(
                "\nShowing {displayed} of {total} neighbors (increase max_neighbors for more)"
            ));
        } else {
            out.push_str(&format!("\nTotal: {total} neighbor(s)"));
        }

        out
    }

    /// Get a concise summary of a community by ID.
    fn get_community(&self, community_id: i32, include_members: Option<bool>) -> String {
        self.summarize_community(community_id as usize, include_members.unwrap_or(false))
    }

    /// Return the top N most-connected hub nodes.
    fn god_nodes(
        &self,
        n: Option<i32>,
        path_prefix: Option<String>,
        exclude_path: Option<String>,
        node_types: Option<Vec<String>>,
        generated_code_mode: Option<String>,
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
            true,
        );

        if scoped.as_ref().is_some_and(HashSet::is_empty) {
            return "No hub nodes found in the selected filter scope.".to_string();
        }

        let self_g = self.graph();
        let result = crate::analyze::god_nodes_in_scope(&self_g, top_n, scoped.as_ref());

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
        // Filter out namespace aggregation hubs and test-anchored nodes
        let total_before = result.len();
        let filtered: Vec<_> = result
            .into_iter()
            .filter(|gn| {
                let node = self_g.node_data(&gn.node_id);
                if let Some(n) = node {
                    if crate::ranking::is_namespace_aggregation_node(n, &self_g) {
                        return false;
                    }
                    if crate::serve::traversal::is_test_like_path(&n.source_file) {
                        return false;
                    }
                }
                true
            })
            .collect();

        out.push_str(&format!(
            "# Top {} Hub Nodes ({} filtered)\n\n",
            filtered.len(),
            total_before.saturating_sub(filtered.len())
        ));
        let root = self_g.metadata.project_root.as_deref();
        for gn in &filtered {
            let comm = gn
                .community
                .map(|c| format!(" [community {c}]"))
                .unwrap_or_default();
            out.push_str(&format!(
                "- **{}**{} — degree {}, {}\n",
                gn.display_label(),
                comm,
                gn.degree,
                relative_path(&gn.source_file, root)
            ));
        }
        out
    }

    /// Return summary statistics for the relational index.
    fn graph_stats(
        &self,
        path_prefix: Option<String>,
        exclude_path: Option<String>,
        node_types: Option<Vec<String>>,
        generated_code_mode: Option<String>,
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
            true,
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
             ## Schema\n{provenance_rows}\n\n\
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
            provenance_rows = {
                let meta = &g.metadata;
                let mut parts = Vec::new();
                if let Some(ref v) = meta.schema_version {
                    parts.push(format!("Schema version: {v}"));
                }
                if let Some(ref v) = meta.created_at {
                    parts.push(format!("Built at: {v}"));
                }
                parts.join("\n")
            },
        ));
        out = meta_block + &out;
        out
    }

    /// Return higher-level architectural highlights for the graph.
    fn architecture_summary(
        &self,
        path_prefix: Option<String>,
        exclude_path: Option<String>,
        node_types: Option<Vec<String>>,
        generated_code_mode: Option<String>,
        ast_only_tuning: Option<bool>,
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
            true,
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
    fn shortest_path(
        &self,
        from: String,
        to: String,
        path_prefix: Option<String>,
        exclude_path: Option<String>,
        node_types: Option<Vec<String>>,
        include_relations: Option<Vec<String>>,
        exclude_relations: Option<Vec<String>>,
        mode: Option<String>,
        exclude_framework_noise: Option<bool>,
        generated_code_mode: Option<String>,
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
            true,
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
                // Build path labels with disambiguation for same-label consecutive nodes
                let path_ids: Vec<String> = result.path.clone();
                let mut labels: Vec<String> = Vec::new();
                let root = graph.metadata.project_root.as_deref();
                for (i, id) in path_ids.iter().enumerate() {
                    let Some(node) = graph.node_data(id) else {
                        continue;
                    };
                    let label = node.display_label().to_string();
                    // If next hop shares this label, append a file hint
                    let next_same = path_ids.get(i + 1).and_then(|nid| {
                        graph.node_data(nid).filter(|next| next.label == node.label)
                    });
                    if next_same.is_some() {
                        let rel = crate::serve::traversal::relative_path(&node.source_file, root);
                        labels.push(format!("{label} ({rel})"));
                    } else {
                        labels.push(label);
                    }
                }
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
                let confidence_details =
                    crate::serve::traversal::format_path_confidence(&self.graph(), &result.path);
                out.push_str(&format!(
                    "{mode_label} ({hops} hop(s), cost {cost:.2}): {path}\n\nConfidence breakdown:\n{confidence}",
                    hops = result.hops,
                    cost = result.total_cost_millis as f64 / 1000.0,
                    path = labels.join(" → "),
                    confidence = confidence_details,
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
    fn summarize_file(
        &self,
        path: String,
        group_by: Option<String>,
        _min_degree: Option<i32>,
        show_leaves: Option<bool>,
    ) -> String {
        let graph = self.graph();
        let self_g = self.graph();
        let root = self_g.metadata.project_root.as_deref();
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
                out.push_str(&format!("  - {}\n", relative_path(f, root)));
            }
        } else if let Some(f) = distinct_files.first() {
            out.push_str(&format!("- File: {}\n", relative_path(f, root)));
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

            let show_leaves = show_leaves.unwrap_or(false);
            let hubs: Vec<_> = rows.iter().filter(|(_, d)| *d > 5).collect();
            let leaves: Vec<_> = rows.iter().filter(|(_, d)| *d <= 5).collect();

            for (n, degree) in &hubs {
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

            if show_leaves {
                for (n, degree) in &leaves {
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
            } else if !leaves.is_empty() {
                out.push_str(&format!(
                    "\n_Omitted {} low-degree leaf symbols to save tokens. Use `show_leaves=true` to expand._\n",
                    leaves.len()
                ));
            }
        }

        out
    }

    // ── analyse_symbol ─────────────────────────────────────────────────────────

    fn analyse_symbol(&self, symbol: String) -> String {
        let id = match self.resolve_id(&symbol) {
            Some(id) => id,
            None => return format!("Symbol '{symbol}' not found."),
        };

        let graph = self.graph();

        let node = match graph.node_data(&id) {
            Some(n) => n,
            None => {
                return format!("Symbol '{symbol}' (resolved to '{id}') not found in graph.");
            }
        };

        let degree = graph.degree(&id);
        let comm_str = node
            .community
            .map(|c| format!("community {c}"))
            .unwrap_or_else(|| "none".to_string());

        // ── Node info header ────────────────────────────────────────────────
        let mut out = format!(
            "## Node: {label}\n\n\
             - **ID**: `{id}`\n\
             - **Type**: {ft}\n\
             - **File**: {file}\n\
             - **Location**: {loc}\n\
             - **Degree**: {degree}\n\
             - **Community**: {comm_str}\n\n",
            label = node.display_label(),
            id = node.id,
            ft = node.file_type,
            file = crate::serve::traversal::relative_path(
                &node.source_file,
                graph.metadata.project_root.as_deref()
            ),
            loc = node.source_location,
            degree = degree,
            comm_str = comm_str,
        );

        // ── Gather and classify edges ──────────────────────────────────────
        // Each entry: (relation, source, target, confidence_label)
        let mut behavioral: Vec<(String, String, String, String)> = Vec::new();
        let mut structural: Vec<(String, String, String, String)> = Vec::new();

        for edge in graph.edges_iter() {
            if edge.source != id && edge.target != id {
                continue;
            }
            let conf_label = format!("{:?}", edge.confidence);

            match edge.relation.as_str() {
                "calls" | "uses" | "inherits" | "implements" => {
                    behavioral.push((
                        edge.relation.clone(),
                        edge.source.clone(),
                        edge.target.clone(),
                        conf_label,
                    ));
                }
                "contains" | "imports" => {
                    structural.push((
                        edge.relation.clone(),
                        edge.source.clone(),
                        edge.target.clone(),
                        conf_label,
                    ));
                }
                _ => {
                    behavioral.push((
                        edge.relation.clone(),
                        edge.source.clone(),
                        edge.target.clone(),
                        conf_label,
                    ));
                }
            }
        }

        // ── Helper to sort by confidence then relation ─────────────────────
        let sort_edges = |edges: &mut Vec<(String, String, String, String)>| {
            edges.sort_by(|a, b| {
                let a_prio = match a.3.as_str() {
                    "Extracted" => 0,
                    "Inferred" => 1,
                    _ => 2,
                };
                let b_prio = match b.3.as_str() {
                    "Extracted" => 0,
                    "Inferred" => 1,
                    _ => 2,
                };
                a_prio.cmp(&b_prio).then_with(|| a.0.cmp(&b.0))
            });
        };

        // ── Behavioral connections (capped at 10) ──────────────────────────
        out.push_str("### Behavioral Connections\n\n");
        if behavioral.is_empty() {
            out.push_str("None.\n\n");
        } else {
            sort_edges(&mut behavioral);
            for (rel, src, tgt, conf) in behavioral.iter().take(10) {
                if *src == id {
                    out.push_str(&format!("- `{rel}` → `{tgt}` (confidence: {conf})\n"));
                } else {
                    out.push_str(&format!("- `{rel}` ← `{src}` (confidence: {conf})\n"));
                }
            }
            if behavioral.len() > 10 {
                out.push_str(&format!(
                    "- *… and {} more behavioral connections*\n",
                    behavioral.len() - 10
                ));
            }
            out.push('\n');
        }

        // ── Structural connections (capped at 5) ───────────────────────────
        out.push_str("### Structural Connections\n\n");
        if structural.is_empty() {
            out.push_str("None.\n\n");
        } else {
            sort_edges(&mut structural);
            for (rel, src, tgt, conf) in structural.iter().take(5) {
                if *src == id {
                    out.push_str(&format!("- `{rel}` → `{tgt}` (confidence: {conf})\n"));
                } else {
                    out.push_str(&format!("- `{rel}` ← `{src}` (confidence: {conf})\n"));
                }
            }
            if structural.len() > 5 {
                out.push_str(&format!(
                    "- *… and {} more structural connections*\n",
                    structural.len() - 5
                ));
            }
            out.push('\n');
        }

        // ── Trust profile ──────────────────────────────────────────────────
        out.push_str(&format!(
            "**Trust profile**: {} edges ({} behavioral, {} structural).\n",
            behavioral.len() + structural.len(),
            behavioral.len(),
            structural.len(),
        ));

        out
    }

    // ── module_dependencies ────────────────────────────────────────────────────

    fn module_dependencies(&self, module_a: String, module_b: String) -> String {
        let graph = self.graph();
        let a_lower = module_a.to_lowercase();
        let b_lower = module_b.to_lowercase();

        let mut grouped: HashMap<String, Vec<String>> = HashMap::new();
        let mut total = 0usize;

        for edge in graph.edges_iter() {
            let src_node = match graph.node_data(&edge.source) {
                Some(n) => n,
                None => continue,
            };
            let tgt_node = match graph.node_data(&edge.target) {
                Some(n) => n,
                None => continue,
            };

            // Check dual: path-based OR namespace/qualified_label matching
            // Also include depends_on edges for C# project references
            let src_matches = Self::is_node_in_module(src_node, &a_lower);
            let tgt_matches = Self::is_node_in_module(tgt_node, &b_lower)
                || (a_lower == b_lower && Self::is_node_in_module(tgt_node, &a_lower));

            if src_matches && tgt_matches {
                total += 1;
                grouped
                    .entry(edge.relation.clone())
                    .or_default()
                    .push(format!(
                        "- `{}` {} → {}",
                        edge.relation,
                        src_node.display_label(),
                        tgt_node.display_label(),
                    ));
            }
        }

        let mut out = format!(
            "# Module Dependencies: `{ma}` → `{mb}`\n\n\
             {total} connections from {ma} to {mb}.\n",
            ma = module_a,
            mb = module_b,
            total = total,
        );

        if total == 0 {
            // Try namespace-hub bridging: edges through import aggregation hubs
            let mut bridged = 0usize;
            for n in graph.nodes() {
                if !crate::ranking::is_namespace_aggregation_node(n, &graph) {
                    continue;
                }
                let mut a_nodes: Vec<String> = Vec::new();
                let mut b_nodes: Vec<String> = Vec::new();
                for edge in graph.edges_iter() {
                    let src_node = match graph.node_data(&edge.source) {
                        Some(node) => node,
                        None => continue,
                    };
                    let _tgt_node = match graph.node_data(&edge.target) {
                        Some(node) => node,
                        None => continue,
                    };
                    // Hub is target of import: edge.source imports the hub
                    if edge.target == n.id && edge.relation == "imports" {
                        if Self::is_node_in_module(src_node, &a_lower) {
                            a_nodes.push(format!(
                                "{} ({})",
                                src_node.display_label(),
                                src_node.source_file
                            ));
                        }
                        if Self::is_node_in_module(src_node, &b_lower) {
                            b_nodes.push(format!(
                                "{} ({})",
                                src_node.display_label(),
                                src_node.source_file
                            ));
                        }
                    }
                }
                if !a_nodes.is_empty() && !b_nodes.is_empty() {
                    bridged += 1;
                    out.push_str(&format!(
                        "- `imports` bridging via namespace **{}**: {} cross-module edges\n",
                        n.label,
                        a_nodes.len() * b_nodes.len()
                    ));
                }
            }

            if bridged > 0 {
                out.push_str(&format!(
                    "\nTotal: {} cross-module connections via {} bridging hubs.\n",
                    bridged, bridged
                ));
            } else {
                out.push_str("\nNo direct dependencies found between these modules.\n");
                let calls_resolved = graph.edges_iter().any(|e| {
                    e.relation == "calls" && e.resolution_status.as_deref() == Some("resolved")
                });
                if graph.is_ast_only() && !calls_resolved {
                    out.push_str(
                        "[NOTE] This graph is in AST-only mode (calls 0% resolved). Cross-module coupling \
                        relying on function calls or implicit references cannot be detected. If these modules \
                        rely on implicit runtime coupling, run Graphenium with a semantic pass (`gm run .`).\n\
                        Additionally, verify that namespace queries match the project namespace labels.\n"
                    );
                }
            }
            return out;
        }

        let mut relations: Vec<_> = grouped.keys().cloned().collect();
        relations.sort();
        for rel in relations {
            let entries = grouped.get(&rel).unwrap();
            out.push_str(&format!("\n### `{rel}` ({count})\n", count = entries.len()));
            for entry in entries {
                out.push_str(entry);
                out.push('\n');
            }
        }

        out
    }

    /// Try to load a snapshot graph from a list of candidate paths.
    /// Returns Ok(graph) on first success, or None if no path exists.
    fn load_snapshot_graph(
        candidates: &[std::path::PathBuf],
        _name: &str,
    ) -> Option<crate::model::GrapheniumGraph> {
        for path in candidates {
            if path.exists() {
                match crate::export::json::load_graph(path) {
                    Ok(g) => return Some(g),
                    Err(_) => continue,
                }
            }
        }
        None
    }

    // ── explain_change ────────────────────────────────────────────────────────────

    fn explain_change(&self, symbol: String) -> String {
        let graph = self.graph();
        let target_id = match self.resolve_id(&symbol) {
            Some(id) => id,
            None => {
                return format!(
                    "Error: Could not resolve symbol '{}' to a node in the graph.",
                    symbol
                )
            }
        };
        let Some(explanation) = crate::serve::traversal::explain_subsystem(&graph, &target_id)
        else {
            return format!(
                "Error: Failed to generate architectural explanation for '{}'.",
                symbol
            );
        };
        crate::serve::traversal::format_explanation_report(&graph, &explanation)
    }

    // ── what_changed ──────────────────────────────────────────────────────────

    fn what_changed(&self, snapshot_name: Option<String>, budget: Option<usize>) -> String {
        let name = snapshot_name.unwrap_or_else(|| "backup".to_string());

        // Try graphenium-snapshots/<name>.json first, then <name>.json
        let candidate_paths = [
            PathBuf::from(format!("graphenium-snapshots/{name}.json")),
            PathBuf::from(format!("{name}.json")),
        ];

        let old_graph = match Self::load_snapshot_graph(&candidate_paths, &name) {
            Some(g) => g,
            None => {
                return format!(
                    "Snapshot '{name}' not found. Looked in:\n\
                     - graphenium-snapshots/{name}.json\n\
                     - {name}.json"
                );
            }
        };

        let new_graph = self.graph();
        let changes = crate::analyze::impact::symbol_inventory_diff(&old_graph, &new_graph);
        // Short-circuit for very large deltas: count-only summary to prevent OOM
        let _budget_max = budget.unwrap_or(10000);
        if changes.len() > 5000 {
            let changes_len = changes.len();
            let removed = changes
                .iter()
                .filter(|c| matches!(c, crate::analyze::impact::SymbolChange::Removed { .. }))
                .count();
            let moved = changes
                .iter()
                .filter(|c| {
                    matches!(
                        c,
                        crate::analyze::impact::SymbolChange::CommunityChanged { .. }
                    )
                })
                .count();
            let added = changes.len() - removed - moved;
            return format!(
                "# What Changed (snapshot: `{name}`)\n**Delta too large to expand** ({changes_len} changes). Summary:\n- Removed: {removed}\n- Community moves: {moved}\n- Added: {added}\n\nThe full diff would require multi-MB output. Increase the `budget` parameter or use a shorter snapshot window."
            );
        }
        let impact = if changes.len() > 200 {
            crate::analyze::impact::ImpactReport {
                changed_symbols: Vec::new(),
                downstream_nodes: Vec::new(),
                affected_communities: Vec::new(),
                extracted_edges: 0,
                inferred_edges: 0,
                ambiguous_edges: 0,
            }
        } else {
            crate::analyze::impact::downstream_impact(&new_graph, &changes)
        };

        let _budget_max = budget.unwrap_or(10000);
        let mut out = format!("# What Changed (snapshot: `{name}`)\n\n");

        // Separate changes by type
        let mut removed: Vec<&crate::analyze::impact::SymbolChange> = Vec::new();
        let mut moved: Vec<&crate::analyze::impact::SymbolChange> = Vec::new();
        let mut added: Vec<&crate::analyze::impact::SymbolChange> = Vec::new();

        for change in &changes {
            match change {
                crate::analyze::impact::SymbolChange::Removed { .. } => removed.push(change),
                crate::analyze::impact::SymbolChange::CommunityChanged { .. } => moved.push(change),
                crate::analyze::impact::SymbolChange::Added { .. } => added.push(change),
            }
        }

        // ── REMOVED (highest risk) ─────────────────────────────────────────
        if removed.is_empty() {
            out.push_str("## ❌ Removed Symbols\n\nNone.\n\n");
        } else {
            out.push_str(&format!(
                "## ❌ Removed Symbols ({count})\n\n",
                count = removed.len()
            ));
            for change in &removed {
                if let crate::analyze::impact::SymbolChange::Removed { id, label, file } = change {
                    out.push_str(&format!("- **{label}** (`{id}`) — {file}\n"));
                }
            }
            out.push('\n');
        }

        // ── COMMUNITY MOVES ────────────────────────────────────────────────
        if moved.is_empty() {
            out.push_str("## 🔄 Community Moves\n\nNone.\n\n");
        } else {
            out.push_str(&format!(
                "## 🔄 Community Moves ({count})\n\n",
                count = moved.len()
            ));
            for change in &moved {
                if let crate::analyze::impact::SymbolChange::CommunityChanged {
                    id,
                    label,
                    old_community,
                    new_community,
                } = change
                {
                    let old_s =
                        old_community.map_or_else(|| "none".to_string(), |c| format!("{c}"));
                    let new_s =
                        new_community.map_or_else(|| "none".to_string(), |c| format!("{c}"));
                    out.push_str(&format!(
                        "- **{label}** (`{id}`): community {old_s} → {new_s}\n"
                    ));
                }
            }
            out.push('\n');
        }

        // ── ADDED (lowest risk) ────────────────────────────────────────────
        if added.is_empty() {
            out.push_str("## ✅ Added Symbols\n\nNone.\n\n");
        } else {
            out.push_str(&format!(
                "## ✅ Added Symbols ({count})\n\n",
                count = added.len()
            ));
            for change in &added {
                if let crate::analyze::impact::SymbolChange::Added {
                    id,
                    label,
                    file,
                    file_type,
                } = change
                {
                    out.push_str(&format!("- **{label}** (`{id}`) — {file_type} in {file}\n"));
                }
            }
            out.push('\n');
        }

        // ── Downstream impact ──────────────────────────────────────────────
        out.push_str("## 📊 Downstream Impact\n\n");
        out.push_str(&format!(
            "- Downstream nodes affected: {}\n",
            impact.downstream_nodes.len()
        ));
        out.push_str(&format!(
            "- Affected communities: {}\n",
            impact.affected_communities.len()
        ));
        if !impact.affected_communities.is_empty() {
            let comms: Vec<String> = impact
                .affected_communities
                .iter()
                .map(|c| format!("{c}"))
                .collect();
            out.push_str(&format!("- Community IDs: {}\n", comms.join(", ")));
        }
        out.push_str(&format!(
            "- Edge trust: {} extracted, {} inferred, {} ambiguous\n",
            impact.extracted_edges, impact.inferred_edges, impact.ambiguous_edges,
        ));

        out
    }

    // ── query_transitive ───────────────────────────────────────────────────────

    fn query_transitive(
        &self,
        seed: String,
        depth: Option<i32>,
        _relation: Option<String>,
        direction: Option<String>,
        budget: Option<i32>,
    ) -> String {
        let graph = self.graph();
        let seed_id = match self.resolve_id(&seed) {
            Some(id) => id,
            None => return format!("Node '{seed}' not found."),
        };
        let max_depth = depth.unwrap_or(3).clamp(1, 6) as usize;
        let dir = direction.as_deref().unwrap_or("forward");
        let budget_chars = (budget.unwrap_or(2000) as usize).max(200);

        let results = crate::serve::traversal::query_transitive_with_direction(
            &graph, &seed_id, dir, max_depth, None,
        );

        let depth_map: std::collections::HashMap<String, usize> = results.into_iter().collect();
        let total_nodes = depth_map.len();

        let mut out = format!(
            "# Transitive Closure from '{}'\n\n- Reachable nodes: {}\n- Max depth: {}\n\n",
            seed, total_nodes, max_depth
        );

        let mut displayed = 0usize;
        for depth in 0..=max_depth {
            let at_depth: Vec<&str> = depth_map
                .iter()
                .filter(|(_, d)| **d == depth)
                .map(|(id, _)| id.as_str())
                .collect();
            if at_depth.is_empty() {
                continue;
            }

            // Estimate cost of this whole depth block before adding it
            let depth_header = format!("## Depth {}\n", depth);
            let mut lines_cost = 0usize;
            let mut depth_entries = Vec::new();
            for id in &at_depth {
                if let Some(n) = graph.node_data(id) {
                    let entry = format!("  - {} (`{}`)\n", n.label, n.id);
                    lines_cost += entry.len();
                    depth_entries.push(entry);
                }
            }
            let block_cost = depth_header.len() + lines_cost + 1; // +1 for trailing '\n'

            if out.len() + block_cost > budget_chars {
                out.push_str(&format!(
                    "... showing {displayed} of {total_nodes} reachable nodes. Increase budget or reduce depth."
                ));
                break;
            }

            out.push_str(&depth_header);
            for entry in depth_entries {
                out.push_str(&entry);
            }
            out.push('\n');
            displayed += at_depth.len();
        }

        out
    }

    // ── references_to ─────────────────────────────────────────────────────────

    fn references_to(&self, symbol: String) -> String {
        let graph = self.graph();
        let resolved = match self.resolve_id(&symbol) {
            Some(id) => id,
            None => return format!("Symbol '{}' not found in the graph.", symbol),
        };
        let refs = crate::serve::traversal::find_structural_references(&graph, &resolved);
        if refs.is_empty() {
            return format!("No structural references found for '{}'.", symbol);
        }
        let mut out = String::new();
        out.push_str(&format!("# Structural References for '{}'\n\n", symbol));
        for (rel, _sf, ref_id) in &refs {
            let label = graph
                .node_data(ref_id)
                .map(|n| n.label.as_str())
                .unwrap_or(ref_id);
            out.push_str(&format!("- `{}` — `{}` ({})\n", label, rel, ref_id));
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

    /// Add or update a node in the relational index.
    fn add_node(
        &self,
        id: String,
        label: String,
        file_type: String,
        source_file: String,
        source_location: Option<String>,
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
        let total_nodes = self.graph().node_count();
        let total_edges = self.graph().edge_count();
        format!(
            "{action} node '{}' (id: {}, file_type: {}). Total: {} nodes, {} edges. {persist_msg}",
            node.display_label(),
            node.id,
            node.file_type,
            total_nodes,
            total_edges,
        )
    }

    /// Add an edge between two nodes in the relational index.
    fn add_edge(
        &self,
        source: String,
        target: String,
        relation: String,
        confidence: String,
        source_file: String,
        source_location: Option<String>,
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

        let total_nodes = self.graph().node_count();
        let total_edges = self.graph().edge_count();
        format!(
            "Added edge: {src} --[{relation}]--> {tgt} (confidence: {conf}). Total: {total_nodes} nodes, {total_edges} edges. {persist_msg}",
            src = src_id,
            tgt = tgt_id,
            relation = relation.to_lowercase(),
            conf = conf,
            total_nodes = total_nodes,
            total_edges = total_edges,
        )
    }

    /// Remove edges matching the given criteria from the relational index.
    fn remove_edge(&self, source: String, target: String, relation: Option<String>) -> String {
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
        let total_nodes = self.graph().node_count();
        let total_edges = self.graph().edge_count();
        format!(
            "Removed {count} edge(s) between '{src_id}' and '{tgt_id}'{rel_desc}. Total: {total_nodes} nodes, {total_edges} edges. {persist_msg}",
            src_id = src_id,
            tgt_id = tgt_id,
        )
    }

    fn recluster(&self) -> String {
        let mut graph = (*self.graph()).clone();
        let options = crate::cluster::ClusterOptions::default();
        let _community_stats = crate::cluster::cluster(&mut graph, &options);
        self.graph_store.store(Arc::new(graph));
        let persist_msg = self
            .persist_graph()
            .unwrap_or_else(|e| format!("(persist warning: {e})"));
        let community_count = self
            .graph()
            .nodes()
            .filter_map(|n| n.community)
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        format!("Reclustered into {community_count} communities. {persist_msg}")
    }

    // ── v3 MCP tools ────────────────────────────────────────────────────────

    fn resolution_report(&self) -> String {
        let graph = self.graph_store.load();
        let mut report = crate::trust::ResolutionReport::default();

        for edge in graph.edges_iter() {
            match edge.relation.as_str() {
                "imports" => {
                    report.total_import_edges += 1;
                    if edge.resolution_status.as_deref() == Some("resolved") {
                        report.resolved_imports += 1;
                    }
                    if edge.resolution_status.as_deref() == Some("unresolved") {
                        report.unresolved_refs += 1;
                    }
                }
                "calls" => {
                    report.total_call_edges += 1;
                    if edge.resolution_status.as_deref() == Some("resolved") {
                        report.resolved_calls += 1;
                    }
                }
                _ => {
                    if edge.relation != "contains" && edge.relation != "method" {
                        report.total_method_edges += 1;
                        if edge.resolution_status.as_deref() == Some("resolved") {
                            report.resolved_methods += 1;
                        }
                    }
                }
            }
            match edge.confidence {
                crate::model::Confidence::Extracted => {}
                crate::model::Confidence::Inferred => report.heuristic_edges += 1,
                crate::model::Confidence::Ambiguous => report.ambiguous_edges += 1,
            }
        }

        report.format()
    }

    fn ambiguous_symbols(&self) -> String {
        let graph = self.graph_store.load();

        // 1. Check ambiguous edges (existing behavior)
        let ambiguous_edges: Vec<_> = graph
            .edges_iter()
            .filter(|e| e.confidence == crate::model::Confidence::Ambiguous)
            .collect();

        // 2. Check label collisions: same label, different node IDs
        let mut label_counts: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        for n in graph.nodes() {
            label_counts
                .entry(n.label.clone())
                .or_default()
                .push(n.id.clone());
        }
        let colliding_labels: Vec<&String> = label_counts
            .iter()
            .filter(|(_, ids)| {
                // Exclude same-file collisions (overloads, partial classes — expected)
                if ids.len() <= 1 {
                    return false;
                }
                let mut seen_files = std::collections::HashSet::new();
                for id in ids.iter() {
                    if let Some(n) = graph.node_data(id) {
                        seen_files.insert(n.source_file.clone());
                    }
                }
                seen_files.len() > 1
            })
            .map(|(label, _)| label)
            .collect();

        let mut output = String::new();
        if ambiguous_edges.is_empty() && colliding_labels.is_empty() {
            return "No ambiguous symbols or label collisions found.\n                    Every node has a unique label, and every edge has EXTRACTED or INFERRED confidence."
                .to_string();
        }

        if !ambiguous_edges.is_empty() {
            output.push_str(&format!(
                "## Inferred/Ambiguous Edges ({})\n\n",
                ambiguous_edges.len()
            ));
            for e in &ambiguous_edges {
                let src_label = graph
                    .node_data(&e.source)
                    .map(|n| n.label.as_str())
                    .unwrap_or(e.source.as_str());
                let tgt_label = graph
                    .node_data(&e.target)
                    .map(|n| n.label.as_str())
                    .unwrap_or(e.target.as_str());
                let root = graph.metadata.project_root.as_deref();
                output.push_str(&format!(
                    "- {} `{}` {} [{}]\n",
                    src_label,
                    e.relation,
                    tgt_label,
                    crate::serve::traversal::relative_path(&e.source_file, root)
                ));
            }
            output.push('\n');
        }

        if !colliding_labels.is_empty() {
            output.push_str(&format!(
                "## Label Collisions ({})\n\n",
                colliding_labels.len()
            ));
            for label in colliding_labels.iter().take(20) {
                if let Some(ids) = label_counts.get(*label) {
                    // Skip same-file collisions (overloads, partial classes — expected)
                    let files: std::collections::BTreeSet<&str> = ids
                        .iter()
                        .filter_map(|id| graph.node_data(id).map(|n| n.source_file.as_str()))
                        .collect();
                    if files.len() <= 1 {
                        continue;
                    }
                    let entries: Vec<String> = ids
                        .iter()
                        .map(|id| {
                            let n = graph.node_data(id);
                            let sf = n.map(|n| n.source_file.as_str()).unwrap_or("?");
                            format!("`{}` ({})", id, sf)
                        })
                        .collect();
                    output.push_str(&format!(
                        "- **{}** — {} files, {} nodes: {}\n",
                        label,
                        files.len(),
                        ids.len(),
                        entries.join(", ")
                    ));
                }
            }
            if colliding_labels.len() > 20 {
                output.push_str(&format!(
                    "  ... and {} more colliding labels\n",
                    colliding_labels.len() - 20
                ));
            }
        }
        output
    }

    fn unresolved_references(&self) -> String {
        let graph = self.graph_store.load();
        let unresolved: Vec<_> = graph
            .edges_iter()
            .filter(|e| {
                e.relation == "imports" && e.resolution_status.as_deref() == Some("unresolved")
            })
            .collect();

        if unresolved.is_empty() {
            return "No unresolved references found.".to_string();
        }

        let mut output = format!("## Unresolved References ({})\n\n", unresolved.len());
        for e in &unresolved {
            let src_label = graph
                .node_data(&e.source)
                .map(|n| n.label.as_str())
                .unwrap_or(e.source.as_str());
            output.push_str(&format!(
                "- {} imports `{}` (not found)\n",
                src_label, e.target
            ));
        }
        output
    }

    fn safest_path(&self, from: String, to: String) -> String {
        let graph = self.graph_store.load();

        let from_id = self.resolve_id(&from);
        let to_id = self.resolve_id(&to);

        let (from, to) = match (from_id, to_id) {
            (Some(f), Some(t)) => (f, t),
            _ => {
                return format!(
                    "Could not resolve one or both node identifiers: '{}' and '{}'",
                    from, to
                );
            }
        };

        match crate::serve::traversal::safest_path_with_filters(&graph, &from, &to, None, &[], &[])
        {
            Some((path, safety)) => {
                let mut output = format!("## Safest Path (safety score: {:.2})\n\n", safety);
                for (i, node_id) in path.iter().enumerate() {
                    let label = graph
                        .node_data(node_id)
                        .map(|n| n.label.as_str())
                        .unwrap_or(node_id);
                    output.push_str(&format!("  {}. {}\n", i + 1, label));
                }
                output
            }
            None => format!("No path found between '{}' and '{}'.", from, to),
        }
    }

    fn verification_plan(&self, changed_nodes: String) -> String {
        let graph = self.graph_store.load();
        // Resolve labels/ids to real node ids; plan_verification matches against node ids,
        // so raw labels would never match (same bug that was in blast_radius).
        let (ids, _unresolved) = self.resolve_symbols_to_ids(&changed_nodes);

        let plan = crate::analyze::verifier::plan_verification(&graph, &ids, None);
        crate::analyze::verifier::format_plan(&plan)
    }

    fn blast_radius(&self, changed_nodes: String) -> String {
        let graph = self.graph_store.load();
        // Resolve each input (label or id) to a real node id; edge endpoints are ids, so
        // comparing the raw label would never match.
        let ids: Vec<String> = changed_nodes
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| self.resolve_id(s).unwrap_or_else(|| s.to_string()))
            .collect();

        let mut output = String::new();

        if ids.is_empty() {
            return "No changed nodes provided.".to_string();
        }

        // Once the Stack Graphs resolver has bound cross-file calls, the AST-only "calls 0%"
        // banner is no longer accurate — suppress it.
        let calls_resolved = graph
            .edges_iter()
            .any(|e| e.relation == "calls" && e.resolution_status.as_deref() == Some("resolved"));

        // Count direct affected neighbors
        let mut affected_files: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        let mut affected_communities: std::collections::BTreeSet<usize> =
            std::collections::BTreeSet::new();
        let mut extracted = 0usize;
        let mut inferred = 0usize;
        let mut ambiguous = 0usize;

        for id in &ids {
            for edge in graph.edges_iter() {
                if edge.source == *id || edge.target == *id {
                    let other = if edge.source == *id {
                        &edge.target
                    } else {
                        &edge.source
                    };
                    if let Some(node) = graph.node_data(other) {
                        affected_files.insert(node.source_file.clone());
                        if let Some(c) = node.community {
                            affected_communities.insert(c);
                        }
                    }
                    match edge.confidence {
                        crate::model::Confidence::Extracted => extracted += 1,
                        crate::model::Confidence::Inferred => inferred += 1,
                        crate::model::Confidence::Ambiguous => ambiguous += 1,
                    }
                }
            }
        }

        // 1. Warn only when cross-file calls are genuinely unresolved.
        if graph.is_ast_only() && !calls_resolved {
            output.push_str("⚠️  [SAFETY WARNING — AST-ONLY MODE ACTIVE]\n");
            output.push_str(
                "This graph was built in AST-only mode. Behavioral cross-file call resolution is 0%.\n\
                The blast radius shown below is strictly limited to static import/include directories \n\
                and DOES NOT reflect true runtime function call sites.\n\n\
                DO NOT rely on this result as a guarantee of change safety. To resolve call-graph coupling,\n\
                re-run Graphenium with a semantic pass: `gm run . --provider <provider>`.\n\n\
                ------------------------------------------------------------------------\n\n"
            );
        }

        output.push_str("## Blast Radius\n\n");
        if affected_files.is_empty() && graph.is_ast_only() && !calls_resolved {
            output.push_str(
                "Result: 0 affected symbols found.\n\
                (Note: This empty result is highly likely an artifact of AST-only mode rather than true decoupling.)\n\n"
            );
        }
        output.push_str(&format!("- Changed nodes: {}\n", ids.len()));
        output.push_str(&format!("- Affected files: {}\n", affected_files.len()));
        output.push_str(&format!(
            "- Affected communities: {}\n",
            affected_communities.len()
        ));
        output.push_str(&format!("- Extracted edges: {}\n", extracted));
        output.push_str(&format!("- Inferred edges: {}\n", inferred));
        output.push_str(&format!("- Ambiguous edges: {}\n", ambiguous));

        if !affected_files.is_empty() {
            output.push_str("\n### Affected Files\n");
            for f in &affected_files {
                output.push_str(&format!("- {}\n", f));
            }
        }

        output
    }

    // ── New MCP tools (agent_change_gate, diff_graph, next_files_to_read, review_plan) ──

    fn agent_change_gate(
        &self,
        changed_nodes: String,
        min_resolution: Option<f64>,
        max_ambiguous: Option<usize>,
        plan_id: Option<String>,
    ) -> String {
        let graph = self.graph_store.load();
        let ids: Vec<String> = changed_nodes
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        // Build a resolution report (same logic as resolution_report tool)
        let mut report = crate::trust::ResolutionReport::default();
        for edge in graph.edges_iter() {
            match edge.relation.as_str() {
                "imports" => {
                    report.total_import_edges += 1;
                    if edge.resolution_status.as_deref() == Some("resolved") {
                        report.resolved_imports += 1;
                    }
                    if edge.resolution_status.as_deref() == Some("unresolved") {
                        report.unresolved_refs += 1;
                    }
                }
                "calls" => {
                    report.total_call_edges += 1;
                    if edge.resolution_status.as_deref() == Some("resolved") {
                        report.resolved_calls += 1;
                    }
                }
                _ => {
                    if edge.relation != "contains" && edge.relation != "method" {
                        report.total_method_edges += 1;
                        if edge.resolution_status.as_deref() == Some("resolved") {
                            report.resolved_methods += 1;
                        }
                    }
                }
            }
            match edge.confidence {
                crate::model::Confidence::Extracted => {}
                crate::model::Confidence::Inferred => report.heuristic_edges += 1,
                crate::model::Confidence::Ambiguous => report.ambiguous_edges += 1,
            }
        }

        // Build policy list: start from defaults, override thresholds if provided
        let mut policies = crate::policy::default_policies();
        if let Some(res) = min_resolution {
            // Replace existing MinResolution or append
            let replaced = policies.iter_mut().any(|p| {
                if matches!(p, crate::policy::Policy::MinResolution(_)) {
                    *p = crate::policy::Policy::MinResolution(res);
                    true
                } else {
                    false
                }
            });
            if !replaced {
                policies.push(crate::policy::Policy::MinResolution(res));
            }
        }
        if let Some(amb) = max_ambiguous {
            let replaced = policies.iter_mut().any(|p| {
                if matches!(p, crate::policy::Policy::MaxAmbiguous(_)) {
                    *p = crate::policy::Policy::MaxAmbiguous(amb);
                    true
                } else {
                    false
                }
            });
            if !replaced {
                policies.push(crate::policy::Policy::MaxAmbiguous(amb));
            }
        }

        let results = crate::policy::evaluate_policies(&graph, &report, &policies);

        let import_pct = if report.total_import_edges > 0 {
            (report.resolved_imports as f64 / report.total_import_edges as f64) * 100.0
        } else {
            100.0
        };

        let mut output = String::new();
        output.push_str("## Agent Change Gate\n\n");
        output.push_str(&format!("- Changed nodes: {}\n", ids.len()));
        output.push_str(&format!("- Resolution score: {:.1}%\n", import_pct));
        output.push_str(&format!("- Ambiguous edges: {}\n", report.ambiguous_edges));
        output.push('\n');

        output.push_str("| Gate | Actual | Threshold | Status |\n");
        output.push_str("|------|--------|-----------|--------|\n");
        for r in &results {
            let gate_name = r.message.split(" — ").next().unwrap_or(&r.message);
            output.push_str(&format!(
                "| {} | {:.1} | {:.1} | {} |\n",
                gate_name,
                r.actual,
                r.threshold,
                if r.passed { "✅ PASS" } else { "❌ FAIL" },
            ));
        }

        if let Some(ref pid) = plan_id {
            output.push_str("\n## Pre-Flight Architecture Policy\n\n");
            let preflight = self.validate_plan_preflight(pid);
            if preflight.passes {
                output.push_str(&format!(
                    "Plan `{}`: ✅ PASS ({} rules checked)\n",
                    pid,
                    self.load_arch_policy().rules.len()
                ));
            } else {
                output.push_str(&format!("Plan `{}`: ❌ FAIL\n\n", pid));
                for v in &preflight.violations {
                    output.push_str(&format!("- {v}\n"));
                }
            }

            output.push_str("\n## Post-Edit Compliance\n\n");
            let verification = crate::harness::verify_plan(graph.as_ref(), pid);
            output.push_str(&format_plan_verification(&verification));
        }

        output
    }

    fn diff_graph(&self, before_graph: String, after_graph: String) -> String {
        use std::path::Path;

        let old = match crate::export::json::load_graph(Path::new(&before_graph)) {
            Ok(g) => g,
            Err(e) => {
                return format!(
                    "Failed to load 'before' graph from '{}': {}",
                    before_graph, e
                );
            }
        };
        let new = match crate::export::json::load_graph(Path::new(&after_graph)) {
            Ok(g) => g,
            Err(e) => {
                return format!("Failed to load 'after' graph from '{}': {}", after_graph, e);
            }
        };

        let d = crate::analyze::diff::diff(&old, &new);

        let mut output = String::new();
        output.push_str("## Graph Diff\n\n");
        output.push_str(&format!(
            "- Nodes added: {}  |  removed: {}\n",
            d.added_nodes.len(),
            d.removed_nodes.len(),
        ));
        output.push_str(&format!(
            "- Edges added: {}  |  removed: {}\n\n",
            d.added_edges.len(),
            d.removed_edges.len(),
        ));

        if !d.added_nodes.is_empty() {
            output.push_str("### Added Nodes\n");
            for n in &d.added_nodes {
                output.push_str(&format!("- {}\n", n));
            }
            output.push('\n');
        }

        if !d.removed_nodes.is_empty() {
            output.push_str("### Removed Nodes\n");
            for n in &d.removed_nodes {
                output.push_str(&format!("- {}\n", n));
            }
            output.push('\n');
        }

        if !d.added_edges.is_empty() {
            output.push_str("### Added Edges\n");
            for (s, t, r) in &d.added_edges {
                output.push_str(&format!("- {} {} {}\n", s, r, t));
            }
            output.push('\n');
        }

        if !d.removed_edges.is_empty() {
            output.push_str("### Removed Edges\n");
            for (s, t, r) in &d.removed_edges {
                output.push_str(&format!("- {} {} {}\n", s, r, t));
            }
            output.push('\n');
        }

        output
    }

    fn next_files_to_read(&self, changed_nodes: String) -> String {
        let graph = self.graph_store.load();

        // 1. Check if the graph is entirely empty
        if graph.node_count() == 0 {
            return "Error: The Graphenium graph is empty. Please run a codebase scan first using `gm run .` to generate the index.".to_string();
        }

        // 2. Resolve input symbols with fuzzy matching
        let (resolved_ids, unresolved) = self.resolve_symbols_to_ids(&changed_nodes);

        if !unresolved.is_empty() && resolved_ids.is_empty() {
            return format!(
                "Error: Could not resolve the symbol(s) {:?} to any nodes in the graph.\n\n\
                Suggestions:\n\
                - Check spelling and casing.\n\
                - Run `query_graph` to search for similar symbol names.\n\
                - Ensure the language extractor for this file type is active in `gm doctor`.",
                unresolved
            );
        }

        // 3. Generate verification plan
        let plan = crate::analyze::verifier::plan_verification(&graph, &resolved_ids, None);

        if plan.must_read.is_empty() {
            let mut msg = format!(
                "No files recommended to read for resolved symbols: {:?}.\n\n",
                resolved_ids
            );

            let calls_resolved = graph.edges_iter().any(|e| {
                e.relation == "calls" && e.resolution_status.as_deref() == Some("resolved")
            });
            if graph.is_ast_only() && !calls_resolved {
                msg.push_str(
                    "WARNING: AST-Only Mode Limitation —\n\
                    Because the graph is currently in AST-only mode, cross-file behavioral calls are 0% resolved.\n\
                    Graphenium cannot trace callers or downstream dependents to recommend files.\n\
                    To enable this, run a semantic scan: `gm run . --provider <provider>`.\n"
                );
            } else {
                msg.push_str(
                    "These symbols appear completely isolated. They have no incoming or outgoing connections \
                    registered in the graph.\n"
                );
            }
            return msg;
        }

        // 4. Post-process: deduplicate by file and filter namespace hubs
        let mut seen_files: std::collections::HashSet<String> = std::collections::HashSet::new();
        let filtered: Vec<&crate::analyze::verifier::VerificationStep> = plan
            .must_read
            .iter()
            .filter(|step| seen_files.insert(step.file.clone()))
            .filter(|step| {
                if let Some(node) = graph.node_data(&step.target) {
                    !crate::ranking::is_namespace_aggregation_node(node, &graph)
                } else {
                    true
                }
            })
            .collect();

        // 5. Return successful file recommendations
        let mut output = String::new();
        output.push_str(&format!(
            "## Files to Read ({} resolved symbol(s))\n\n",
            resolved_ids.len()
        ));
        for step in &filtered {
            output.push_str(&format!("- `{}` — {}\n", step.file, step.reason));
        }
        output
    }

    fn review_plan(
        &self,
        before_graph_path: Option<String>,
        after_graph_path: Option<String>,
    ) -> String {
        use std::path::Path;

        let before = match before_graph_path {
            Some(ref p) => match crate::export::json::load_graph(Path::new(p)) {
                Ok(g) => g,
                Err(e) => {
                    return format!("Failed to load 'before' graph from '{}': {}", p, e);
                }
            },
            None => crate::model::GrapheniumGraph::new(),
        };

        let after = match after_graph_path {
            Some(ref p) => match crate::export::json::load_graph(Path::new(p)) {
                Ok(g) => g,
                Err(e) => {
                    return format!("Failed to load 'after' graph from '{}': {}", p, e);
                }
            },
            None => {
                let arc = self.graph_store.load();
                crate::model::GrapheniumGraph::clone(&arc)
            }
        };

        let changes = crate::analyze::impact::symbol_inventory_diff(&before, &after);

        // Extract changed node IDs from the symbol inventory diff
        let changed_ids: Vec<String> = changes
            .iter()
            .map(|c| match c {
                crate::analyze::impact::SymbolChange::Added { id, .. }
                | crate::analyze::impact::SymbolChange::Removed { id, .. }
                | crate::analyze::impact::SymbolChange::CommunityChanged { id, .. } => id.clone(),
            })
            .collect();

        let plan = crate::analyze::verifier::plan_verification(&after, &changed_ids, None);

        let mut output = String::new();
        output.push_str("## Review Plan\n\n");
        output.push_str(&format!("- Changes detected: {}\n\n", changes.len()));

        if !changes.is_empty() {
            output.push_str("### Symbol Changes\n\n");
            output.push_str("| Change | ID | File |\n");
            output.push_str("|--------|----|------|\n");
            for change in &changes {
                match change {
                    crate::analyze::impact::SymbolChange::Added { id, file, .. } => {
                        output.push_str(&format!("| ➕ Added | `{}` | `{}` |\n", id, file));
                    }
                    crate::analyze::impact::SymbolChange::Removed { id, file, .. } => {
                        output.push_str(&format!("| ➖ Removed | `{}` | `{}` |\n", id, file));
                    }
                    crate::analyze::impact::SymbolChange::CommunityChanged {
                        id,
                        old_community,
                        new_community,
                        ..
                    } => {
                        output.push_str(&format!(
                            "| 🔄 Community | `{}` | {:?} → {:?} |\n",
                            id, old_community, new_community
                        ));
                    }
                }
            }
            output.push('\n');
        }

        output.push_str(&crate::analyze::verifier::format_plan(&plan));
        output
    }

    // ── Planning workspace tools (k) ────────────────────────────────────────

    fn render_plan_validation(&self, plan_id: String) -> String {
        let graph = self.graph();
        let project_root = graph
            .metadata
            .project_root
            .as_deref()
            .map(Path::new)
            .unwrap_or_else(|| Path::new("."));

        let report = match crate::harness::validate_plan(&graph, &plan_id, project_root) {
            Ok(report) => report,
            Err(e) => return format!("Plan `{plan_id}`: validation failed — {e}"),
        };

        if report.passes {
            let policy = self.load_arch_policy();
            if policy.rules.is_empty() {
                return format!(
                    "Plan `{}`: ✅ PASS (dynamic delta gating — no explicit policy rules configured)",
                    plan_id
                );
            }
            return format!(
                "Plan `{}`: ✅ PASS ({} policy rules + dynamic delta gating)",
                plan_id,
                policy.rules.len()
            );
        }

        let mut output = format!("Plan `{}`: ❌ PRE_FLIGHT_VIOLATION\n\n", plan_id);
        for v in &report.violations {
            output.push_str(&format!("- {v}\n"));
        }
        output
    }

    fn evaluate_delta_gate(
        &self,
        plan_id: String,
        modularity_tolerance: Option<f64>,
        surprise_threshold: Option<f64>,
    ) -> String {
        let graph = self.graph();
        let mod_tol = modularity_tolerance.unwrap_or(-0.02);
        let sur_thresh = surprise_threshold.unwrap_or(5.0);

        let report =
            match crate::analyze::delta::evaluate_delta_gate(&graph, &plan_id, mod_tol, sur_thresh)
            {
                Ok(report) => report,
                Err(e) => return format!("Delta evaluation failed: {e}"),
            };

        let mut response_md = format!(
            "### Topological Delta Report for Plan: `{}`\n\n\
             - **Status:** {}\n\
             - **Modularity Delta (ΔQ):** {:.4} (Baseline: {:.4} → Virtual: {:.4})\n",
            report.plan_id,
            if report.passes {
                "✅ PASSED"
            } else {
                "❌ FAILED"
            },
            report.modularity_delta,
            report.modularity_baseline,
            report.modularity_virtual
        );

        if !report.plan_surprise_edges.is_empty() {
            response_md.push_str("\n#### High-Surprise Edges Proposed:\n");
            for e in &report.plan_surprise_edges {
                response_md.push_str(&format!(
                    "- `{}` ──► `{}` (Confidence: {:.1})\n  *Reason:* {}\n",
                    e.source,
                    e.target,
                    e.score,
                    e.reasons.join(", ")
                ));
            }
        }

        if !report.drift_events.is_empty() {
            response_md.push_str("\n#### Structural Drift Warnings:\n");
            for d in &report.drift_events {
                response_md.push_str(&format!("- **{}**: {}\n", d.label, d.detail));
            }
        }

        response_md
    }

    fn get_plan_details(&self, plan_id: String) -> String {
        let graph = self.graph();
        let report = crate::harness::verify_plan(graph.as_ref(), &plan_id);
        let planned_ids: Vec<String> = graph
            .nodes()
            .filter(|n| n.plan_id.as_deref() == Some(&plan_id))
            .map(|n| n.id.clone())
            .collect();

        if planned_ids.is_empty() {
            return format!(
                "No planned nodes found for plan ID '{}'.\n\n## Plan Verification\n\n{}",
                plan_id,
                format_plan_verification(&report)
            );
        }

        let mut output = crate::serve::traversal::subgraph_to_text_with_match_details(
            &graph,
            &planned_ids,
            4000,
            &[],
            &[],
            &[],
        );
        output.push_str("\n\n## Plan Verification\n\n");
        output.push_str(&format_plan_verification(&report));
        output
    }

    // ── Meta tools ──────────────────────────────────────────────────────────

    fn graph_info(&self) -> String {
        let graph = self.graph();
        let meta = &graph.metadata;
        let mut out = String::new();
        out.push_str("# Graph Info\n\n");
        if let Some(ref v) = meta.schema_version {
            out.push_str(&format!("**Schema:** {v}\n"));
        }
        if let Some(ref v) = meta.project_root {
            let clean = crate::serve::traversal::clean_windows_path(v);
            out.push_str(&format!("**Project root:** {clean}\n"));
        }
        if let Some(ref v) = meta.created_at {
            out.push_str(&format!("**Built at:** {v}\n"));
        }
        if let Some(ref modes) = meta.extraction_modes {
            out.push_str(&format!("**Extraction:** {}\n", modes.join(", ")));
        }
        if let Some(ref langs) = meta.languages {
            out.push_str(&format!("**Languages:** {}\n", langs.join(", ")));
        }
        out.push_str(&format!("**Nodes:** {}\n", graph.node_count()));
        out.push_str(&format!("**Edges:** {}\n", graph.edge_count()));
        out.push_str(&format!("**Hyperedges:** {}\n", graph.hyperedges.len()));
        let communities = graph
            .nodes()
            .filter_map(|n| n.community)
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        out.push_str(&format!("**Communities:** {communities}\n"));
        if let Some(path) = self.default_graph_path() {
            out.push_str(&format!("**Graph path:** {}\n", path.display()));
        }
        if let Some(warning) = self.staleness_warning() {
            out.push('\n');
            out.push_str(&warning);
        }

        let policy = self.load_arch_policy();
        if policy.rules.is_empty() {
            out.push_str(
                "\n**Policy Gates Active:** Dynamic Delta Gating (Zero-Config Modularity Protection)\n",
            );
        } else {
            out.push_str(&format!(
                "\n**Policy Gates Active:** {} explicit policy rule(s) + Dynamic Delta Gating\n",
                policy.rules.len()
            ));
        }

        // Lean 4 trust-boundary summary (semantic `#print axioms` ingestion).
        let mut sorry_gaps = 0usize;
        let mut native_decide = 0usize;
        let mut external_axioms = 0usize;
        for edge in graph.edges_iter() {
            match edge.relation.as_str() {
                "unverified_sorry" => sorry_gaps += 1,
                "unverified_native_decide" => native_decide += 1,
                "depends_on_axiom" => {
                    let axiom = edge.tgt_original.to_ascii_lowercase();
                    if !matches!(
                        axiom.as_str(),
                        "propext" | "classical.choice" | "quot.sound"
                    ) && !matches!(
                        edge.target.as_str(),
                        "propext" | "classical_choice" | "quot_sound"
                    ) {
                        external_axioms += 1;
                    }
                }
                _ => {}
            }
        }
        if sorry_gaps + native_decide + external_axioms > 0 {
            out.push_str(&format!(
                "\n**Lean trust:** {sorry_gaps} sorry gap(s) · {native_decide} native_decide shortcut(s) · {external_axioms} external axiom edge(s)\n"
            ));
        }

        // Trust banner: mode and resolution status (reflect actual cross-file resolution).
        let is_ast = graph.is_ast_only();
        let (mut calls_total, mut calls_res) = (0usize, 0usize);
        for e in graph.edges_iter() {
            if e.relation == "calls" {
                calls_total += 1;
                if e.resolution_status.as_deref() == Some("resolved") {
                    calls_res += 1;
                }
            }
        }
        let calls_pct = if calls_total > 0 {
            calls_res
                .checked_mul(100)
                .and_then(|n| n.checked_div(calls_total))
                .unwrap_or(0)
        } else {
            0
        };
        if is_ast && calls_res == 0 {
            out.push_str("\n**Graph mode:** AST-only · imports 100% resolved · calls 0% resolved (requires semantic pass) · 0 ambiguous\n");
            out.push_str("Behavioral edges (calls/uses) are heuristic hints; structural edges (imports/contains) are ground truth.\n");
        } else if is_ast {
            out.push_str(&format!(
                "\n**Graph mode:** AST + Stack Graphs · imports 100% resolved · calls {calls_pct}% resolved cross-file (INFERRED) · 0 ambiguous\n"
            ));
            out.push_str("Structural edges are ground truth; resolved cross-file calls are high-probability hints (INFERRED) — corroborate before acting.\n");
        } else {
            out.push_str("\n**Graph mode:** Semantic (LLM-enriched) · calls resolved via AI\n");
        }

        out
    }
}

// ── Tool definitions ──────────────────────────────────────────────────────────

#[tool(tool_box)]
impl GrapheniumServer {
    /// Consolidated graph health and orientation tool.
    ///
    /// Parameters:
    /// - `include_stats`: append scoped statistics from the graph_stats backend.
    /// - `include_unresolved`: append the unresolved import reference report.
    /// - `include_resolution`: append the resolution-quality report.
    /// - `include_ambiguous`: append the ambiguous edge and label collision report.
    /// - `path_prefix`, `exclude_path`, `node_types`, `generated_code_mode`,
    ///   `ast_only_tuning`: optional scope filters forwarded to the statistics backend.
    #[tool(
        description = "Return graph metadata, health, and diagnostics in one call. \
        Always returns project root, schema version, build time, extraction modes, languages, \
        node/edge/hyperedge/community counts, active policy gates, and the graph trust mode. \
        Set include_stats to append scoped statistics, include_unresolved to list unresolved \
        imports, include_resolution to append the resolution-quality report, and \
        include_ambiguous to list ambiguous edges and label collisions."
    )]
    fn get_graph_info(
        &self,
        #[tool(param)]
        #[schemars(
            description = "Append scoped statistics: node and edge counts, node-type breakdown, edge confidence distribution, and extractor provenance. Default false."
        )]
        include_stats: Option<bool>,
        #[tool(param)]
        #[schemars(
            description = "Append the list of unresolved import references (import edges whose target symbol is missing from the graph). Default false."
        )]
        include_unresolved: Option<bool>,
        #[tool(param)]
        #[schemars(
            description = "Append the resolution-quality report covering import, call, and method resolution rates plus ambiguous edge counts. Default false."
        )]
        include_resolution: Option<bool>,
        #[tool(param)]
        #[schemars(
            description = "Append the ambiguous symbol report listing low-confidence edges and cross-file label collisions. Default false."
        )]
        include_ambiguous: Option<bool>,
        #[tool(param)]
        #[schemars(
            description = "Optional path prefix or path fragment to include in statistics (case-insensitive)."
        )]
        path_prefix: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Optional path prefix or path fragment to exclude from statistics (case-insensitive)."
        )]
        exclude_path: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Optional file types to include in statistics, for example ['code', 'document', 'rationale']."
        )]
        node_types: Option<Vec<String>>,
        #[tool(param)]
        #[schemars(
            description = "Generated-like code filter for statistics: 'include' (default), 'exclude', or 'only'."
        )]
        generated_code_mode: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "AST-only tuning for statistics: true enables AST-only noise suppression, false disables it, omitted = auto."
        )]
        ast_only_tuning: Option<bool>,
    ) -> String {
        let mut sections = vec![self.graph_info()];

        if include_stats.unwrap_or(false) {
            sections.push(self.graph_stats(
                path_prefix,
                exclude_path,
                node_types,
                generated_code_mode,
                ast_only_tuning,
            ));
        }
        if include_unresolved.unwrap_or(false) {
            sections.push(self.unresolved_references());
        }
        if include_resolution.unwrap_or(false) {
            sections.push(self.resolution_report());
        }
        if include_ambiguous.unwrap_or(false) {
            sections.push(self.ambiguous_symbols());
        }

        sections.join("\n\n---\n\n")
    }

    /// Consolidated architecture orientation tool.
    ///
    /// Parameters:
    /// - `community_id`: focus on a single community instead of the repository overview.
    /// - `include_community_members`: append the full member list for the focused community.
    /// - `include_god_nodes`: append the top hub nodes.
    /// - `god_node_limit`: number of hub nodes to return.
    /// - `path_prefix`, `exclude_path`, `node_types`, `generated_code_mode`,
    ///   `ast_only_tuning`: optional scope filters.
    /// - `max_communities`: maximum number of communities in the overview.
    #[tool(
        description = "Return a repository-level architecture summary with communities, \
        cross-community connectors, and architectural hotspots. Set community_id to focus on \
        one community instead of the overview, include_community_members to append its full \
        member list, and include_god_nodes to append the most highly connected hub nodes."
    )]
    fn get_architecture_summary(
        &self,
        #[tool(param)]
        #[schemars(
            description = "Optional integer community ID. When set, the response focuses on that community instead of the repository-wide overview."
        )]
        community_id: Option<i32>,
        #[tool(param)]
        #[schemars(
            description = "When community_id is set, append the full member list for that community. Default false."
        )]
        include_community_members: Option<bool>,
        #[tool(param)]
        #[schemars(
            description = "Append the top architectural hub nodes (god nodes) after the architecture summary. Default false."
        )]
        include_god_nodes: Option<bool>,
        #[tool(param)]
        #[schemars(
            description = "Number of hub nodes to return when include_god_nodes is true (default 10, max 50)."
        )]
        god_node_limit: Option<i32>,
        #[tool(param)]
        #[schemars(
            description = "Optional path prefix or path fragment to include (case-insensitive)."
        )]
        path_prefix: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Optional path prefix or path fragment to exclude (case-insensitive)."
        )]
        exclude_path: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Optional file types to include, for example ['code', 'document', 'rationale']."
        )]
        node_types: Option<Vec<String>>,
        #[tool(param)]
        #[schemars(
            description = "Generated-like code filter: 'include' (default), 'exclude', or 'only'."
        )]
        generated_code_mode: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "AST-only tuning: true enables AST-only noise suppression, false disables it, omitted = auto."
        )]
        ast_only_tuning: Option<bool>,
        #[tool(param)]
        #[schemars(
            description = "Maximum number of communities to summarize in the overview (default 5, max 10)."
        )]
        max_communities: Option<i32>,
    ) -> String {
        let god_nodes_section = if include_god_nodes.unwrap_or(false) {
            Some(self.god_nodes(
                god_node_limit,
                path_prefix.clone(),
                exclude_path.clone(),
                node_types.clone(),
                generated_code_mode.clone(),
                ast_only_tuning,
            ))
        } else {
            None
        };

        let overview = match community_id {
            Some(id) => self.get_community(id, include_community_members),
            None => self.architecture_summary(
                path_prefix,
                exclude_path,
                node_types,
                generated_code_mode,
                ast_only_tuning,
                max_communities,
            ),
        };

        match god_nodes_section {
            Some(hubs) => format!("{overview}\n\n---\n\n{hubs}"),
            None => overview,
        }
    }

    /// Inspect a symbol and retrieve its structural neighborhood and metrics.
    ///
    /// Use this read-only tool as your primary orientation step before modifying a specific class,
    /// function, or module. Use this instead of `query_graph` when you already know the exact
    /// `target` ID and need its direct callers, neighbors, or a verification plan.
    ///
    /// The baseline response returns canonical node details. Use the boolean flags to append
    /// optional data blocks (for example, `include_callers: true` appends all incoming
    /// dependencies). Note: setting multiple flags to `true` (such as `include_analysis` and
    /// `include_file_summary`) can produce large responses; use only the flags needed for your
    /// immediate task to conserve context budget.
    ///
    /// Parameters:
    /// - `target`: symbol ID or label, or a file path when `include_file_summary` is set.
    /// - `include_neighbors`: append direct neighbors with edge details.
    /// - `include_callers`: append structural references (containers, imports, inheritance, implementations).
    /// - `include_analysis`: append the composite behavioral and structural analysis.
    /// - `include_file_summary`: append all symbols extracted from the target's source file.
    /// - `include_explanation`: append hierarchy, entry points, must-read files, and test scaffolding.
    /// - `relation`, `max_neighbors`, `extracted_only`: neighbor filters.
    /// - `group_by`, `show_leaves`: file summary filters.
    #[tool(
        description = "Inspect a symbol and retrieve its structural neighborhood and metrics. \
        Use this read-only tool as your primary orientation step before modifying a specific class, \
        function, or module. Use this instead of query_graph when you already know the exact target \
        ID and need its direct callers, neighbors, or a verification plan. The baseline response \
        returns canonical node details. Use the boolean flags to append optional data blocks (for \
        example, include_callers=true appends all incoming dependencies). Setting multiple flags to \
        true (such as include_analysis and include_file_summary) can produce large responses; use \
        only the flags needed for your immediate task to conserve context budget."
    )]
    fn inspect_symbol(
        &self,
        #[tool(param)]
        #[schemars(
            description = "Node ID or label to inspect. When include_file_summary is set, a file path or filename suffix is also accepted."
        )]
        target: String,
        #[tool(param)]
        #[schemars(
            description = "Append direct neighbors of the target with relation types, confidence, and scores. Default false."
        )]
        include_neighbors: Option<bool>,
        #[tool(param)]
        #[schemars(
            description = "Append structural references to the target: containers, imports, inheritance, and implementations. Default false."
        )]
        include_callers: Option<bool>,
        #[tool(param)]
        #[schemars(
            description = "Append the composite symbol analysis with behavioral and structural connections plus a confidence summary. Default false."
        )]
        include_analysis: Option<bool>,
        #[tool(param)]
        #[schemars(
            description = "Append a summary of every symbol extracted from the target's source file. Default false."
        )]
        include_file_summary: Option<bool>,
        #[tool(param)]
        #[schemars(
            description = "Append a pre-change explanation with hierarchy, community context, entry points, must-read files, and test scaffolding. Default false."
        )]
        include_explanation: Option<bool>,
        #[tool(param)]
        #[schemars(
            description = "Optional substring used to filter neighbors by relation type, for example 'calls' or 'imports'."
        )]
        relation: Option<String>,
        #[tool(param)]
        #[schemars(description = "Maximum number of neighbors to return (default 50, minimum 1).")]
        max_neighbors: Option<i32>,
        #[tool(param)]
        #[schemars(
            description = "When include_neighbors is true, show only edges with EXTRACTED confidence. Default false."
        )]
        extracted_only: Option<bool>,
        #[tool(param)]
        #[schemars(
            description = "When include_file_summary is true, group symbols by 'kind' (default) or 'community'."
        )]
        group_by: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "When include_file_summary is true, include low-degree leaf symbols of degree 5 or less. Default false."
        )]
        show_leaves: Option<bool>,
    ) -> String {
        let mut sections = vec![self.get_node(target.clone())];

        if include_neighbors.unwrap_or(false) {
            sections.push(self.get_neighbors(
                target.clone(),
                relation,
                max_neighbors,
                extracted_only,
            ));
        }
        if include_callers.unwrap_or(false) {
            sections.push(self.references_to(target.clone()));
        }
        if include_analysis.unwrap_or(false) {
            sections.push(self.analyse_symbol(target.clone()));
        }
        if include_explanation.unwrap_or(false) {
            sections.push(self.explain_change(target.clone()));
        }
        if include_file_summary.unwrap_or(false) {
            let file_target = self
                .resolve_id(&target)
                .and_then(|id| {
                    self.graph()
                        .node_data(&id)
                        .map(|node| node.source_file.clone())
                })
                .unwrap_or_else(|| target.clone());
            sections.push(self.summarize_file(file_target, group_by, None, show_leaves));
        }

        sections.join("\n\n---\n\n")
    }

    /// Trace connectivity and multi-hop paths across the codebase architecture.
    ///
    /// Use this read-only tool to map blast radius, find shortest paths, or verify transitive
    /// layering. Use this instead of `query_graph` or `inspect_symbol` when you need to prove deep,
    /// multi-hop relationships between specific components.
    ///
    /// The tool operates in five distinct modes: `shortest` (finds semantic or strict AST paths
    /// between source and target), `safest` (finds the path with highest aggregate confidence),
    /// `transitive_forward` (downstream reachability), `transitive_reverse` (upstream callers), and
    /// `module_dependencies` (cross-module edges). Returns a structured Markdown report detailing
    /// the traced paths, confidence scores, and edge relations. Traversal halts when `depth` is
    /// reached, and output is truncated to fit the `budget`.
    ///
    /// Parameters:
    /// - `source`: starting node ID or label, or the source module fragment for module_dependencies.
    /// - `target`: destination node ID or label, or the target module fragment for module_dependencies.
    /// - `mode`: routing mode: shortest, safest, transitive_forward, transitive_reverse, or module_dependencies.
    /// - `depth`: maximum traversal depth for transitive modes.
    /// - `direction`: optional direction override for transitive modes.
    /// - `budget`: approximate output character budget for transitive modes.
    /// - `path_prefix`, `exclude_path`, `node_types`: optional scope filters for shortest paths.
    /// - `include_relations`, `exclude_relations`: optional relation filters for shortest paths.
    /// - `path_mode`: shortest path strategy: semantic or strict.
    /// - `exclude_framework_noise`: exclude framework and import-noise bridge nodes.
    /// - `generated_code_mode`, `ast_only_tuning`: optional generated-code and AST tuning filters.
    #[tool(
        description = "Trace connectivity and multi-hop paths across the codebase architecture. \
        Use this read-only tool to map blast radius, find shortest paths, or verify transitive \
        layering. Use this instead of query_graph or inspect_symbol when you need to prove deep, \
        multi-hop relationships between specific components. The tool operates in five distinct \
        modes: shortest (finds semantic or strict AST paths between source and target), safest \
        (finds the path with highest aggregate confidence), transitive_forward (downstream \
        reachability), transitive_reverse (upstream callers), and module_dependencies (cross-module \
        edges). Returns a structured Markdown report detailing the traced paths, confidence scores, \
        and edge relations. Traversal halts when depth is reached, and output is truncated to fit \
        the budget."
    )]
    fn trace_path(
        &self,
        #[tool(param)]
        #[schemars(
            description = "Starting node ID or label. For module_dependencies, this is the source module or directory path fragment."
        )]
        source: String,
        #[tool(param)]
        #[schemars(
            description = "Destination node ID or label. Required for shortest, safest, and module_dependencies modes. For module_dependencies, this is the target module or directory path fragment."
        )]
        target: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Routing mode: 'shortest' (default), 'safest', 'transitive_forward', 'transitive_reverse', or 'module_dependencies'."
        )]
        mode: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Maximum traversal depth for transitive modes (default 3, max 6)."
        )]
        depth: Option<i32>,
        #[tool(param)]
        #[schemars(
            description = "Optional direction override for transitive modes: 'forward', 'reverse', or 'both'."
        )]
        direction: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Approximate output character budget for transitive modes (default 2000)."
        )]
        budget: Option<i32>,
        #[tool(param)]
        #[schemars(
            description = "Optional path prefix or path fragment to include for shortest paths (case-insensitive)."
        )]
        path_prefix: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Optional path prefix or path fragment to exclude for shortest paths (case-insensitive)."
        )]
        exclude_path: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Optional file types to include for shortest paths, for example ['code', 'document', 'rationale']."
        )]
        node_types: Option<Vec<String>>,
        #[tool(param)]
        #[schemars(
            description = "Optional relation filters to include for shortest paths, for example ['calls', 'uses']."
        )]
        include_relations: Option<Vec<String>>,
        #[tool(param)]
        #[schemars(
            description = "Optional relation filters to exclude for shortest paths, for example ['imports']."
        )]
        exclude_relations: Option<Vec<String>>,
        #[tool(param)]
        #[schemars(
            description = "Shortest path strategy: 'semantic' (default) prefers calls, uses, and contains style relations; 'strict' uses the exact fewest hops."
        )]
        path_mode: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Exclude framework and import-noise bridge nodes from shortest path search. Default false."
        )]
        exclude_framework_noise: Option<bool>,
        #[tool(param)]
        #[schemars(
            description = "Generated-like code filter for shortest paths: 'include' (default), 'exclude', or 'only'."
        )]
        generated_code_mode: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "AST-only tuning for shortest paths: true enables AST-only noise suppression, false disables it, omitted = auto."
        )]
        ast_only_tuning: Option<bool>,
    ) -> String {
        let mode = mode.unwrap_or_else(|| "shortest".to_string());
        match mode.trim().to_lowercase().as_str() {
            "shortest" => {
                let Some(destination) = target else {
                    return "trace_path mode 'shortest' requires a target node ID or label."
                        .to_string();
                };
                self.shortest_path(
                    source,
                    destination,
                    path_prefix,
                    exclude_path,
                    node_types,
                    include_relations,
                    exclude_relations,
                    path_mode,
                    exclude_framework_noise,
                    generated_code_mode,
                    ast_only_tuning,
                )
            }
            "safest" => {
                let Some(destination) = target else {
                    return "trace_path mode 'safest' requires a target node ID or label."
                        .to_string();
                };
                self.safest_path(source, destination)
            }
            "transitive_forward" => self.query_transitive(
                source,
                depth,
                None,
                Some(direction.unwrap_or_else(|| "forward".to_string())),
                budget,
            ),
            "transitive_reverse" => self.query_transitive(
                source,
                depth,
                None,
                Some(direction.unwrap_or_else(|| "reverse".to_string())),
                budget,
            ),
            "module_dependencies" => {
                let Some(destination) = target else {
                    return "trace_path mode 'module_dependencies' requires a target module or directory path fragment."
                        .to_string();
                };
                self.module_dependencies(source, destination)
            }
            other => format!(
                "Unknown trace mode '{other}'. Expected one of: shortest, safest, transitive_forward, transitive_reverse, module_dependencies."
            ),
        }
    }

    /// Consolidated plan validation tool.
    ///
    /// Parameters:
    /// - `plan_id`: planning workspace identifier.
    /// - `skip_policy_rules`: run only the Louvain delta gate and skip explicit policy rules.
    /// - `modularity_tolerance`: maximum allowed modularity drop for the delta gate.
    /// - `surprise_threshold`: surprise score threshold for the delta gate.
    #[tool(
        description = "Validate a planning workspace before writing code. Always runs the Louvain \
        modularity delta gate and surprise analysis. Unless skip_policy_rules is true, it also \
        applies explicit .graphenium/policy.json rules when present."
    )]
    fn validate_plan(
        &self,
        #[tool(param)]
        #[schemars(description = "The workspace plan identifier to validate.")]
        plan_id: String,
        #[tool(param)]
        #[schemars(
            description = "When true, skip explicit architecture policy rules and run only the Louvain modularity delta gate. Default false."
        )]
        skip_policy_rules: Option<bool>,
        #[tool(param)]
        #[schemars(
            description = "Maximum allowed modularity drop for the delta gate (default -0.02)."
        )]
        modularity_tolerance: Option<f64>,
        #[tool(param)]
        #[schemars(
            description = "Surprise threshold used to flag structurally surprising planned edges (default 5.0)."
        )]
        surprise_threshold: Option<f64>,
    ) -> String {
        let delta =
            self.evaluate_delta_gate(plan_id.clone(), modularity_tolerance, surprise_threshold);

        if skip_policy_rules.unwrap_or(false) {
            return delta;
        }

        let policy = self.render_plan_validation(plan_id);
        format!("{policy}\n\n---\n\n{delta}")
    }

    /// Consolidated change verification tool.
    ///
    /// Parameters:
    /// - `plan_id`: optional planning workspace to audit for scope creep.
    /// - `target_symbols`: optional list of changed node IDs or labels.
    /// - `min_resolution`: optional minimum import resolution percentage for the gate.
    /// - `max_ambiguous`: optional maximum allowed ambiguous edges for the gate.
    /// - `snapshot_name`: optional snapshot name for a what-changed report.
    /// - `before_graph_path`, `after_graph_path`: optional graph files for a diff and review plan.
    #[tool(
        description = "Verify a change in one unified report. Returns the scope creep audit, \
        blast radius, verification checklist, and must-read files for the supplied target symbols. \
        When plan_id is set it also returns the plan details and post-edit compliance audit. \
        Optional snapshot or graph path inputs add what-changed and graph-diff sections."
    )]
    fn verify_changes(
        &self,
        #[tool(param)]
        #[schemars(
            description = "Optional planning workspace ID. When set, includes pre-flight policy checks, post-edit compliance, and the full planned subgraph."
        )]
        plan_id: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Optional list of changed node IDs or labels to audit. Each entry may be an exact ID or a case-insensitive label."
        )]
        target_symbols: Option<Vec<String>>,
        #[tool(param)]
        #[schemars(
            description = "Optional override for the minimum import resolution percentage used by the change gate (default 80.0)."
        )]
        min_resolution: Option<f64>,
        #[tool(param)]
        #[schemars(
            description = "Optional override for the maximum allowed ambiguous edges used by the change gate (default 10)."
        )]
        max_ambiguous: Option<usize>,
        #[tool(param)]
        #[schemars(
            description = "Optional snapshot name. When set, appends a risk-sorted what-changed report comparing the snapshot to the loaded graph."
        )]
        snapshot_name: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Optional filesystem path to a before graph.json file. Used with after_graph_path for a graph diff and review plan."
        )]
        before_graph_path: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Optional filesystem path to an after graph.json file. Used with before_graph_path for a graph diff and review plan."
        )]
        after_graph_path: Option<String>,
    ) -> String {
        let changed = target_symbols
            .as_ref()
            .map(|symbols| symbols.join(","))
            .unwrap_or_default();
        let has_changed = !changed.trim().is_empty();

        let mut sections = vec![self.agent_change_gate(
            changed.clone(),
            min_resolution,
            max_ambiguous,
            plan_id.clone(),
        )];

        if has_changed {
            sections.push(self.blast_radius(changed.clone()));
            sections.push(self.verification_plan(changed.clone()));
            sections.push(self.next_files_to_read(changed.clone()));
        }

        if let Some(pid) = plan_id {
            sections.push(self.get_plan_details(pid));
        }

        if let Some(name) = snapshot_name {
            sections.push(self.what_changed(Some(name), None));
        }

        match (before_graph_path, after_graph_path) {
            (Some(before), Some(after)) => {
                sections.push(self.diff_graph(before.clone(), after.clone()));
                sections.push(self.review_plan(Some(before), Some(after)));
            }
            (Some(before), None) => {
                sections.push(self.review_plan(Some(before), None));
            }
            (None, Some(after)) => {
                sections.push(self.review_plan(None, Some(after)));
            }
            (None, None) => {}
        }

        sections.join("\n\n---\n\n")
    }

    /// Consolidated manual graph mutation tool.
    ///
    /// Parameters:
    /// - `action`: one of add_node, add_edge, remove_edge, or recluster.
    /// - `id`, `label`, `file_type`, `source_file`, `source_location`, `qualified_label`: add_node payload.
    /// - `source`, `target`, `relation`, `confidence`, `source_file`, `source_location`, `weight`: add_edge payload.
    /// - `source`, `target`, `relation`: remove_edge payload.
    #[tool(
        description = "Apply a manual mutation to the relational index in one call. Use \
        action='add_node' to register a concept or rationale node, action='add_edge' to record a \
        verified relationship, action='remove_edge' to delete a stale or false-positive edge, or \
        action='recluster' to re-run community detection. Successful mutations persist to disk immediately."
    )]
    fn mutate_graph(
        &self,
        #[tool(param)]
        #[schemars(
            description = "Mutation action: 'add_node', 'add_edge', 'remove_edge', or 'recluster'."
        )]
        action: String,
        #[tool(param)]
        #[schemars(
            description = "Node identifier for add_node. Normalized automatically into a stable ID."
        )]
        id: Option<String>,
        #[tool(param)]
        #[schemars(description = "Human-readable display name for add_node.")]
        label: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Node type for add_node: 'code', 'document', 'paper', 'image', or 'rationale'."
        )]
        file_type: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Relative source file path for add_node, or the file where an add_edge relationship was observed."
        )]
        source_file: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Optional source location hint, for example 'L42', used by add_node or add_edge."
        )]
        source_location: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Optional qualified scope-prefixed label used by add_node for disambiguation."
        )]
        qualified_label: Option<String>,
        #[tool(param)]
        #[schemars(description = "Source node ID or label for add_edge and remove_edge.")]
        source: Option<String>,
        #[tool(param)]
        #[schemars(description = "Target node ID or label for add_edge and remove_edge.")]
        target: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Relation type for add_edge, for example 'calls', 'uses', 'delegates_to', or 'rationale_for'. For remove_edge, an optional relation filter."
        )]
        relation: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Edge confidence for add_edge: 'EXTRACTED' (verified by inspection), 'INFERRED' (strong hint), or 'AMBIGUOUS' (uncertain)."
        )]
        confidence: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Optional traversal weight override for add_edge, clamped to the 0.0 to 1.0 range."
        )]
        weight: Option<f64>,
    ) -> String {
        match action.trim().to_lowercase().as_str() {
            "add_node" => {
                let (Some(id), Some(label), Some(file_type), Some(source_file)) =
                    (id, label, file_type, source_file)
                else {
                    return "mutate_graph action 'add_node' requires id, label, file_type, and source_file."
                        .to_string();
                };
                self.add_node(
                    id,
                    label,
                    file_type,
                    source_file,
                    source_location,
                    qualified_label,
                )
            }
            "add_edge" => {
                let (Some(source), Some(target), Some(relation), Some(confidence), Some(source_file)) =
                    (source, target, relation, confidence, source_file)
                else {
                    return "mutate_graph action 'add_edge' requires source, target, relation, confidence, and source_file."
                        .to_string();
                };
                self.add_edge(
                    source,
                    target,
                    relation,
                    confidence,
                    source_file,
                    source_location,
                    weight,
                )
            }
            "remove_edge" => {
                let (Some(source), Some(target)) = (source, target) else {
                    return "mutate_graph action 'remove_edge' requires source and target."
                        .to_string();
                };
                self.remove_edge(source, target, relation)
            }
            "recluster" => self.recluster(),
            other => format!(
                "Unknown mutate_graph action '{other}'. Expected one of: add_node, add_edge, remove_edge, recluster."
            ),
        }
    }

    /// Query the relational index with keywords to find relevant nodes and explore their topology.
    ///
    /// Use this read-only tool for initial discovery when you do not yet know the exact node IDs.
    /// If you already know the target ID, use `inspect_symbol` instead. To trace deep multi-hop
    /// paths, use `trace_path` instead.
    ///
    /// The query mechanism scores nodes by keyword match and traverses their neighborhood (BFS by
    /// default, or DFS if `dfs=true`). Results are rendered as a structured Markdown report. To
    /// prevent context bloat, the output is strictly truncated to fit within the provided `budget`
    /// (default 2000).
    #[tool(
        description = "Query the relational index with keywords to find relevant nodes and \
        explore their topology. Use this read-only tool for initial discovery when you do not yet \
        know the exact node IDs. If you already know the target ID, use inspect_symbol instead. To \
        trace deep multi-hop paths, use trace_path instead. The query mechanism scores nodes by \
        keyword match and traverses their neighborhood (BFS by default, or DFS if dfs=true). \
        Results are rendered as a structured Markdown report. To prevent context bloat, the output \
        is strictly truncated to fit within the provided budget (default 2000)."
    )]
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
        #[tool(param)]
        #[schemars(
            description = "Include test/spec nodes in results. Default false (test/spec excluded)."
        )]
        include_tests: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Minimum node degree to include (filters low-degree noise)")]
        min_degree: Option<i32>,
    ) -> String {
        let include_tests = include_tests.unwrap_or(false);
        let min_degree_val = min_degree.unwrap_or(0).max(0) as usize;
        if self.graph().node_count() == 0 {
            return "The relational index is currently empty. Please run `gm run . --no-semantic` in your project directory to compile the relational index.".to_string();
        }
        let depth = (depth.unwrap_or(3) as usize).clamp(1, 6);
        let budget = (budget.unwrap_or(2000) as usize).max(200);
        let use_dfs = dfs.unwrap_or(false);
        // Budget-enforced traversal cap: limits nodes visited during BFS/DFS,
        // preventing unbounded memory use before output formatting.
        let max_nodes = (budget / 40).clamp(5, 200);
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
            true,
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
        let ranked: Vec<_> = if !include_tests {
            ranked
                .into_iter()
                .filter(|node| {
                    graph.node_data(&node.id).is_none_or(|n| {
                        let p = n.source_file.to_lowercase();
                        let l = n.label.to_lowercase();
                        !p.contains("/test/")
                            && !p.contains("/tests/")
                            && !p.contains("_test.")
                            && !l.starts_with("test_")
                            && !l.starts_with("test")
                            && l != "test"
                    })
                })
                .collect()
        } else {
            ranked
        };
        let ranked: Vec<_> = if min_degree_val > 0 {
            ranked
                .into_iter()
                .filter(|node| {
                    graph.node_data(&node.id).is_none_or(|_| {
                        graph
                            .edges_iter()
                            .filter(|e| e.source == node.id || e.target == node.id)
                            .count()
                            >= min_degree_val
                    })
                })
                .collect()
        } else {
            ranked
        };
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

    // ── run_datalog ─────────────────────────────────────────────────────────

    #[tool(
        description = "Run a logic query over the codebase graph. Standard library predicates are pre-loaded (do not redefine them):\n\
                        - calls_transitive(X, Y) : X transitively calls Y\n\
                        - imports_transitive(X, Y) : X transitively imports Y\n\
                        - depends_transitive(X, Y) : X transitively depends on Y\n\
                        - same_community(X, Y) : X and Y are in the same Louvain cluster\n\
                        - is_hub(X) : X has > 15 connections\n\
                        - is_orphan(X) : X has no incoming/outgoing edges\n\
                        - circular_dependency(X, Y) : X and Y call each other\n\
                        - bypasses_layer(X, Y, Z) : X depends on Z, bypassing intermediary Y\n\
                        Base EDB relations: node, calls, imports, contains, inherits, implements, degree, edge.\n\
                        Supports user rules (:-), goals (?-), facts, and negation (not). Budget-bounded (max 10,000 steps)."
    )]
    fn run_datalog(
        &self,
        #[tool(param)]
        #[schemars(
            description = "Datalog program text. Supports goals (?-), user rules (:-), facts, and negation (not). Standard library predicates are pre-loaded."
        )]
        query: String,
        #[tool(param)]
        #[schemars(
            description = "Maximum solver steps before the query is bounded (default 1000, clamped to 100 through 10000)."
        )]
        step_budget: Option<usize>,
        #[tool(param)]
        #[schemars(
            description = "Output format: 'text' (default, Markdown) or 'json' (datalog.v1 machine-readable report with per-query rows, variables, and step metadata)."
        )]
        format: Option<String>,
    ) -> String {
        let graph = self.graph();
        let budget = step_budget.unwrap_or(1000).clamp(100, 10_000);
        let rendered = match format.as_deref().unwrap_or("text") {
            "json" => crate::analyze::query::run_datalog_json(&graph, &query, budget),
            _ => crate::analyze::query::run_datalog_query(&graph, &query, budget),
        };
        match rendered {
            Ok(r) => r,
            Err(e) => format!("Datalog error: {}", e),
        }
    }

    /// Hot-reload the graph backing this server.
    ///
    /// Reads `graph_path` from disk (falling back to the path the server was
    /// launched with) and atomically swaps it in. Active tool handlers that
    /// already took a snapshot via `self.graph()` keep seeing the old graph
    /// until their request finishes - swaps are lock-free on the read side.
    #[tool(
        description = "Reload the relational index from a graph.json file without restarting the \
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

        let mut msg = format!(
            "Reloaded graph from '{}': {} nodes, {} edges.",
            resolved_path.display(),
            n,
            e
        );
        let graph = self.graph();
        let project_root = graph.metadata.project_root.as_deref().map(Path::new);
        if let Some(warning) =
            super::freshness::check_staleness(&resolved_path, project_root).warning_message()
        {
            msg.push('\n');
            msg.push_str(&warning);
        }
        msg
    }

    /// Create a new virtual planning workspace.
    ///
    /// Use this tool *first* when starting any multi-file feature or refactoring task, before
    /// calling `add_planned_symbol` or `validate_plan`. Do not use this tool if a plan already
    /// exists for your current task.
    ///
    /// This is a mutating operation that initializes an empty Virtual AST associated with the
    /// provided `plan_id`. The workspace is stored in the server's graph state and persisted to
    /// disk immediately, so it remains available for the duration of the server session. If the
    /// `plan_id` already exists, this operation is idempotent and returns the existing workspace.
    /// Returns the initialized workspace ID on success.
    #[tool(
        description = "Create a new virtual planning workspace. Use this tool first when \
        starting any multi-file feature or refactoring task, before calling add_planned_symbol or \
        validate_plan. Do not use this tool if a plan already exists for your current task. This is \
        a mutating operation that initializes an empty Virtual AST associated with the provided \
        plan_id. The workspace is stored in the server's graph state and persisted to disk \
        immediately, so it remains available for the duration of the server session. If the plan_id \
        already exists, this operation is idempotent and returns the existing workspace. Returns \
        the initialized workspace ID on success."
    )]
    fn create_planning_workspace(
        &self,
        #[tool(param)]
        #[schemars(description = "Unique identifier for the planning workspace")]
        plan_id: String,
        #[tool(param)]
        #[schemars(description = "Human-readable description of the plan's purpose")]
        description: String,
    ) -> String {
        let mut graph = (*self.graph()).clone();
        if graph.metadata.extraction_modes.is_none() {
            graph.metadata.extraction_modes = Some(Vec::new());
        }
        let mode_str = format!("plan:{}:{}", plan_id, description);
        graph
            .metadata
            .extraction_modes
            .as_mut()
            .unwrap()
            .push(mode_str);
        self.graph_store.store(Arc::new(graph));
        let _persist = self.persist_graph().ok();
        format!("Planning workspace '{}' successfully created.", plan_id)
    }

    /// Register an intended new or modified symbol and its dependency edges in a virtual planning
    /// workspace.
    ///
    /// Use this tool to declare your architectural design *before* writing code. Do NOT use
    /// `mutate_graph` (which modifies the physical index) for planned changes.
    ///
    /// Prerequisites: You must first call `create_planning_workspace` to obtain a valid `plan_id`.
    /// Side effects: This is an in-memory mutation that adds a virtual node to the specified
    /// workspace. It automatically runs pre-flight policy checks; if the proposed symbol or its
    /// dependencies violate `.graphenium/policy.json` rules (for example, forbidden dependencies),
    /// the tool returns a `PRE_FLIGHT_VIOLATION` error and blocks the registration. If the
    /// `target_symbol` does not exist, the tool returns an error and records nothing. Duplicate
    /// calls for the same symbol are idempotent. Returns a success confirmation or a structural
    /// violation error.
    #[tool(
        description = "Register an intended new or modified symbol and its dependency edges in a \
        virtual planning workspace. Use this tool to declare your architectural design before \
        writing code. Do NOT use mutate_graph (which modifies the physical index) for planned \
        changes. Prerequisites: You must first call create_planning_workspace to obtain a valid \
        plan_id. Side effects: This is an in-memory mutation that adds a virtual node to the \
        specified workspace. It automatically runs pre-flight policy checks; if the proposed symbol \
        or its dependencies violate .graphenium/policy.json rules (for example, forbidden \
        dependencies), the tool returns a PRE_FLIGHT_VIOLATION error and blocks the registration. \
        If the target_symbol does not exist, the tool returns an error and records nothing. \
        Duplicate calls for the same symbol are idempotent. Returns a success confirmation or a \
        structural violation error."
    )]
    fn add_planned_symbol(
        &self,
        #[tool(param)]
        #[schemars(description = "Planning workspace ID")]
        plan_id: String,
        #[tool(param)]
        #[schemars(description = "Label for the new planned symbol")]
        label: String,
        #[tool(param)]
        #[schemars(description = "File path where the symbol will be added")]
        file_path: String,
        #[tool(param)]
        #[schemars(description = "Relation type (calls, depends_on, implements, etc.)")]
        relation: String,
        #[tool(param)]
        #[schemars(description = "ID or label of the existing node to link to")]
        target_symbol: String,
    ) -> String {
        let _graph = self.graph();
        let (resolved_targets, _) = self.resolve_symbols_to_ids(&target_symbol);
        let Some(target_id) = resolved_targets.first() else {
            return format!(
                "Error: Target symbol '{}' not found in graph.",
                target_symbol
            );
        };

        let planned_node_id = crate::model::make_id(&[&plan_id, &label]);
        let mut p_node = crate::model::Node::new(
            &planned_node_id,
            &label,
            crate::model::FileType::Code,
            &file_path,
        );
        p_node.plan_id = Some(plan_id.clone());
        p_node.extractor = Some("virtual-planner".to_string());
        p_node.resolution_status = Some("planned".to_string());

        let mut graph = (*self.graph()).clone();
        graph.upsert_node(p_node);

        let mut p_edge = crate::model::Edge::new(
            &planned_node_id,
            target_id,
            &relation,
            crate::model::Confidence::Inferred,
            &file_path,
        );
        p_edge.plan_id = Some(plan_id.clone());
        p_edge.extractor = Some("virtual-planner".to_string());
        p_edge.resolution_status = Some("planned".to_string());
        graph.add_edge(p_edge);

        let policy = self.load_arch_policy();
        if !policy.rules.is_empty() {
            let report = crate::harness::validate_plan_preflight(&graph, &plan_id, &policy.rules);
            if !report.passes {
                return Self::format_preflight_violation(&plan_id, &report);
            }
        }

        self.graph_store.store(Arc::new(graph));
        let _persist = self.persist_graph().ok();
        format!(
            "Planned symbol '{}' registered and linked to '{}'.",
            label, target_symbol
        )
    }
}

// ── ServerHandler ─────────────────────────────────────────────────────────────

impl ServerHandler for GrapheniumServer {
    async fn list_tools(
        &self,
        _: PaginatedRequestParam,
        _: RequestContext<rmcp::RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let tools = Self::tool_box()
            .list()
            .into_iter()
            .map(|mut tool| {
                let mut schema = tool.input_schema.as_ref().clone();
                super::tool_schema::sanitize_tool_input_schema(&mut schema);
                tool.input_schema = std::sync::Arc::new(schema);
                tool
            })
            .collect();
        Ok(ListToolsResult {
            next_cursor: None,
            tools,
        })
    }

    async fn call_tool(
        &self,
        call_tool_request_param: CallToolRequestParam,
        context: RequestContext<rmcp::RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let context = ToolCallContext::new(self, call_tool_request_param, context);
        Self::tool_box().call(context).await
    }

    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Graphenium relational index server with 12 consolidated tools. \
                 Use get_graph_info for the handshake, health checks, and diagnostics, \
                 get_architecture_summary for repo-level structure, communities, and hotspots, \
                 query_graph for keyword exploration, inspect_symbol for node details, neighbors, \
                 references, analysis, file summaries, and change explanations, and trace_path for \
                 shortest, safest, transitive, and cross-module paths. \
                 Use run_datalog for logic queries, create_planning_workspace and \
                 add_planned_symbol to declare designs, validate_plan for policy and delta gates, \
                 and verify_changes for scope-creep audits, blast radius, and verification plans. \
                 Use mutate_graph for add_node, add_edge, remove_edge, and recluster mutations, \
                 and reload_graph to hot-swap the underlying graph. All writes persist to disk immediately."
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
            Some(true),
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
            None,
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
            None,
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
            None,
            None,
        );
        assert!(result.contains("generated/template/vendor paths excluded"));
        assert!(!result.contains("TemplateScreen"));
    }

    #[test]
    fn inspect_symbol_returns_node_by_id() {
        let s = make_server();
        let result = s.inspect_symbol(
            "src_alpha".to_string(),
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
        assert!(result.contains("src/alpha.rs"));
        assert!(result.contains("Span: L10:C1-L24:C2"));
    }

    #[test]
    fn inspect_symbol_returns_node_by_label() {
        let s = make_server();
        let result = s.inspect_symbol(
            "Beta".to_string(),
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
        assert!(result.contains("src_beta"));
    }

    #[test]
    fn inspect_symbol_reports_missing_node() {
        let s = make_server();
        let result = s.inspect_symbol(
            "NoSuchNode".to_string(),
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
        assert!(result.contains("not found"));
    }

    #[test]
    fn inspect_symbol_can_include_neighbors() {
        let s = make_server();
        let result = s.inspect_symbol(
            "src_alpha".to_string(),
            Some(true),
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
        assert!(result.contains("Beta"));
        assert!(result.contains("calls"));
    }

    #[test]
    fn inspect_symbol_neighbor_relation_filter() {
        let s = make_server();
        let result = s.inspect_symbol(
            "src_alpha".to_string(),
            Some(true),
            None,
            None,
            None,
            None,
            Some("imports".to_string()),
            None,
            None,
            None,
            None,
        );
        assert!(result.contains("No neighbors found"));
    }

    #[test]
    fn inspect_symbol_deduplicates_neighbor_rows() {
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
        let result = s.inspect_symbol(
            "src_alpha".to_string(),
            Some(true),
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
        assert_eq!(result.matches("**Beta** via `calls`").count(), 1);
        assert!(result.contains("Total: 1 neighbor(s)"));
    }

    #[test]
    fn get_architecture_summary_returns_community_members() {
        let s = make_server();
        let result = s.get_architecture_summary(
            Some(0),
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
        assert!(result.contains("# Community 0"));
        assert!(result.contains("## Representative Nodes"));
        assert!(result.contains("## Representative Files"));
        assert!(result.contains("## Dominant Internal Relations"));
        assert!(result.contains("Alpha"));
    }

    #[test]
    fn get_architecture_summary_can_include_full_community_member_list() {
        let s = make_server();
        let result = s.get_architecture_summary(
            Some(0),
            Some(true),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert!(result.contains("## Members"));
        assert!(result.contains("Alpha"));
        assert!(result.contains("Beta"));
    }

    #[test]
    fn get_architecture_summary_reports_missing_community() {
        let s = make_server();
        let result = s.get_architecture_summary(
            Some(99),
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
        assert!(result.contains("not found"));
    }

    #[test]
    fn get_architecture_summary_can_include_god_nodes() {
        let s = make_server();
        let result = s.get_architecture_summary(
            None,
            None,
            Some(true),
            Some(5),
            None,
            None,
            None,
            None,
            None,
            None,
        );
        // Alpha has degree 1 (connected to Beta), which may be filtered as stub.
        // At minimum, the consolidated response should not panic or be empty.
        assert!(!result.is_empty());
    }

    #[test]
    fn get_architecture_summary_god_nodes_honor_path_scope() {
        let s = make_scoped_server();
        let result = s.get_architecture_summary(
            None,
            None,
            Some(true),
            Some(5),
            Some("tests/".to_string()),
            None,
            None,
            None,
            None,
            None,
        );
        assert!(result.contains("selected filter scope"));

        let src_result = s.get_architecture_summary(
            None,
            None,
            Some(true),
            Some(5),
            Some("src/".to_string()),
            None,
            None,
            None,
            None,
            None,
        );
        assert!(src_result.contains("OrderService"));
    }

    #[test]
    fn get_architecture_summary_god_nodes_honor_node_type_filter() {
        let s = make_server();
        let result = s.get_architecture_summary(
            None,
            None,
            Some(true),
            Some(5),
            None,
            None,
            Some(vec!["document".to_string()]),
            None,
            None,
            None,
        );
        assert!(result.contains("selected filter scope") || result.contains("too small"));
    }

    #[test]
    fn get_graph_info_can_include_stats() {
        let s = make_server();
        let result = s.get_graph_info(Some(true), None, None, None, None, None, None, None, None);
        assert!(result.contains("Nodes: 3"));
        assert!(result.contains("Edges: 1"));
    }

    #[test]
    fn get_graph_info_stats_honor_path_scope() {
        let s = make_scoped_server();
        let result = s.get_graph_info(
            Some(true),
            None,
            None,
            None,
            Some("tests/".to_string()),
            None,
            None,
            None,
            None,
        );
        assert!(result.contains("Nodes: 1"));
        assert!(result.contains("Edges: 0"));
    }

    #[test]
    fn get_graph_info_stats_honor_node_type_filter() {
        let s = make_server();
        let result = s.get_graph_info(
            Some(true),
            None,
            None,
            None,
            None,
            None,
            Some(vec!["document".to_string()]),
            None,
            None,
        );
        assert!(result.contains("Nodes: 1"));
        assert!(result.contains("document: 1"));
    }

    #[test]
    fn get_architecture_summary_reports_major_sections() {
        let s = make_architecture_server();
        let result = s.get_architecture_summary(
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(3),
        );
        assert!(result.contains("# Architecture Summary"));
        assert!(result.contains("## Largest Communities"));
        assert!(result.contains("## Cross-Community Connectors"));
        assert!(result.contains("## Architectural Hotspots"));
        assert!(result.contains("ApiController") || result.contains("ApiService"));
    }

    #[test]
    fn get_architecture_summary_honors_generated_code_mode() {
        let s = make_generated_server();
        let result = s.get_architecture_summary(
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some("only".to_string()),
            None,
            None,
        );
        assert!(result.contains("only generated/template/vendor paths included"));
    }

    #[test]
    fn get_architecture_summary_auto_tunes_for_ast_only_graphs() {
        let s = make_ast_only_generated_server();
        let result =
            s.get_architecture_summary(None, None, None, None, None, None, None, None, None, None);
        assert!(result.contains("generated/template/vendor paths excluded"));
    }

    #[test]
    fn trace_path_shortest_found() {
        let s = make_server();
        let result = s.trace_path(
            "src_alpha".to_string(),
            Some("src_beta".to_string()),
            Some("shortest".to_string()),
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
            None,
            None,
        );
        assert!(result.contains("Semantic path"));
        assert!(result.contains("hop"));
        assert!(result.contains("Alpha"));
        assert!(result.contains("Beta"));
    }

    #[test]
    fn trace_path_shortest_no_path() {
        let s = make_server();
        // Guide is disconnected
        let result = s.trace_path(
            "src_alpha".to_string(),
            Some("docs_guide".to_string()),
            Some("shortest".to_string()),
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
            None,
            None,
        );
        assert!(result.contains("No path"));
    }

    #[test]
    fn trace_path_shortest_same_node() {
        let s = make_server();
        let result = s.trace_path(
            "src_alpha".to_string(),
            Some("Alpha".to_string()),
            Some("shortest".to_string()),
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
            None,
            None,
        );
        assert!(result.contains("same node"));
    }

    #[test]
    fn trace_path_shortest_honors_relation_filters() {
        let s = make_server();
        let result = s.trace_path(
            "src_alpha".to_string(),
            Some("src_beta".to_string()),
            Some("shortest".to_string()),
            None,
            None,
            None,
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
    fn trace_path_rejects_unknown_path_mode() {
        let s = make_server();
        let result = s.trace_path(
            "src_alpha".to_string(),
            Some("src_beta".to_string()),
            Some("shortest".to_string()),
            None,
            None,
            None,
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
    fn trace_path_shortest_auto_tunes_for_ast_only_graphs() {
        let s = make_ast_only_generated_server();
        let result = s.trace_path(
            "TemplateScreen".to_string(),
            Some("RealService".to_string()),
            Some("shortest".to_string()),
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
    fn list_tools_exposes_exactly_twelve_consolidated_tools() {
        let tools = GrapheniumServer::tool_box().list();
        let mut names: Vec<String> = tools.iter().map(|tool| tool.name.to_string()).collect();
        names.sort();

        let expected = [
            "add_planned_symbol",
            "create_planning_workspace",
            "get_architecture_summary",
            "get_graph_info",
            "inspect_symbol",
            "mutate_graph",
            "query_graph",
            "reload_graph",
            "run_datalog",
            "trace_path",
            "validate_plan",
            "verify_changes",
        ];
        assert_eq!(
            names, expected,
            "the MCP surface must expose exactly the 12 consolidated tools"
        );
    }

    #[test]
    fn every_tool_parameter_has_a_schema_description() {
        let tools = GrapheniumServer::tool_box().list();
        for tool in tools {
            let properties = tool.input_schema["properties"]
                .as_object()
                .unwrap_or_else(|| panic!("tool {} has no properties object", tool.name));
            for (name, schema) in properties {
                let description = schema.get("description").and_then(|value| value.as_str());
                assert!(
                    description.is_some_and(|text| !text.trim().is_empty()),
                    "tool '{}' parameter '{}' is missing a JSON schema description",
                    tool.name,
                    name
                );
            }
        }
    }

    #[test]
    fn list_tools_sanitizes_gemini_incompatible_nullable_schemas() {
        let tools: Vec<_> = GrapheniumServer::tool_box()
            .list()
            .into_iter()
            .map(|mut tool| {
                let mut schema = tool.input_schema.as_ref().clone();
                crate::serve::tool_schema::sanitize_tool_input_schema(&mut schema);
                tool.input_schema = std::sync::Arc::new(schema);
                tool
            })
            .collect();

        let mutate_graph = tools
            .iter()
            .find(|tool| tool.name.as_ref() == "mutate_graph")
            .expect("mutate_graph tool should be registered");

        let weight = &mutate_graph.input_schema["properties"]["weight"];
        assert_eq!(weight["type"], serde_json::json!("number"));
        assert!(
            weight.get("anyOf").is_none(),
            "weight must not use anyOf for Gemini compatibility"
        );

        let source_location = &mutate_graph.input_schema["properties"]["source_location"];
        assert_eq!(source_location["type"], serde_json::json!("string"));
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
    fn inspect_symbol_file_summary_lists_symbols_matching_suffix() {
        let s = make_server();
        // make_server has src/alpha.rs (Alpha), src/beta.rs (Beta), docs/guide.md (Guide)
        let out = s.inspect_symbol(
            "alpha.rs".to_string(),
            None,
            None,
            None,
            Some(true),
            None,
            None,
            None,
            None,
            None,
            Some(true),
        );
        assert!(out.contains("Alpha"), "expected Alpha symbol: {out}");
        assert!(!out.contains("Beta"), "beta.rs should not match: {out}");
    }

    #[test]
    fn inspect_symbol_file_summary_handles_backslash_paths() {
        let s = make_server();
        // Windows-style input should normalize the same as forward slashes.
        let out = s.inspect_symbol(
            r"src\beta.rs".to_string(),
            None,
            None,
            None,
            Some(true),
            None,
            None,
            None,
            None,
            None,
            Some(true),
        );
        assert!(out.contains("Beta"), "expected Beta symbol: {out}");
    }

    #[test]
    fn inspect_symbol_file_summary_no_match_returns_clean_message() {
        let s = make_server();
        let out = s.inspect_symbol(
            "does_not_exist.rs".to_string(),
            None,
            None,
            None,
            Some(true),
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert!(out.contains("No nodes found"));
    }

    #[test]
    fn inspect_symbol_file_summary_can_group_by_community() {
        let s = make_server();
        let out = s.inspect_symbol(
            "alpha.rs".to_string(),
            None,
            None,
            None,
            Some(true),
            None,
            None,
            None,
            None,
            Some("community".to_string()),
            None,
        );
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

    #[test]
    fn mutate_graph_persist_roundtrip_preserves_all_edges() {
        use std::io::Write;

        // Reproduce the MCP mutate_graph add_edge persist flow:
        // 1. Build a graph with 3 nodes and 2 edges
        // 2. Serialize to JSON file (simulate persist)
        // 3. Reload via load_graph
        // 4. Clone graph, add a new edge (simulate mutate_graph action=add_edge)
        // 5. Serialize again
        // 6. Reload and verify ALL 3 edges are present
        let mut g = GrapheniumGraph::new();
        g.upsert_node(Node::new("a", "A", FileType::Code, "src/a.rs"));
        g.upsert_node(Node::new("b", "B", FileType::Code, "src/b.rs"));
        g.upsert_node(Node::new("c", "C", FileType::Code, "src/c.rs"));
        g.add_edge(Edge::extracted("a", "b", "calls", "src/a.rs"));
        g.add_edge(Edge::extracted("b", "c", "calls", "src/b.rs"));
        assert_eq!(g.edge_count(), 2);

        // Step 1-3: Serialize to file, reload (simulate initial persist)
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("graph.json");
        let json1 = crate::export::json::to_json(&g).unwrap();
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(json1.as_bytes()).unwrap();
        drop(f);
        let g_loaded = crate::export::json::load_graph(&path).unwrap();
        assert_eq!(g_loaded.edge_count(), 2);

        // Step 4: Clone graph, add edge (MCP mutate_graph action=add_edge flow)
        let mut cloned = g.clone();
        cloned.add_edge(Edge::extracted("c", "a", "depends_on", "src/c.rs"));
        assert_eq!(cloned.edge_count(), 3);

        // Step 5-6: Serialize to file, reload (simulate persist after mutate_graph action=add_edge)
        let json2 = crate::export::json::to_json(&cloned).unwrap();
        let mut f2 = std::fs::File::create(&path).unwrap();
        f2.write_all(json2.as_bytes()).unwrap();
        drop(f2);
        let g_final = crate::export::json::load_graph(&path).unwrap();
        assert_eq!(
            g_final.edge_count(),
            3,
            "BUG: add_edge persist lost edges! Expected 3, got {}",
            g_final.edge_count()
        );

        // Verify all edges are of the correct type
        let relation_types: std::collections::BTreeSet<&str> =
            g_final.edges_iter().map(|e| e.relation.as_str()).collect();
        assert!(relation_types.contains("calls"));
        assert!(relation_types.contains("depends_on"));
    }

    #[test]
    fn verify_changes_reports_missing_planned_edges() {
        use crate::model::Confidence;

        let mut g = GrapheniumGraph::new();
        let mut plan_alpha = Node::new("plan_alpha", "Alpha", FileType::Code, "src/a.rs");
        plan_alpha.plan_id = Some("plan-x".to_string());
        g.upsert_node(plan_alpha);
        let mut plan_beta = Node::new("plan_beta", "Beta", FileType::Code, "src/b.rs");
        plan_beta.plan_id = Some("plan-x".to_string());
        g.upsert_node(plan_beta);
        g.upsert_node(Node::new("real_alpha", "Alpha", FileType::Code, "src/a.rs"));
        g.upsert_node(Node::new("real_beta", "Beta", FileType::Code, "src/b.rs"));

        let mut planned_edge = Edge::new(
            "plan_alpha",
            "plan_beta",
            "calls",
            Confidence::Extracted,
            "src/a.rs",
        );
        planned_edge.plan_id = Some("plan-x".to_string());
        g.add_edge(planned_edge);

        let server = GrapheniumServer::new(g);
        let output = server.verify_changes(
            Some("plan-x".to_string()),
            Some(vec!["real_alpha".to_string()]),
            None,
            None,
            None,
            None,
            None,
        );

        assert!(output.contains("## Post-Edit Compliance"), "{output}");
        assert!(output.contains("Missing edges"), "{output}");
        assert!(output.contains("Alpha -[calls]-> Beta"), "{output}");
    }

    #[test]
    fn get_graph_info_can_append_stats_and_diagnostics() {
        let s = make_server();
        let result = s.get_graph_info(
            Some(true),
            Some(true),
            Some(true),
            Some(true),
            None,
            None,
            None,
            None,
            None,
        );

        assert!(result.contains("# Graph Info"));
        assert!(result.contains("# Graph Statistics"));
        assert!(result.contains("No unresolved references found."));
        assert!(result.contains("Resolution coverage:"));
        assert!(result.contains("No ambiguous symbols or label collisions found."));
    }

    #[test]
    fn get_architecture_summary_can_focus_on_community_and_include_god_nodes() {
        let s = make_architecture_server();
        let result = s.get_architecture_summary(
            Some(1),
            Some(true),
            Some(true),
            Some(3),
            None,
            None,
            None,
            None,
            None,
            Some(3),
        );

        assert!(result.contains("# Community 1"), "{result}");
        assert!(result.contains("## Members"), "{result}");
        assert!(
            result.contains("Hub Nodes") || result.contains("No hub nodes"),
            "{result}"
        );
    }

    #[test]
    fn inspect_symbol_composes_detail_neighbors_references_analysis_and_file_summary() {
        let s = make_server();
        let result = s.inspect_symbol(
            "Alpha".to_string(),
            Some(true),
            Some(true),
            Some(true),
            Some(true),
            Some(true),
            None,
            None,
            None,
            None,
            Some(true),
        );

        assert!(result.contains("**Alpha**"), "{result}");
        assert!(result.contains("# Neighbors of Alpha"), "{result}");
        assert!(
            result.contains("# Structural References for 'Alpha'")
                || result.contains("No structural references"),
            "{result}"
        );
        assert!(result.contains("## Node: Alpha"), "{result}");
        assert!(result.contains("### Behavioral Connections"), "{result}");
        assert!(result.contains("# File Summary: src/alpha.rs"), "{result}");
    }

    #[test]
    fn trace_path_routes_shortest_safest_transitive_and_module_modes() {
        let s = make_server();

        let shortest = s.trace_path(
            "src_alpha".to_string(),
            Some("src_beta".to_string()),
            Some("shortest".to_string()),
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
            None,
            None,
        );
        assert!(shortest.contains("Semantic path"), "{shortest}");

        let safest = s.trace_path(
            "src_alpha".to_string(),
            Some("src_beta".to_string()),
            Some("safest".to_string()),
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
            None,
            None,
        );
        assert!(safest.contains("Safest Path"), "{safest}");

        let transitive = s.trace_path(
            "src_alpha".to_string(),
            None,
            Some("transitive_forward".to_string()),
            Some(2),
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
            None,
        );
        assert!(
            transitive.contains("# Transitive Closure from 'src_alpha'"),
            "{transitive}"
        );

        let modules = s.trace_path(
            "src".to_string(),
            Some("src".to_string()),
            Some("module_dependencies".to_string()),
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
            None,
            None,
        );
        assert!(modules.contains("# Module Dependencies"), "{modules}");

        let unknown = s.trace_path(
            "src_alpha".to_string(),
            None,
            Some("sideways".to_string()),
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
            None,
            None,
        );
        assert!(unknown.contains("Unknown trace mode"), "{unknown}");

        let missing_target = s.trace_path(
            "src_alpha".to_string(),
            None,
            Some("shortest".to_string()),
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
            None,
            None,
        );
        assert!(
            missing_target.contains("requires a target"),
            "{missing_target}"
        );
    }

    #[test]
    fn validate_plan_always_runs_the_delta_gate() {
        let s = make_server();
        let result = s.validate_plan("missing-plan".to_string(), Some(true), None, None);
        assert!(result.contains("Topological Delta Report"), "{result}");
    }

    #[test]
    fn mutate_graph_routes_add_node_add_edge_remove_edge_and_recluster() {
        let s = make_server();

        let added = s.mutate_graph(
            "add_node".to_string(),
            Some("manifest_note".to_string()),
            Some("Manifest Note".to_string()),
            Some("rationale".to_string()),
            Some("docs/notes.md".to_string()),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert!(added.contains("Added node"), "{added}");
        assert_eq!(s.graph().node_count(), 4);

        let edge = s.mutate_graph(
            "add_edge".to_string(),
            None,
            None,
            None,
            Some("docs/notes.md".to_string()),
            None,
            None,
            Some("manifest_note".to_string()),
            Some("src_alpha".to_string()),
            Some("rationale_for".to_string()),
            Some("EXTRACTED".to_string()),
            None,
        );
        assert!(edge.contains("Added edge"), "{edge}");
        assert_eq!(s.graph().edge_count(), 2);

        let removed = s.mutate_graph(
            "remove_edge".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
            Some("manifest_note".to_string()),
            Some("src_alpha".to_string()),
            Some("rationale_for".to_string()),
            None,
            None,
        );
        assert!(removed.contains("Removed 1 edge"), "{removed}");
        assert_eq!(s.graph().edge_count(), 1);

        let reclustered = s.mutate_graph(
            "recluster".to_string(),
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
            None,
        );
        assert!(reclustered.contains("Reclustered"), "{reclustered}");
    }
}
