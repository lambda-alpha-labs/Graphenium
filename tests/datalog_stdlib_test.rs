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

#[test]
fn test_stdlib_aliased_container_call_detects_struct_field_overlap() {
    let mut graph = GrapheniumGraph::new();
    graph.upsert_node(Node::new(
        "caller",
        "init_caller",
        FileType::Code,
        "malloc.c",
    ));
    graph.upsert_node(Node::new(
        "callee",
        "tcache_free_init",
        FileType::Code,
        "malloc.c",
    ));
    graph.upsert_node(Node::new(
        "mp_struct",
        "malloc_par",
        FileType::Code,
        "malloc.c",
    ));
    graph.upsert_node(Node::new(
        "tmn_field",
        "tcache_bins",
        FileType::Code,
        "malloc.c",
    ));

    // mp_struct contains tmn_field
    graph.add_edge(Edge::extracted(
        "mp_struct",
        "tmn_field",
        "contains",
        "malloc.c",
    ));
    // caller calls callee and touches both mp_struct and tmn_field
    graph.add_edge(Edge::extracted("caller", "callee", "calls", "malloc.c"));
    graph.add_edge(Edge::extracted("caller", "mp_struct", "calls", "malloc.c"));
    graph.add_edge(Edge::extracted("caller", "tmn_field", "calls", "malloc.c"));

    let result = run_datalog_query(
        &graph,
        r#"?- aliased_container_call("caller", "callee", Container, Member)."#,
        1000,
    )
    .unwrap();
    assert!(
        result.contains("mp_struct"),
        "Should detect container: {result}"
    );
    assert!(
        result.contains("tmn_field"),
        "Should detect member: {result}"
    );
}

#[test]
fn test_stdlib_tainted_by_proof_gap_propagates_sorry() {
    let src = br#"
theorem helper_unfinished : 1 = 1 := by
  sorry

theorem main_theorem : 1 = 1 := by
  exact helper_unfinished
"#;
    let ext = graphenium::extract::lean::extract(src, "Proof.lean");
    let (graph, _) = graphenium::build::build_merged([ext]);

    let result = run_datalog_query(&graph, r#"?- tainted_by_proof_gap(X)."#, 1000).unwrap();
    assert!(
        result.contains("proof_helper_unfinished"),
        "Direct sorry theorem should be flagged: {result}"
    );
    assert!(
        result.contains("proof_main_theorem"),
        "Theorem depending on sorry lemma should be tainted: {result}"
    );
}
