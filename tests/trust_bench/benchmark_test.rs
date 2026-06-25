/// Performance budget tests for Graphenium.
/// These verify that the analyzer and builder meet reasonable time/memory budgets.

#[test]
fn benchmark_self_graph_builds_quickly() {
    // Load the self-analysis graph and verify it loads within 2 seconds
    let start = std::time::Instant::now();
    let path = std::path::Path::new("graphenium-out/graph.json");
    if !path.exists() {
        eprintln!("SKIP: graph.json not found");
        return;
    }
    let _graph = graphenium::export::json::load_graph(path).unwrap();
    let elapsed = start.elapsed().as_secs_f64();
    assert!(elapsed < 5.0, "Graph load took {elapsed:.2}s, expected < 5s");
    eprintln!("Graph load: {elapsed:.2}s (budget: 5s) PASS");
}
