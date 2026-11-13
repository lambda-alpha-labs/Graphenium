//! Integration tests for the pre-loaded Datalog Standard Library.

use graphenium::analyze::query::{
    run_datalog_json, run_datalog_json_with_options, run_datalog_query, run_datalog_report,
    run_datalog_report_with_options, stdlib_rules, DatalogProgram,
};
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
fn test_inequality_between_variables() {
    let graph = chain_graph();
    let query = r#"
        different(X, Y) :- node(X, _, _, _, _), node(Y, _, _, _, _), X != Y.
        ?- different("a", Y).
    "#;
    let result = run_datalog_query(&graph, query, 1000).unwrap();
    assert!(result.contains("b"), "b differs from a: {result}");
    assert!(result.contains("c"), "c differs from a: {result}");
    assert!(!result.contains("`a`"), "a must be excluded: {result}");
}

#[test]
fn test_inequality_against_string_literal() {
    let graph = chain_graph();
    let query = r#"
        not_a(X) :- node(X, _, _, _, _), X != "a".
        ?- not_a(X).
    "#;
    let result = run_datalog_query(&graph, query, 1000).unwrap();
    assert!(result.contains("b") && result.contains("c"), "{result}");
    assert!(!result.contains("`a`"), "a must be excluded: {result}");
}

#[test]
fn test_ungrounded_inequality_is_rejected() {
    let graph = chain_graph();
    let query = r#"
        bad(X) :- X != "a".
        ?- bad(X).
    "#;
    let err = run_datalog_query(&graph, query, 1000).unwrap_err();
    assert!(
        err.contains("only in an inequality"),
        "expected grounding error, got: {err}"
    );
}

#[test]
fn test_multiple_queries_share_one_edb() {
    let graph = chain_graph();
    let query = r#"
        ?- [direct] calls("a", X, _).
        ?- [transitive] calls_transitive("a", Y).
        ?- [missing] calls("c", Z, _).
    "#;
    let report = run_datalog_report(&graph, query, 1000).unwrap();
    assert_eq!(report.schema_version, "datalog.v1");
    assert_eq!(report.queries.len(), 3);
    assert_eq!(report.queries[0].name.as_deref(), Some("direct"));
    assert_eq!(report.queries[0].rows, vec![vec!["b".to_string()]]);
    assert_eq!(report.queries[1].name.as_deref(), Some("transitive"));
    assert_eq!(report.queries[1].row_count, 2);
    assert_eq!(report.queries[2].row_count, 0);
    assert!(report.edb_facts > 0);
}

#[test]
fn test_datalog_json_is_valid_for_empty_results() {
    let graph = chain_graph();
    let json = run_datalog_json(&graph, r#"?- calls("zzz", X, _)."#, 1000).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["schema_version"], "datalog.v1");
    assert_eq!(parsed["queries"][0]["row_count"], 0);
    assert_eq!(parsed["queries"][0]["rows"], serde_json::json!([]));
    assert!(parsed["queries"][0]["steps_used"].is_number());
}

#[test]
fn test_show_derived_is_opt_in_and_includes_idb_relations() {
    let graph = chain_graph();
    let query = r#"
        e(A, B) :- calls(A, B, _).
        ?- [missing_reverse] e(A, B), not e(B, A).
    "#;

    // Default remains goal-only.
    let report = run_datalog_report(&graph, query, 1000).unwrap();
    assert!(
        report.derived.is_empty(),
        "goal-only output must not include derived relations"
    );
    let json: serde_json::Value =
        serde_json::from_str(&run_datalog_json(&graph, query, 1000).unwrap()).unwrap();
    assert!(json.get("derived").is_none(), "{json}");

    let report = run_datalog_report_with_options(&graph, query, 1000, true).unwrap();
    let relation = report
        .derived
        .iter()
        .find(|relation| relation.name == "e")
        .expect("e should be materialized");
    assert_eq!(relation.arity, 2);
    assert_eq!(relation.row_count, 2);
    assert_eq!(
        relation.rows,
        vec![
            vec!["a".to_string(), "b".to_string()],
            vec!["b".to_string(), "c".to_string()],
        ]
    );

    let json: serde_json::Value =
        serde_json::from_str(&run_datalog_json_with_options(&graph, query, 1000, true).unwrap())
            .unwrap();
    assert_eq!(json["schema_version"], "datalog.v1");
    assert_eq!(json["derived"][0]["name"], "e");
    assert_eq!(json["derived"][0]["row_count"], 2);
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

#[test]
fn test_stratified_negation_on_200k_edges_is_fast() {
    use std::time::Instant;

    const EDGES: usize = 200_000;
    const NODES: usize = 633;
    let mut graph = GrapheniumGraph::new();
    for i in 0..NODES {
        let id = format!("n{i}");
        graph.upsert_node(Node::new(&id, &id, FileType::Code, "big.rs"));
    }
    let mut inserted = 0usize;
    'outer: for i in 0..NODES {
        for j in (i + 1)..NODES {
            graph.add_edge(Edge::extracted(
                &format!("n{i}"),
                &format!("n{j}"),
                "calls",
                "big.rs",
            ));
            inserted += 1;
            if inserted == EDGES {
                break 'outer;
            }
        }
    }
    assert_eq!(inserted, EDGES);

    let query = r#"
        e(A, B) :- calls(A, B, _).
        ?- [missing] e(A, B), not e(B, A).
    "#;

    let start = Instant::now();
    let report = run_datalog_report(&graph, query, 10_000).unwrap();
    let elapsed = start.elapsed();

    assert_eq!(report.queries.len(), 1);
    assert_eq!(
        report.queries[0].row_count, EDGES,
        "every forward edge lacks a reverse edge"
    );
    // Release builds must meet the spec's 1s budget. Debug builds are ~4x
    // slower, so they use a relaxed regression bound while still catching the
    // old >300s linear-scan behaviour.
    let budget_ms = if cfg!(debug_assertions) {
        10_000
    } else {
        1_000
    };
    assert!(
        elapsed.as_millis() < budget_ms,
        "stratified negation over {EDGES} edges took {elapsed:?} (budget {budget_ms}ms)"
    );
}

#[test]
fn test_unstratifiable_negation_is_rejected() {
    let graph = chain_graph();
    let query = r#"
        p(X) :- node(X, _, _, _, _), not p(X).
        ?- p(X).
    "#;
    let err = run_datalog_query(&graph, query, 1000).unwrap_err();
    assert!(
        err.contains("unstratifiable negation"),
        "expected an explicit stratification error, got: {err}"
    );
}

#[test]
fn test_negation_requires_bound_variables() {
    let graph = chain_graph();
    let query = r#"
        bad(X) :- node(X, _, _, _, _), not calls(Y, X, _).
        ?- bad(X).
    "#;
    let err = run_datalog_query(&graph, query, 1000).unwrap_err();
    assert!(
        err.contains("range restriction"),
        "expected a range-restriction error, got: {err}"
    );
}

#[test]
fn test_inherits_hypothesis_is_queryable() {
    let src = br#"
structure EdgeAtlasCertificate where
  witness : Nat

theorem no_four_mubs_seed (h : EdgeAtlasCertificate) : 1 = 1 := by
  exact rfl

theorem downstream : 1 = 1 := by
  exact no_four_mubs_seed { witness := 0 }
"#;
    let ext = graphenium::extract::lean::extract(src, "Proof.lean");
    let (graph, _) = graphenium::build::build_merged([ext]);

    let result = run_datalog_query(
        &graph,
        r#"?- inherits_hypothesis("proof_no_four_mubs_seed", H)."#,
        1000,
    )
    .unwrap();
    assert!(
        result.to_ascii_lowercase().contains("edgeatlascertificate"),
        "seed should inherit its binder hypothesis: {result}"
    );

    let result = run_datalog_query(
        &graph,
        r#"?- inherits_hypothesis("proof_downstream", H)."#,
        1000,
    )
    .unwrap();
    assert!(
        result.to_ascii_lowercase().contains("edgeatlascertificate"),
        "downstream should inherit the hypothesis transitively: {result}"
    );
}
