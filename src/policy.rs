//! Policy-based gates for graph trust quality.
//!
//! Policies are simple rules checked after graph generation or during CI.
//! They gate on metrics like resolution coverage, ambiguous edge counts,
//! evidence staleness, and community drift.
//!
//! Architecture policy rules (`ArchRule`) declare structural dependency
//! boundaries evaluated during pre-flight plan validation.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

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

/// Declarative architecture constraint for pre-flight plan validation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ArchRule {
    /// Prevent any edge (imports, calls) from matching `from_pattern` to `to_pattern`.
    /// Patterns use glob-style paths (e.g. from `src/routes/**` to `src/db/**`).
    ForbiddenDependency {
        from_pattern: String,
        to_pattern: String,
        reason: String,
    },
    /// Enforce a strict hierarchy where layer[i] can only call layer[i+1]..layer[n],
    /// never backwards. E.g. `["src/controllers", "src/services", "src/repositories"]`.
    StrictLayering {
        layers: Vec<String>,
        reason: String,
    },
    /// Banned symbol or namespace (e.g. direct usage of legacy raw SQL modules).
    BannedSymbol {
        symbol_label: String,
        reason: String,
    },
}

/// Repository architecture policy loaded from `.graphenium/policy.json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ArchPolicyConfig {
    #[serde(default)]
    pub rules: Vec<ArchRule>,
}

impl ArchPolicyConfig {
    /// Path to the policy file inside a repository root.
    pub fn policy_path(project_root: &Path) -> PathBuf {
        project_root.join(".graphenium").join("policy.json")
    }

    /// Load architecture policy from disk. Returns defaults when the file is missing.
    pub fn load_from_file(path: &Path) -> Result<Self, crate::GrapheniumError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)?;
        let config: Self = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// Load policy from the standard location under `project_root`.
    pub fn load_for_project(project_root: &Path) -> Result<Self, crate::GrapheniumError> {
        Self::load_from_file(&Self::policy_path(project_root))
    }
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
            let passed = graph.is_ast_only() || call_pct >= threshold;
            let actual = if graph.is_ast_only() { 100.0 } else { call_pct };
            PolicyResult {
                passed,
                actual,
                threshold,
                message: if graph.is_ast_only() {
                    "Call resolution: [SKIPPED] (AST-only graph — run with semantic pass for call resolution) — PASS".to_string()
                } else {
                    format!(
                        "Call resolution: {:.0}% (threshold: {:.0}%) — {}",
                        call_pct,
                        threshold,
                        if passed { "PASS" } else { "FAIL" }
                    )
                },
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

    #[test]
    fn arch_policy_defaults_when_missing() {
        let dir = std::env::temp_dir().join(format!(
            "graphenium-policy-test-{}",
            std::process::id()
        ));
        let config = ArchPolicyConfig::load_from_file(&dir.join("policy.json")).unwrap();
        assert!(config.rules.is_empty());
    }

    #[test]
    fn arch_policy_roundtrip() {
        let json = r#"{
            "rules": [{
                "type": "forbidden_dependency",
                "from_pattern": "src/controllers/**",
                "to_pattern": "src/db/**",
                "reason": "Use services"
            }]
        }"#;
        let config: ArchPolicyConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.rules.len(), 1);
        assert!(matches!(
            &config.rules[0],
            ArchRule::ForbiddenDependency { .. }
        ));
    }
}
