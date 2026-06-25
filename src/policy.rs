//! Policy-based gates for graph trust quality.
//!
//! Policies are simple rules checked after graph generation or during CI.
//! They gate on metrics like resolution coverage, ambiguous edge counts,
//! evidence staleness, and community drift.

use crate::model::GrapheniumGraph;
use crate::trust::ResolutionReport;

/// A single gate policy.
#[derive(Debug, Clone, Copy)]
pub enum Policy {
    /// Minimum import resolution percentage.
    MinResolution(f64),
    /// Maximum number of ambiguous edges.
    MaxAmbiguous(usize),
    /// Maximum number of stale evidence spans.
    MaxStale(usize),
    /// Minimum community coherence (0.0–1.0).
    MinCoherence(f64),
    /// Maximum number of unresolved references.
    MaxUnresolved(usize),
    /// Minimum call-edge resolution percentage.
    MinCallResolution(f64),
}

/// Result of a single policy evaluation.
#[derive(Debug, Clone)]
pub struct PolicyResult {
    pub passed: bool,
    pub actual: f64,
    pub threshold: f64,
    pub message: String,
}

/// Default Graphenium policies for CI gates.
pub fn default_policies() -> Vec<Policy> {
    vec![
        Policy::MinResolution(80.0),
        Policy::MaxAmbiguous(10),
        Policy::MaxUnresolved(100),
        Policy::MinCallResolution(70.0),
    ]
}

/// Evaluate a list of policies against a graph and resolution report.
pub fn evaluate_policies(
    graph: &GrapheniumGraph,
    report: &ResolutionReport,
    policies: &[Policy],
) -> Vec<PolicyResult> {
    let mut results = Vec::new();

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

    for policy in policies {
        let result = evaluate_one(*policy, report, graph, import_pct, call_pct);
        results.push(result);
    }
    results
}

fn evaluate_one(
    policy: Policy,
    report: &ResolutionReport,
    graph: &GrapheniumGraph,
    import_pct: f64,
    call_pct: f64,
) -> PolicyResult {
    match policy {
        Policy::MinResolution(threshold) => {
            let passed = import_pct >= threshold;
            PolicyResult {
                passed,
                actual: import_pct,
                threshold,
                message: format!(
                    "Import resolution: {:.0}% (threshold: {:.0}%) — {}",
                    import_pct,
                    threshold,
                    if passed { "PASS" } else { "FAIL" }
                ),
            }
        }
        Policy::MaxAmbiguous(threshold) => {
            let actual = report.ambiguous_edges as f64;
            let passed = actual <= threshold as f64;
            PolicyResult {
                passed,
                actual,
                threshold: threshold as f64,
                message: format!(
                    "Ambiguous edges: {} (max: {}) — {}",
                    report.ambiguous_edges,
                    threshold,
                    if passed { "PASS" } else { "FAIL" }
                ),
            }
        }
        Policy::MaxUnresolved(threshold) => {
            let actual = report.unresolved_refs as f64;
            let passed = actual <= threshold as f64;
            PolicyResult {
                passed,
                actual,
                threshold: threshold as f64,
                message: format!(
                    "Unresolved references: {} (max: {}) — {}",
                    report.unresolved_refs,
                    threshold,
                    if passed { "PASS" } else { "FAIL" }
                ),
            }
        }
        Policy::MinCallResolution(threshold) => {
            let passed = call_pct >= threshold;
            PolicyResult {
                passed,
                actual: call_pct,
                threshold,
                message: format!(
                    "Call resolution: {:.0}% (threshold: {:.0}%) — {}",
                    call_pct,
                    threshold,
                    if passed { "PASS" } else { "FAIL" }
                ),
            }
        }
        Policy::MaxStale(threshold) => {
            let actual = report.evidence_stale as f64;
            let passed = actual <= threshold as f64;
            PolicyResult {
                passed,
                actual,
                threshold: threshold as f64,
                message: format!(
                    "Stale evidence: {} (max: {}) — {}",
                    report.evidence_stale,
                    threshold,
                    if passed { "PASS" } else { "FAIL" }
                ),
            }
        }
        Policy::MinCoherence(threshold) => {
            let total_edges = graph.edge_count().max(1) as f64;
            let unique_communities = graph
                .nodes()
                .filter_map(|n| n.community)
                .collect::<std::collections::BTreeSet<_>>()
                .len() as f64;
            let actual = 1.0 - (unique_communities / total_edges);
            let passed = actual >= threshold;
            PolicyResult {
                passed,
                actual,
                threshold,
                message: format!(
                    "Community coherence: {:.2} (threshold: {:.2}) — {}",
                    actual,
                    threshold,
                    if passed { "PASS" } else { "FAIL" }
                ),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::GrapheniumGraph;
    use crate::trust::ResolutionReport;

    #[test]
    fn min_resolution_passes() {
        let report = ResolutionReport {
            total_import_edges: 100,
            resolved_imports: 90,
            ..Default::default()
        };
        let graph = GrapheniumGraph::new();
        let results = evaluate_policies(&graph, &report, &[Policy::MinResolution(85.0)]);
        assert!(results[0].passed);
    }

    #[test]
    fn max_ambiguous_fails() {
        let report = ResolutionReport {
            ambiguous_edges: 20,
            ..Default::default()
        };
        let graph = GrapheniumGraph::new();
        let results = evaluate_policies(&graph, &report, &[Policy::MaxAmbiguous(5)]);
        assert!(!results[0].passed);
    }

    #[test]
    fn default_policies_are_non_empty() {
        let p = default_policies();
        assert!(!p.is_empty());
    }
}
