//! Integration tests for the pre-loaded Datalog Standard Library.

use graphenium::analyze::query::{run_datalog_query, stdlib_rules, DatalogProgram};
use graphenium::model::{Edge, FileType, GrapheniumGraph, Node};

fn chain_graph() -> GrapheniumGraph {
    let mut graph = GrapheniumGraph::new();
    graph.upsert_node(Node::new("a", "A", FileType::Code, "a.rs"));
    graph.upsert_node(Node::new("b", "B", FileType::Code, "b.rs"));
    graph.upsert_node(Node::new("c", "C", FileType::Code, "c.rs"));
    graph.add_edge(Edge::extracted("a", "b", "calls", "a.rs"));
    graph.add_edge(Edge::extracted("b", "c", "calls", "b.rs"));
    graph
}

#[test]
fn test_stdlib_parses_successfully() {
    let rules = stdlib_rules();
    assert!(
        !rules.is_empty(),
        "stdlib should contain transitive and topological rules"
    );
}

#[test]
fn test_stdlib_transitive_calls() {
    let graph = chain_graph();
    let result = run_datalog_query(&graph, r#"?- calls_transitive("a", X)."#, 1000).unwrap();

    assert!(
        result.contains("b"),
        "Should find direct callee b: {result}"
    );
    assert!(
        result.contains("c"),
        "Should find transitive callee c: {result}"
    );
}

#[test]
fn test_stdlib_depends_transitive() {
    let graph = chain_graph();
    let result = run_datalog_query(&graph, r#"?- depends_transitive("a", X)."#, 1000).unwrap();

    assert!(result.contains("b"), "Should depend on b: {result}");
    assert!(result.contains("c"), "Should depend on c: {result}");
}

#[test]
fn test_stdlib_same_community() {
    let mut graph = GrapheniumGraph::new();
    let mut a = Node::new("x1", "X1", FileType::Code, "mod.rs");
    a.community = Some(3);
    let mut b = Node::new("x2", "X2", FileType::Code, "mod.rs");
    b.community = Some(3);
    let mut c = Node::new("y1", "Y1", FileType::Code, "other.rs");
    c.community = Some(7);
    graph.upsert_node(a);
    graph.upsert_node(b);
    graph.upsert_node(c);

    let result = run_datalog_query(&graph, r#"?- same_community("x1", X)."#, 1000).unwrap();
    assert!(
        result.contains("x2"),
        "x1 and x2 share community 3: {result}"
    );
    assert!(
        !result.contains("y1"),
        "x1 and y1 are in different communities: {result}"
    );
}

#[test]
fn test_stdlib_is_orphan() {
    let mut graph = chain_graph();
    graph.upsert_node(Node::new("lonely", "Lonely", FileType::Code, "lonely.rs"));

    let result = run_datalog_query(&graph, r#"?- is_orphan(X)."#, 1000).unwrap();
    assert!(
        result.contains("lonely"),
        "lonely node has no edges and should be orphan: {result}"
    );
    assert!(
        !result.contains("b"),
        "b is connected and should not be orphan: {result}"
    );
}

#[test]
fn test_stdlib_circular_dependency() {
    let mut graph = GrapheniumGraph::new();
    graph.upsert_node(Node::new("p", "P", FileType::Code, "p.rs"));
    graph.upsert_node(Node::new("q", "Q", FileType::Code, "q.rs"));
    graph.add_edge(Edge::extracted("p", "q", "calls", "p.rs"));
    graph.add_edge(Edge::extracted("q", "p", "calls", "q.rs"));

    let result = run_datalog_query(&graph, r#"?- circular_dependency(X, Y)."#, 1000).unwrap();
    assert!(
        result.contains("p"),
        "Should detect cycle involving p: {result}"
    );
    assert!(
        result.contains("q"),
        "Should detect cycle involving q: {result}"
    );
}

#[test]
fn test_stdlib_bypasses_layer() {
    let mut graph = GrapheniumGraph::new();
    graph.upsert_node(Node::new("ctrl", "Controller", FileType::Code, "ctrl.rs"));
    graph.upsert_node(Node::new("svc", "Service", FileType::Code, "svc.rs"));
    graph.upsert_node(Node::new("repo", "Repository", FileType::Code, "repo.rs"));
    graph.add_edge(Edge::extracted("ctrl", "repo", "calls", "ctrl.rs"));

    let result =
        run_datalog_query(&graph, r#"?- bypasses_layer("ctrl", "svc", "repo")."#, 1000).unwrap();
    assert!(
        !result.contains("no results"),
        "ctrl bypasses svc to reach repo: {result}"
    );
}

#[test]
fn test_merge_stdlib_prepends_rules() {
    let mut program = DatalogProgram::default();
    program.rules.push(graphenium::analyze::query::Rule {
        head: graphenium::analyze::query::Atom {
            name: "user_rule".to_string(),
            terms: vec![],
            negated: false,
        },
        body: vec![],
    });

    let stdlib_len = stdlib_rules().len();
    program.merge_stdlib(stdlib_rules().as_ref().clone());
    assert_eq!(program.rules.len(), stdlib_len + 1);
    assert_ne!(program.rules[0].head.name, "user_rule");
    assert_eq!(program.rules[stdlib_len].head.name, "user_rule");
}
