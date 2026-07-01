use graphenium::harness::verify_plan;
use graphenium::model::graph::GrapheniumGraph;
use graphenium::model::{Edge, FileType, Node};

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
