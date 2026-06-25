//! Trust harness: validates claim quality and evidence freshness in CI.
//!
//! Use this to gate CI pipelines on trust quality:
//!   gm check --min-resolution 90 --max-stale 5

use crate::model::GrapheniumGraph;
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
///
/// Returns a `TrustCheckResult` with pass/fail based on thresholds.
pub fn check_resolution_quality(
    graph: &GrapheniumGraph,
    report: &ResolutionReport,
    min_resolution_pct: f64,
    max_ambiguous: usize,
) -> TrustCheckResult {
    let mut result = TrustCheckResult::default();
    let mut details = Vec::new();

    // Resolution coverage
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

    // Combined resolution percentage (weighted average)
    let total_relevant = report.total_import_edges
        + report.total_call_edges
        + report.total_method_edges;
    let total_resolved = report.resolved_imports
        + report.resolved_calls
        + report.resolved_methods;
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
    details.push(format!(
        "Call resolution: {:.0}% ({}/{})",
        call_pct, report.resolved_calls, report.total_call_edges
    ));
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
        assert!(
            result.details.iter().any(|d| d.contains("FAIL"))
        );
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
}
