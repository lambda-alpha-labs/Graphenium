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

#[derive(Debug, Clone, serde::Serialize)]
pub struct PlanVerificationReport {
    pub plan_id: String,
    pub implemented_nodes: Vec<String>,
    pub missing_nodes: Vec<String>,
    pub unplanned_modified_files: Vec<String>,
    pub passes_compliance: bool,
}

/// Verify that a planning workspace's declared changes match the real graph.
pub fn verify_plan(graph: &GrapheniumGraph, plan_id: &str) -> PlanVerificationReport {
    let mut implemented_nodes = Vec::new();
    let mut missing_nodes = Vec::new();
    let mut planned_files = std::collections::HashSet::new();

    let planned_nodes: Vec<&Node> = graph
        .nodes()
        .filter(|n| n.plan_id.as_deref() == Some(plan_id))
        .collect();

    for p_node in &planned_nodes {
        planned_files.insert(p_node.source_file.clone());

        let has_real_impl = graph
            .nodes()
            .any(|n| n.label == p_node.label && n.id != p_node.id && n.plan_id.is_none());

        if has_real_impl {
            implemented_nodes.push(p_node.label.clone());
        } else {
            missing_nodes.push(p_node.label.clone());
        }
    }

    let actual_files: std::collections::HashSet<String> = graph
        .nodes()
        .filter(|n| n.plan_id.is_none())
        .map(|n| n.source_file.clone())
        .collect();

    let mut unplanned_modified_files: Vec<String> = actual_files
        .difference(&planned_files)
        .filter(|f| !crate::serve::traversal::is_test_like_path(f))
        .cloned()
        .collect();
    unplanned_modified_files.sort();
    unplanned_modified_files.truncate(20);

    let passes_compliance = missing_nodes.is_empty() && unplanned_modified_files.is_empty();

    PlanVerificationReport {
        plan_id: plan_id.to_string(),
        implemented_nodes,
        missing_nodes,
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
