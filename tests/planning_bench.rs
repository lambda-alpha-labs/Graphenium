use graphenium::harness::{validate_plan_preflight, verify_plan};
use graphenium::model::graph::GrapheniumGraph;
use graphenium::model::{Confidence, Edge, FileType, Node};
use graphenium::policy::ArchRule;

#[test]
fn test_planning_workspace_lifecycle() {
    let mut graph = GrapheniumGraph::new();

    // 1. Setup planned nodes — the baseline node is part of the plan too
    let mut existing_planned = Node::new(
        "existing_svc_planned",
        "ExistingService",
        FileType::Code,
        "src/service.rs",
    );
    existing_planned.plan_id = Some("plan-1".to_string());
    graph.upsert_node(existing_planned);
    // Also add a real version of the baseline node
    graph.upsert_node(Node::new(
        "existing_svc_real",
        "ExistingService",
        FileType::Code,
        "src/service.rs",
    ));

    // 2. Setup planned (virtual) nodes under workspace "plan-1"
    let mut p_node = Node::new(
        "planned_v2",
        "NewValidationService",
        FileType::Code,
        "src/validation.rs",
    );
    p_node.plan_id = Some("plan-1".to_string());
    graph.upsert_node(p_node);

    // 3. Verify compliance fails (missing node implementation)
    let report = verify_plan(&graph, "plan-1");
    assert!(
        !report.passes_compliance,
        "Expected FAIL for unimplemented plan"
    );
    assert_eq!(
        report.missing_nodes,
        vec!["NewValidationService".to_string()]
    );

    // 4. Simulate real code implementation
    graph.upsert_node(Node::new(
        "real_v2",
        "NewValidationService",
        FileType::Code,
        "src/validation.rs",
    ));

    // 5. Verify compliance passes
    let report = verify_plan(&graph, "plan-1");
    assert!(
        report.passes_compliance,
        "Expected PASS after implementation"
    );
    assert!(report.missing_nodes.is_empty());
}

#[test]
fn test_preflight_forbidden_dependency() {
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

    assert!(!report.passes, "Expected pre-flight check to fail");
    assert_eq!(report.violations.len(), 1);
    assert!(report.violations[0].contains("violates forbidden dependency"));
}
