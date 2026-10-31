//! Trust harness: validates claim quality and evidence freshness in CI.
//!
//! Use this to gate CI pipelines on trust quality:
//!   gm check --min-resolution 90 --max-stale 5

use std::path::Path;

use globset::{Glob, GlobMatcher};

use crate::analyze::delta::evaluate_delta_gate;
use crate::model::graph::GrapheniumGraph;
use crate::model::{Edge, Node};
use crate::policy::{ArchPolicyConfig, ArchRule};
use crate::trust::ResolutionReport;

/// Result of a trust check, suitable for CI gate logic.
#[derive(Debug, Clone, Default)]
pub struct TrustCheckResult {
    pub resolution_pct: f64,
    pub stale_evidence_count: usize,
    pub ambiguous_edge_count: usize,

    pub passed: bool,
    pub details: Vec<String>,
}

/// Run a trust check against the graph.
/// Returns a `TrustCheckResult` with pass/fail based on thresholds.
pub fn check_resolution_quality(
    graph: &GrapheniumGraph,
    report: &ResolutionReport,
    min_resolution_pct: f64,
    max_ambiguous: usize,
) -> TrustCheckResult {
    let mut result = TrustCheckResult::default();
    let mut details = Vec::new();

    // Dynamically scale metrics based on graph capability
    let import_pct = if report.total_import_edges > 0 {
        (report.resolved_imports as f64 / report.total_import_edges as f64) * 100.0
    } else {
        100.0
    };

    let call_pct = if report.total_call_edges > 0 {
        (report.resolved_calls as f64 / report.total_call_edges as f64) * 100.0
    } else {
        100.0
    };

    // Combined resolution, scaled for AST-only vs semantic
    let (total_relevant, total_resolved) = if graph.is_ast_only() {
        // AST-only: only imports and methods have resolution data
        let relevant = report.total_import_edges + report.total_method_edges;
        let resolved = report.resolved_imports + report.resolved_methods;
        (relevant, resolved)
    } else {
        // Semantic: includes call resolution
        let relevant =
            report.total_import_edges + report.total_call_edges + report.total_method_edges;
        let resolved = report.resolved_imports + report.resolved_calls + report.resolved_methods;
        (relevant, resolved)
    };

    let resolution_pct = if total_relevant > 0 {
        (total_resolved as f64 / total_relevant as f64) * 100.0
    } else {
        100.0
    };
    result.resolution_pct = resolution_pct;

    details.push(format!(
        "Import resolution: {:.0}% ({}/{})",
        import_pct, report.resolved_imports, report.total_import_edges
    ));

    if !graph.is_ast_only() {
        details.push(format!(
            "Call resolution: {:.0}% ({}/{})",
            call_pct, report.resolved_calls, report.total_call_edges
        ));
    } else {
        details.push(
            "Call resolution: [SKIPPED] (AST-only — run with semantic pass for call resolution)"
                .to_string(),
        );
    }
    details.push(format!(
        "Combined resolution: {:.0}% (threshold: {:.0}%)",
        resolution_pct, min_resolution_pct
    ));

    // Stale evidence
    result.stale_evidence_count = report.evidence_stale;
    if report.evidence_stale > 0 {
        details.push(format!(
            "WARNING: {} evidence spans are stale",
            report.evidence_stale
        ));
    }

    // Ambiguous edges
    result.ambiguous_edge_count = report.ambiguous_edges;
    details.push(format!(
        "Ambiguous edges: {} (threshold: {} max)",
        report.ambiguous_edges, max_ambiguous
    ));

    // Gate check
    let resolution_ok = resolution_pct >= min_resolution_pct;
    let ambiguous_ok = report.ambiguous_edges <= max_ambiguous;
    result.passed = resolution_ok && ambiguous_ok;

    if !resolution_ok {
        details.push(format!(
            "FAIL: Resolution {:.0}% < minimum {:.0}%",
            resolution_pct, min_resolution_pct
        ));
    }
    if !ambiguous_ok {
        details.push(format!(
            "FAIL: {} ambiguous edges > max {}",
            report.ambiguous_edges, max_ambiguous
        ));
    }

    result.details = details;
    result
}

// ── Plan Verification Engine (k) ─────────────────────────────────────────────

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct PlanVerificationReport {
    pub plan_id: String,
    pub implemented_nodes: Vec<String>,
    pub missing_nodes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub implemented_edges: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing_edges: Vec<String>,
    pub unplanned_modified_files: Vec<String>,
    pub passes_compliance: bool,
}

/// Normalize a file path for cross-platform set comparison.
fn normalize_plan_path(path: &str, project_root: Option<&str>) -> String {
    let clean = path
        .strip_prefix(r"\\?\")
        .unwrap_or(path)
        .replace('\\', "/");
    let stripped = if let Some(root) = project_root {
        let root_clean = root
            .strip_prefix(r"\\?\")
            .unwrap_or(root)
            .replace('\\', "/");
        let root_trimmed = root_clean.trim_end_matches('/');
        clean
            .strip_prefix(root_trimmed)
            .map(|s| s.trim_start_matches('/').to_string())
            .unwrap_or(clean)
    } else {
        clean
    };
    stripped.trim_start_matches("./").to_string()
}

/// Query git for files modified in the working tree/index relative to HEAD, if available.
/// Returns `None` if git is unavailable, not a git repo, or no project_root is set.
fn detect_git_modified_files(
    project_root: Option<&str>,
) -> Option<std::collections::HashSet<String>> {
    let root = project_root?;
    let root_path = std::path::Path::new(root);
    if !root_path.join(".git").exists() {
        return None;
    }

    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["status", "--porcelain", "--untracked-files=normal"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut modified = std::collections::HashSet::new();
    for line in stdout.lines() {
        if line.len() < 4 {
            continue;
        }
        let raw_path = line[3..].trim();
        // Handle rename syntax `old -> new`
        let effective_path = raw_path
            .rsplit(" -> ")
            .next()
            .unwrap_or(raw_path)
            .trim_matches('"');
        let norm = normalize_plan_path(effective_path, Some(root));
        if !norm.is_empty()
            && !norm.starts_with("graphenium-out/")
            && !norm.starts_with("graphenium-snapshots/")
            && !norm.starts_with(".graphenium/")
        {
            modified.insert(norm);
        }
    }
    Some(modified)
}

/// Verify that a planning workspace's declared changes match the real graph.
pub fn verify_plan(graph: &GrapheniumGraph, plan_id: &str) -> PlanVerificationReport {
    verify_plan_with_modified_files(graph, plan_id, None)
}

/// Verify a planning workspace against the real graph, optionally supplying an
/// explicit set of modified file paths (e.g. from a snapshot diff or git status).
pub fn verify_plan_with_modified_files(
    graph: &GrapheniumGraph,
    plan_id: &str,
    modified_files_override: Option<&std::collections::HashSet<String>>,
) -> PlanVerificationReport {
    let project_root = graph.metadata.project_root.as_deref();
    let mut implemented_nodes = Vec::new();
    let mut missing_nodes = Vec::new();
    let mut implemented_edges = Vec::new();
    let mut missing_edges = Vec::new();
    let mut planned_files = std::collections::HashSet::new();

    let planned_nodes: Vec<&Node> = graph
        .nodes()
        .filter(|n| n.plan_id.as_deref() == Some(plan_id))
        .collect();

    for p_node in &planned_nodes {
        planned_files.insert(normalize_plan_path(&p_node.source_file, project_root));

        let has_real_impl = graph.nodes().any(|n| {
            n.plan_id.is_none() && n.id != p_node.id && n.label.eq_ignore_ascii_case(&p_node.label)
        });

        if has_real_impl {
            implemented_nodes.push(p_node.label.clone());
        } else {
            missing_nodes.push(p_node.label.clone());
        }
    }

    // Verify planned edges against physical edges in the graph.
    let planned_edges: Vec<&Edge> = graph
        .edges_iter()
        .filter(|e| e.plan_id.as_deref() == Some(plan_id))
        .collect();

    for p_edge in &planned_edges {
        if !p_edge.source_file.is_empty() {
            planned_files.insert(normalize_plan_path(&p_edge.source_file, project_root));
        }

        let src_label = graph
            .node_data(&p_edge.source)
            .map(|n| n.label.as_str())
            .unwrap_or(p_edge.src_original.as_str());
        let tgt_label = graph
            .node_data(&p_edge.target)
            .map(|n| n.label.as_str())
            .unwrap_or(p_edge.tgt_original.as_str());

        let edge_desc = format!("{src_label} -[{}]-> {tgt_label}", p_edge.relation);

        // Only require physical edge realization if both endpoints have physical nodes
        let src_real_ids: Vec<&str> = graph
            .nodes()
            .filter(|n| n.plan_id.is_none() && n.label.eq_ignore_ascii_case(src_label))
            .map(|n| n.id.as_str())
            .collect();
        let tgt_real_ids: Vec<&str> = graph
            .nodes()
            .filter(|n| n.plan_id.is_none() && n.label.eq_ignore_ascii_case(tgt_label))
            .map(|n| n.id.as_str())
            .collect();

        if !src_real_ids.is_empty() && !tgt_real_ids.is_empty() {
            let has_real_edge = graph.edges_iter().any(|e| {
                e.plan_id.is_none()
                    && e.relation.eq_ignore_ascii_case(&p_edge.relation)
                    && ((src_real_ids.contains(&e.source.as_str())
                        && tgt_real_ids.contains(&e.target.as_str()))
                        || (src_real_ids.contains(&e.target.as_str())
                            && tgt_real_ids.contains(&e.source.as_str())))
            });
            if has_real_edge {
                implemented_edges.push(edge_desc);
            } else {
                missing_edges.push(edge_desc);
            }
        }
    }

    // Determine modified files:
    // 1. Use explicit override if provided by caller, OR
    // 2. Use git working-tree modified files if project_root is a git repo, OR
    // 3. Otherwise (in-memory unit tests without git), only inspect files when
    //    explicitly provided so untouched baseline repo files are never falsely flagged.
    let candidate_modified_files: std::collections::HashSet<String> =
        if let Some(override_set) = modified_files_override {
            override_set
                .iter()
                .map(|p| normalize_plan_path(p, project_root))
                .collect()
        } else {
            detect_git_modified_files(project_root).unwrap_or_default()
        };

    let mut unplanned_modified_files: Vec<String> = if planned_files.is_empty() {
        Vec::new()
    } else {
        candidate_modified_files
            .difference(&planned_files)
            .filter(|f| !crate::detect::path::is_test_like_path(f))
            .cloned()
            .collect()
    };
    unplanned_modified_files.sort();
    unplanned_modified_files.truncate(20);

    let passes_compliance =
        missing_nodes.is_empty() && missing_edges.is_empty() && unplanned_modified_files.is_empty();

    PlanVerificationReport {
        plan_id: plan_id.to_string(),
        implemented_nodes,
        missing_nodes,
        implemented_edges,
        missing_edges,
        unplanned_modified_files,
        passes_compliance,
    }
}

// ── Pre-Flight Architecture Policy Validation ────────────────────────────────

/// Result of pre-flight architecture policy validation on a planning workspace.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PreFlightReport {
    pub plan_id: String,
    pub passes: bool,
    pub violations: Vec<String>,
}

/// Isolates the planned (virtual) nodes and edges for evaluation.
pub fn get_planned_subgraph(graph: &GrapheniumGraph, plan_id: &str) -> (Vec<Node>, Vec<Edge>) {
    let nodes: Vec<Node> = graph
        .nodes()
        .filter(|n| n.plan_id.as_deref() == Some(plan_id))
        .cloned()
        .collect();

    let edges: Vec<Edge> = graph
        .edges_iter()
        .filter(|e| e.plan_id.as_deref() == Some(plan_id))
        .cloned()
        .collect();

    (nodes, edges)
}

fn compile_glob(pattern: &str) -> Result<GlobMatcher, String> {
    Ok(Glob::new(pattern)
        .map_err(|e| format!("Invalid glob pattern '{pattern}': {e}"))?
        .compile_matcher())
}

/// Validate a planning workspace against architecture policy rules before coding.
pub fn validate_plan_preflight(
    graph: &GrapheniumGraph,
    plan_id: &str,
    rules: &[ArchRule],
) -> PreFlightReport {
    let (p_nodes, p_edges) = get_planned_subgraph(graph, plan_id);
    let mut violations = Vec::new();

    for rule in rules {
        match rule {
            ArchRule::ForbiddenDependency {
                from_pattern,
                to_pattern,
                reason,
            } => {
                let Ok(from_glob) = compile_glob(from_pattern) else {
                    violations.push(format!(
                        "Invalid forbidden_dependency from_pattern '{from_pattern}'"
                    ));
                    continue;
                };
                let Ok(to_glob) = compile_glob(to_pattern) else {
                    violations.push(format!(
                        "Invalid forbidden_dependency to_pattern '{to_pattern}'"
                    ));
                    continue;
                };

                for edge in &p_edges {
                    let src_file = resolve_edge_source_file(graph, edge);
                    let Some(target_node) = graph.node_data(&edge.target) else {
                        continue;
                    };
                    if from_glob.is_match(&src_file) && to_glob.is_match(&target_node.source_file) {
                        violations.push(format!(
                            "Planned edge {} -> {} violates forbidden dependency rule: {from_pattern} -> {to_pattern} (Reason: {reason})",
                            edge.source, edge.target
                        ));
                    }
                }
            }
            ArchRule::BannedSymbol {
                symbol_label,
                reason,
            } => {
                let banned = symbol_label.trim();
                for node in &p_nodes {
                    if node.label == banned || node.id == banned {
                        violations.push(format!(
                            "Planned symbol '{banned}' is banned (Reason: {reason})"
                        ));
                    }
                }
                for edge in &p_edges {
                    if edge.target == banned
                        || graph
                            .node_data(&edge.target)
                            .is_some_and(|n| n.label == banned)
                    {
                        violations.push(format!(
                            "Planned call/dependency target '{banned}' is banned (Reason: {reason})"
                        ));
                    }
                }
            }
            ArchRule::StrictLayering { layers, reason } => {
                check_strict_layering_violations(graph, plan_id, layers, reason, &mut violations);
            }
        }
    }

    check_transitive_violations_with_datalog(graph, plan_id, rules, &mut violations);

    PreFlightReport {
        plan_id: plan_id.to_string(),
        passes: violations.is_empty(),
        violations,
    }
}

/// Validate a planning workspace against explicit policy rules and dynamic delta gating.
pub fn validate_plan(
    graph: &GrapheniumGraph,
    plan_id: &str,
    project_root: &Path,
) -> Result<PreFlightReport, String> {
    let mut violations = Vec::new();
    let mut passes = true;

    // 1. Try to load hardcoded policy rules
    if let Ok(policy_config) = ArchPolicyConfig::load_for_project(project_root) {
        if !policy_config.rules.is_empty() {
            let report = validate_plan_preflight(graph, plan_id, &policy_config.rules);
            violations.extend(report.violations);
            passes = report.passes;
        }
    }

    // 2. If passes is still true, run the dynamic Topological Delta check as an invariant gate
    if passes {
        if let Ok(delta_report) = evaluate_delta_gate(graph, plan_id, -0.02, 5.0) {
            if !delta_report.passes {
                passes = false;
                for edge in delta_report.plan_surprise_edges {
                    violations.push(format!(
                        "Topological Entropy Violation: Planned relationship {} -> {} exceeds surprise threshold ({:.1}). Reason: {}",
                        edge.source, edge.target, edge.score, edge.reasons.join(", ")
                    ));
                }
            }
        }
    }

    Ok(PreFlightReport {
        plan_id: plan_id.to_string(),
        passes,
        violations,
    })
}

fn resolve_edge_source_file(graph: &GrapheniumGraph, edge: &Edge) -> String {
    graph
        .node_data(&edge.source)
        .map(|n| n.source_file.clone())
        .unwrap_or_else(|| edge.source_file.clone())
}

fn check_strict_layering_violations(
    graph: &GrapheniumGraph,
    plan_id: &str,
    layers: &[String],
    reason: &str,
    violations: &mut Vec<String>,
) {
    if layers.len() < 2 {
        return;
    }

    let matchers: Vec<Result<GlobMatcher, String>> =
        layers.iter().map(|l| compile_glob(l)).collect();

    let (_, p_edges) = get_planned_subgraph(graph, plan_id);

    for (i, from_matcher) in matchers.iter().enumerate() {
        let Ok(from_glob) = from_matcher else {
            violations.push(format!(
                "Invalid strict_layering layer pattern '{}'",
                layers[i]
            ));
            continue;
        };
        for j in 0..i {
            let Ok(to_glob) = &matchers[j] else {
                violations.push(format!(
                    "Invalid strict_layering layer pattern '{}'",
                    layers[j]
                ));
                continue;
            };

            for edge in &p_edges {
                let src_file = resolve_edge_source_file(graph, edge);
                let Some(target_node) = graph.node_data(&edge.target) else {
                    continue;
                };
                if from_glob.is_match(&src_file) && to_glob.is_match(&target_node.source_file) {
                    violations.push(format!(
                        "Planned edge {} -> {} violates strict layering: {} must not depend on {} (Reason: {reason})",
                        edge.source, edge.target, layers[i], layers[j]
                    ));
                }
            }
        }
    }
}

fn check_transitive_violations_with_datalog(
    graph: &GrapheniumGraph,
    plan_id: &str,
    rules: &[ArchRule],
    violations: &mut Vec<String>,
) {
    let has_layering = rules
        .iter()
        .any(|r| matches!(r, ArchRule::StrictLayering { .. }));
    if !has_layering {
        return;
    }

    let (p_nodes, _) = get_planned_subgraph(graph, plan_id);
    if p_nodes.is_empty() {
        return;
    }

    for rule in rules {
        let ArchRule::StrictLayering { layers, reason } = rule else {
            continue;
        };
        if layers.len() < 2 {
            continue;
        }

        let matchers: Vec<Result<GlobMatcher, String>> =
            layers.iter().map(|l| compile_glob(l)).collect();

        for (i, from_matcher) in matchers.iter().enumerate() {
            let Ok(from_glob) = from_matcher else {
                continue;
            };
            for j in 0..i {
                let Ok(to_glob) = &matchers[j] else {
                    continue;
                };

                let from_nodes: Vec<_> = p_nodes
                    .iter()
                    .filter(|n| from_glob.is_match(&n.source_file))
                    .collect();
                let to_nodes: Vec<_> = graph
                    .nodes()
                    .filter(|n| to_glob.is_match(&n.source_file))
                    .collect();

                for from_node in &from_nodes {
                    for to_node in &to_nodes {
                        if from_node.id == to_node.id {
                            continue;
                        }
                        if violations
                            .iter()
                            .any(|v| v.contains(&from_node.id) && v.contains(&to_node.id))
                        {
                            continue;
                        }
                        match crate::analyze::query::depends_transitive(
                            graph,
                            &from_node.id,
                            &to_node.id,
                            1000,
                        ) {
                            Ok(true) => violations.push(format!(
                                "Transitive layering violation in plan '{plan_id}': {} ({}) depends on {} ({}) (Reason: {reason})",
                                from_node.label,
                                layers[i],
                                to_node.label,
                                layers[j]
                            )),
                            Ok(false) => {}
                            Err(_) => {}
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trust_check_passes_with_good_resolution() {
        let report = ResolutionReport {
            total_import_edges: 100,
            resolved_imports: 95,
            ..Default::default()
        };
        let graph = GrapheniumGraph::new();
        let result = check_resolution_quality(&graph, &report, 90.0, 5);
        assert!(result.passed);
    }

    #[test]
    fn trust_check_fails_with_low_resolution() {
        let report = ResolutionReport {
            total_import_edges: 100,
            resolved_imports: 50,
            ..Default::default()
        };
        let graph = GrapheniumGraph::new();
        let result = check_resolution_quality(&graph, &report, 90.0, 5);
        assert!(!result.passed);
        assert!(result.details.iter().any(|d| d.contains("FAIL")));
    }

    #[test]
    fn trust_check_fails_with_many_ambiguous_edges() {
        let report = ResolutionReport {
            total_import_edges: 10,
            resolved_imports: 10,
            ambiguous_edges: 10,
            ..Default::default()
        };
        let graph = GrapheniumGraph::new();
        let result = check_resolution_quality(&graph, &report, 90.0, 5);
        assert!(!result.passed);
    }

    #[test]
    fn verify_plan_does_not_flag_untouched_baseline_files_as_scope_creep() {
        use crate::model::FileType;

        let mut graph = GrapheniumGraph::new();
        // 5 untouched baseline files in the repository
        for i in 0..5 {
            graph.upsert_node(Node::new(
                &format!("untouched_{i}"),
                &format!("Untouched{i}"),
                FileType::Code,
                &format!("src/module_{i}.rs"),
            ));
        }

        // Plan only touches src/feature.rs
        let mut p_node = Node::new("plan_feat", "NewFeature", FileType::Code, "src/feature.rs");
        p_node.plan_id = Some("plan-clean".to_string());
        graph.upsert_node(p_node);

        // Physical implementation of NewFeature in src/feature.rs
        graph.upsert_node(Node::new(
            "real_feat",
            "NewFeature",
            FileType::Code,
            "src/feature.rs",
        ));

        let report = verify_plan(&graph, "plan-clean");
        assert!(
            report.unplanned_modified_files.is_empty(),
            "Untouched repo files must not be flagged as unplanned edits: {:?}",
            report.unplanned_modified_files
        );
        assert!(report.passes_compliance);
    }

    #[test]
    fn verify_plan_with_modified_files_detects_real_scope_creep() {
        use crate::model::FileType;

        let mut graph = GrapheniumGraph::new();
        let mut p_node = Node::new("plan_feat", "NewFeature", FileType::Code, "src/feature.rs");
        p_node.plan_id = Some("plan-creep".to_string());
        graph.upsert_node(p_node);
        graph.upsert_node(Node::new(
            "real_feat",
            "NewFeature",
            FileType::Code,
            "src/feature.rs",
        ));

        let modified = std::collections::HashSet::from([
            "src/feature.rs".to_string(),
            "src/unrelated_sneaky_edit.rs".to_string(),
        ]);

        let report = verify_plan_with_modified_files(&graph, "plan-creep", Some(&modified));
        assert!(!report.passes_compliance);
        assert_eq!(
            report.unplanned_modified_files,
            vec!["src/unrelated_sneaky_edit.rs".to_string()]
        );
    }

    #[test]
    fn verify_plan_fails_when_planned_edge_is_missing() {
        use crate::model::{Confidence, FileType};

        let mut graph = GrapheniumGraph::new();
        let mut plan_alpha = Node::new("plan_alpha", "Alpha", FileType::Code, "src/a.rs");
        plan_alpha.plan_id = Some("plan-edge".to_string());
        graph.upsert_node(plan_alpha);
        let mut plan_beta = Node::new("plan_beta", "Beta", FileType::Code, "src/b.rs");
        plan_beta.plan_id = Some("plan-edge".to_string());
        graph.upsert_node(plan_beta);

        // Both endpoints exist physically, but the planned dependency edge was not written.
        graph.upsert_node(Node::new("real_alpha", "Alpha", FileType::Code, "src/a.rs"));
        graph.upsert_node(Node::new("real_beta", "Beta", FileType::Code, "src/b.rs"));

        let mut planned_edge = Edge::new(
            "plan_alpha",
            "plan_beta",
            "calls",
            Confidence::Extracted,
            "src/a.rs",
        );
        planned_edge.plan_id = Some("plan-edge".to_string());
        graph.add_edge(planned_edge);

        let report = verify_plan(&graph, "plan-edge");
        assert_eq!(report.missing_edges.len(), 1);
        assert!(!report.passes_compliance);
    }

    #[test]
    fn verify_plan_passes_when_planned_edge_is_implemented() {
        use crate::model::{Confidence, FileType};

        let mut graph = GrapheniumGraph::new();
        let mut plan_alpha = Node::new("plan_alpha", "Alpha", FileType::Code, "src/a.rs");
        plan_alpha.plan_id = Some("plan-edge-ok".to_string());
        graph.upsert_node(plan_alpha);
        let mut plan_beta = Node::new("plan_beta", "Beta", FileType::Code, "src/b.rs");
        plan_beta.plan_id = Some("plan-edge-ok".to_string());
        graph.upsert_node(plan_beta);

        graph.upsert_node(Node::new("real_alpha", "Alpha", FileType::Code, "src/a.rs"));
        graph.upsert_node(Node::new("real_beta", "Beta", FileType::Code, "src/b.rs"));
        graph.add_edge(Edge::extracted(
            "real_alpha",
            "real_beta",
            "calls",
            "src/a.rs",
        ));

        let mut planned_edge = Edge::new(
            "plan_alpha",
            "plan_beta",
            "calls",
            Confidence::Extracted,
            "src/a.rs",
        );
        planned_edge.plan_id = Some("plan-edge-ok".to_string());
        graph.add_edge(planned_edge);

        let report = verify_plan(&graph, "plan-edge-ok");
        assert!(report.missing_edges.is_empty());
        assert_eq!(report.implemented_edges.len(), 1);
        assert!(report.passes_compliance);
    }

    #[test]
    fn preflight_forbidden_dependency_detects_violation() {
        use crate::model::{Confidence, FileType};
        use crate::policy::ArchRule;

        let mut graph = GrapheniumGraph::new();
        let rules = vec![ArchRule::ForbiddenDependency {
            from_pattern: "src/controllers/**".to_string(),
            to_pattern: "src/db/**".to_string(),
            reason: "Controllers must use services, not access DB directly".to_string(),
        }];

        graph.upsert_node(Node::new(
            "db_svc",
            "DatabaseConnection",
            FileType::Code,
            "src/db/connection.rs",
        ));

        let mut controller = Node::new(
            "auth_ctrl",
            "AuthController",
            FileType::Code,
            "src/controllers/auth.rs",
        );
        controller.plan_id = Some("plan-xyz".to_string());
        graph.upsert_node(controller);

        let mut edge = Edge::new(
            "auth_ctrl",
            "db_svc",
            "calls",
            Confidence::Extracted,
            "src/controllers/auth.rs",
        );
        edge.plan_id = Some("plan-xyz".to_string());
        graph.add_edge(edge);

        let report = validate_plan_preflight(&graph, "plan-xyz", &rules);
        assert!(!report.passes);
        assert_eq!(report.violations.len(), 1);
        assert!(report.violations[0].contains("violates forbidden dependency"));
    }
}
