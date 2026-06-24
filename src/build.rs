/// Graph construction from extraction results.
///
/// This is the "assemble" step of the pipeline: validated `ExtractionResult`
/// values are folded into a `GrapheniumGraph` using the model's upsert / add
/// semantics:
///
/// - **Nodes**: inserted with last-write-wins — semantic results intentionally
///   override AST results when the same node ID appears in both.
/// - **Edges**: dangling edges (where either endpoint is not yet in the graph)
///   are silently dropped.  This is the intended behaviour for calls to
///   external libraries or stdlib.
/// - **HyperEdges**: appended to the graph's side-car `Vec<HyperEdge>`.
use crate::model::{ExtractionResult, GrapheniumGraph};

// ── Public API ────────────────────────────────────────────────────────────────

/// Statistics emitted after a build, useful for logging and the report phase.
#[derive(Debug, Default, Clone)]
pub struct BuildStats {
    pub nodes_inserted: usize,
    /// Includes nodes overwritten by last-write-wins.
    pub nodes_overwritten: usize,
    pub edges_inserted: usize,
    pub edges_dropped_dangling: usize,
    pub hyperedges_added: usize,
}

/// Build a `GrapheniumGraph` from a single (already-validated) `ExtractionResult`.
///
/// Prefer `build_merged` when combining AST + semantic results.
pub fn build_from_extraction(result: &ExtractionResult) -> (GrapheniumGraph, BuildStats) {
    let mut graph = GrapheniumGraph::new();
    let mut stats = BuildStats::default();

    // ── Nodes ──────────────────────────────────────────────────────────────
    for node in &result.nodes {
        let already_exists = graph.contains_node(&node.id);
        graph.upsert_node(node.clone());
        if already_exists {
            stats.nodes_overwritten += 1;
        } else {
            stats.nodes_inserted += 1;
        }
    }

    // ── Edges ──────────────────────────────────────────────────────────────
    for edge in &result.edges {
        if graph.add_edge(edge.clone()) {
            stats.edges_inserted += 1;
        } else {
            stats.edges_dropped_dangling += 1;
        }
    }

    // ── HyperEdges ─────────────────────────────────────────────────────────
    graph.hyperedges.extend(result.hyperedges.iter().cloned());
    stats.hyperedges_added = result.hyperedges.len();

    (graph, stats)
}

/// Merge multiple `ExtractionResult` values (AST + semantic) and build a
/// single unified `GrapheniumGraph`.
///
/// Merging is done with `ExtractionResult::merge_all`, which concatenates
/// node and edge lists (deduplication happens via `upsert_node` during build).
/// Token counts are summed so the report phase can display LLM cost.
pub fn build_merged(
    results: impl IntoIterator<Item = ExtractionResult>,
) -> (GrapheniumGraph, BuildStats) {
    let combined = ExtractionResult::merge_all(results);
    build_from_extraction(&combined)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Confidence, Edge, FileType, HyperEdge, Node};

    fn node(id: &str) -> Node {
        Node::new(id, id, FileType::Code, "f.rs")
    }

    fn edge(src: &str, tgt: &str) -> Edge {
        Edge::extracted(src, tgt, "calls", "f.rs")
    }

    #[test]
    fn basic_build() {
        let mut r = ExtractionResult::new();
        r.nodes.push(node("a"));
        r.nodes.push(node("b"));
        r.edges.push(edge("a", "b"));

        let (graph, stats) = build_from_extraction(&r);

        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edge_count(), 1);
        assert_eq!(stats.nodes_inserted, 2);
        assert_eq!(stats.edges_inserted, 1);
        assert_eq!(stats.edges_dropped_dangling, 0);
    }

    #[test]
    fn dangling_edge_dropped() {
        let mut r = ExtractionResult::new();
        r.nodes.push(node("a"));
        // "b" never added -> dangling
        r.edges.push(edge("a", "b"));

        let (graph, stats) = build_from_extraction(&r);

        assert_eq!(graph.edge_count(), 0);
        assert_eq!(stats.edges_dropped_dangling, 1);
        assert_eq!(stats.edges_inserted, 0);
    }

    #[test]
    fn last_write_wins_for_duplicate_id() {
        let mut r = ExtractionResult::new();
        r.nodes.push(node("x"));

        let mut updated = node("x");
        updated.label = "XUpdated".into();
        r.nodes.push(updated);

        let (graph, stats) = build_from_extraction(&r);

        assert_eq!(graph.node_count(), 1);
        assert_eq!(stats.nodes_overwritten, 1);
        assert_eq!(graph.node_data("x").unwrap().label, "XUpdated");
    }

    #[test]
    fn build_merged_combines_results() {
        let mut r1 = ExtractionResult::new();
        r1.nodes.push(node("a"));
        r1.input_tokens = 100;

        let mut r2 = ExtractionResult::new();
        r2.nodes.push(node("b"));
        r2.edges.push(edge("a", "b"));
        r2.input_tokens = 200;

        let (graph, stats) = build_merged([r1, r2]);

        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edge_count(), 1);
        assert_eq!(stats.edges_inserted, 1);
    }

    #[test]
    fn semantic_overrides_ast() {
        // Simulate: AST emits a node, semantic pass emits an enriched version.
        let ast_node = node("foo");

        let mut semantic_node = node("foo");
        semantic_node.label = "FooSemantic".into();

        let mut r = ExtractionResult::new();
        r.nodes.push(ast_node);
        r.nodes.push(semantic_node); // semantic comes after AST

        let (graph, _) = build_from_extraction(&r);

        assert_eq!(graph.node_data("foo").unwrap().label, "FooSemantic");
    }

    #[test]
    fn hyperedges_added_to_graph() {
        let mut r = ExtractionResult::new();
        r.hyperedges.push(HyperEdge {
            id: "he1".into(),
            label: "triangle".into(),
            nodes: vec!["a".into(), "b".into(), "c".into()],
            relation: "related_to".into(),
            confidence: Confidence::Inferred,
            confidence_score: 0.5,
            source_file: "f.py".into(),
        });

        let (graph, stats) = build_from_extraction(&r);

        assert_eq!(graph.hyperedges.len(), 1);
        assert_eq!(stats.hyperedges_added, 1);
    }

    #[test]
    fn token_counts_summed_in_merged() {
        let mut r1 = ExtractionResult::new();
        r1.input_tokens = 500;
        r1.output_tokens = 100;

        let mut r2 = ExtractionResult::new();
        r2.input_tokens = 300;
        r2.output_tokens = 80;

        // build_merged returns graph + stats; token totals aren't in stats but
        // we can verify the merge works by building and checking the source result.
        let combined = ExtractionResult::merge_all([r1, r2]);
        assert_eq!(combined.input_tokens, 800);
        assert_eq!(combined.output_tokens, 180);
    }
}
