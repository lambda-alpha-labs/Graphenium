//! Verification planner: produces structured verification plans for changed code.
//!
//! Given a set of changed nodes (from a diff or file-watch event), the planner
//! produces a prioritized plan:
//!   1. evidence spans for changed nodes
//!   2. direct extracted callers (high trust)
//!   3. direct dependents
//!   4. covering tests
//!   5. public entry points
//!   6. ambiguous or inferred edges to verify
//!   7. runtime hot paths if available

use crate::model::GrapheniumGraph;

/// A single step in a verification plan.
#[derive(Debug, Clone)]
pub struct VerificationStep {
    pub kind: StepKind,
    pub target: String,
    pub file: String,
    pub reason: String,
    pub confidence: String,
    pub priority: usize, // lower = higher priority
}

/// What kind of verification step this is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepKind {
    ReadSource,
    RunTest,
    InspectEdge,
    CheckHotPath,
    RiskGate,
}

/// A complete verification plan.
#[derive(Debug, Clone)]
pub struct VerificationPlan {
    pub changed_nodes: Vec<String>,
    pub must_read: Vec<VerificationStep>,
    pub tests_to_run: Vec<VerificationStep>,
    pub edges_to_verify: Vec<VerificationStep>,
    pub risk_gates: Vec<VerificationStep>,
}

/// Build a verification plan for a set of changed nodes.
///
/// Given changed node IDs, walks the graph to find:
/// - Callers (high-confidence extracted edges)
/// - Dependents (downstream consumers)
/// - Tests (edges with "tests" or "test" relation)
/// - Public entry points (nodes with "public" or no callers)
/// - Ambiguous/inferred edges
pub fn plan_verification(
    graph: &GrapheniumGraph,
    changed_nodes: &[String],
    risk_level: Option<&str>,
) -> VerificationPlan {
    let mut must_read = Vec::new();
    let mut tests_to_run = Vec::new();
    let mut edges_to_verify = Vec::new();
    let mut risk_gates = Vec::new();
    let _risk_level = risk_level.unwrap_or("normal");

    let changed_set: std::collections::HashSet<String> = changed_nodes.iter().cloned().collect();

    for changed_id in changed_nodes {
        // 1. Must-read: the changed node's own source file
        if let Some(node) = graph.node_data(changed_id) {
            must_read.push(VerificationStep {
                kind: StepKind::ReadSource,
                target: changed_id.clone(),
                file: node.source_file.clone(),
                reason: format!("changed symbol evidence span for {}", node.label),
                confidence: "Extracted".to_string(),
                priority: 1,
            });

            // 2. Direct extracted callers (edges going out)
            for edge in graph.edges_iter() {
                if edge.source == *changed_id {
                    let target_node = graph.node_data(&edge.target);
                    let target_file = target_node.map(|n| n.source_file.as_str()).unwrap_or("?");
                    if edge.confidence == crate::model::Confidence::Extracted {
                        must_read.push(VerificationStep {
                            kind: StepKind::ReadSource,
                            target: edge.target.clone(),
                            file: target_file.to_string(),
                            reason: format!(
                                "direct extracted caller: {} {} {}",
                                changed_id, edge.relation, edge.target
                            ),
                            confidence: "Extracted".to_string(),
                            priority: 2,
                        });
                    } else {
                        edges_to_verify.push(VerificationStep {
                            kind: StepKind::InspectEdge,
                            target: format!("{} {} {}", changed_id, edge.relation, edge.target),
                            file: edge.source_file.clone(),
                            reason: format!(
                                "inferred/ambiguous edge from {}: {} -> {}",
                                changed_id, edge.relation, edge.target
                            ),
                            confidence: format!("{:?}", edge.confidence),
                            priority: 6,
                        });
                    }
                }

                // 3. Direct dependents (edges coming in)
                if edge.target == *changed_id {
                    let source_node = graph.node_data(&edge.source);
                    if let Some(sn) = source_node {
                        must_read.push(VerificationStep {
                            kind: StepKind::ReadSource,
                            target: edge.source.clone(),
                            file: sn.source_file.clone(),
                            reason: format!(
                                "dependent: {} depends on {} via {}",
                                edge.source, changed_id, edge.relation
                            ),
                            confidence: "Extracted".to_string(),
                            priority: 3,
                        });
                    }
                }

                // 4. Covering tests
                if edge.relation == "tests" || edge.source.contains("test") {
                    if edge.source == *changed_id || edge.target == *changed_id {
                        let test_node = if edge.source == *changed_id {
                            &edge.target
                        } else {
                            &edge.source
                        };
                        tests_to_run.push(VerificationStep {
                            kind: StepKind::RunTest,
                            target: test_node.clone(),
                            file: edge.source_file.clone(),
                            reason: format!("test covers changed symbol {}", changed_id),
                            confidence: "Extracted".to_string(),
                            priority: 4,
                        });
                    }
                }
            }
        }
    }

    // 5. Risk gates for high-risk operations
    if changed_set.iter().any(|n| {
        n.contains("auth")
            || n.contains("security")
            || n.contains("payment")
            || n.contains("credentials")
    }) {
        risk_gates.push(VerificationStep {
            kind: StepKind::RiskGate,
            target: "security_review".to_string(),
            file: "".to_string(),
            reason: "changes touch security-sensitive code".to_string(),
            confidence: "Extracted".to_string(),
            priority: 7,
        });
    }

    VerificationPlan {
        changed_nodes: changed_nodes.to_vec(),
        must_read,
        tests_to_run,
        edges_to_verify,
        risk_gates,
    }
}

/// Format a verification plan as a human-readable string.
pub fn format_plan(plan: &VerificationPlan) -> String {
    let mut output = String::new();

    output.push_str(&format!(
        "## Verification Plan ({} changed)\n\n",
        plan.changed_nodes.len()
    ));

    if !plan.must_read.is_empty() {
        output.push_str("### Files to Read\n");
        for step in &plan.must_read {
            output.push_str(&format!(
                "  {}. {}  —  {}\n",
                step.priority, step.file, step.reason
            ));
        }
        output.push('\n');
    }

    if !plan.tests_to_run.is_empty() {
        output.push_str("### Tests to Run\n");
        for step in &plan.tests_to_run {
            output.push_str(&format!(
                "  {}. {} [{}]\n",
                step.priority, step.target, step.file
            ));
        }
        output.push('\n');
    }

    if !plan.edges_to_verify.is_empty() {
        output.push_str("### Edges to Inspect\n");
        for step in &plan.edges_to_verify {
            output.push_str(&format!(
                "  [{}] {} — {}\n",
                step.confidence, step.target, step.reason
            ));
        }
        output.push('\n');
    }

    if !plan.risk_gates.is_empty() {
        output.push_str("### Risk Gates\n");
        for step in &plan.risk_gates {
            output.push_str(&format!("  ⚠ {} — {}\n", step.target, step.reason));
        }
        output.push('\n');
    }

    output
}

/// Format a verification plan as a JSON-like string for MCP tools.
pub fn format_plan_json(plan: &VerificationPlan) -> String {
    let must_read_json: Vec<String> = plan
        .must_read
        .iter()
        .map(|s| {
            format!(
                r#"    {{"file": "{}", "reason": "{}", "confidence": "{}"}}"#,
                s.file, s.reason, s.confidence
            )
        })
        .collect();
    let tests_json: Vec<String> = plan
        .tests_to_run
        .iter()
        .map(|s| {
            format!(
                r#"    {{"command": "{}", "reason": "{}"}}"#,
                s.target, s.reason
            )
        })
        .collect();
    let edges_json: Vec<String> = plan
        .edges_to_verify
        .iter()
        .map(|s| format!(r#"    "{}""#, s.target))
        .collect();

    format!(
        r#"{{
  "changed_nodes": {:?},
  "must_read": [
{}
  ],
  "tests_to_run": [
{}
  ],
  "ambiguous_edges_to_verify": [
{}
  ],
  "risk_gates": [
{}
  ]
}}"#,
        plan.changed_nodes,
        must_read_json.join(",\n"),
        tests_json.join(",\n"),
        edges_json.join(",\n"),
        plan.risk_gates
            .iter()
            .map(|s| format!(r#"    "{}: {}""#, s.target, s.reason))
            .collect::<Vec<_>>()
            .join(",\n")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Confidence, Edge, FileType, Node};

    fn make_test_graph() -> GrapheniumGraph {
        let mut graph = GrapheniumGraph::new();
        graph.upsert_node(Node::new(
            "validate_token",
            "validate_token",
            FileType::Code,
            "src/auth/token.rs",
        ));
        graph.upsert_node(Node::new(
            "login_handler",
            "login_handler",
            FileType::Code,
            "src/auth/login.rs",
        ));
        graph.upsert_node(Node::new(
            "test_login",
            "test_login",
            FileType::Document,
            "tests/auth_test.rs",
        ));
        graph.add_edge(Edge::extracted(
            "login_handler",
            "validate_token",
            "calls",
            "src/auth/login.rs",
        ));
        graph.add_edge(Edge::extracted(
            "test_login",
            "login_handler",
            "tests",
            "tests/auth_test.rs",
        ));
        graph
    }

    #[test]
    fn plan_includes_must_read_for_changed_node() {
        let graph = make_test_graph();
        let plan = plan_verification(&graph, &["validate_token".to_string()], None);
        assert!(!plan.must_read.is_empty());
        assert!(plan.must_read[0].file.contains("token.rs"));
    }

    #[test]
    fn plan_includes_tests_when_available() {
        let graph = make_test_graph();
        let plan = plan_verification(&graph, &["login_handler".to_string()], None);
        assert!(!plan.tests_to_run.is_empty());
    }

    #[test]
    fn plan_includes_callers() {
        let graph = make_test_graph();
        let plan = plan_verification(&graph, &["validate_token".to_string()], None);
        assert!(plan.must_read.iter().any(|s| s.target == "login_handler"));
    }

    #[test]
    fn empty_plan_for_no_changes() {
        let graph = GrapheniumGraph::new();
        let plan = plan_verification(&graph, &[], None);
        assert!(plan.must_read.is_empty());
    }

    #[test]
    fn security_changes_add_risk_gate() {
        let graph = make_test_graph();
        let plan = plan_verification(&graph, &["auth_validate_token".to_string()], None);
        // "auth_validate_token" contains "auth" — risk gate triggered
        assert!(!plan.risk_gates.is_empty());
    }
}
