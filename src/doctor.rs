//! `gm doctor` — diagnostic command for installation health.

use std::path::Path;

use crate::export::json;
use crate::model::GrapheniumGraph;

/// Run all doctor checks and print a summary.
pub fn run_doctor(graph_path: Option<&Path>) {
    println!("Graphenium Doctor");
    println!("=================");
    println!();

    // 1. Binary on PATH
    check_binary();

    // 2. Graph file
    let graph = check_graph(graph_path);

    // 3. Graph metadata (schema, modes, languages)
    if let Some(ref g) = graph {
        check_graph_metadata(g);
    }

    // 4. Tree-sitter languages
    check_tree_sitter_langs();

    // 5. API keys
    check_api_keys();

    // 6. Graph quality (if loaded)
    if let Some(ref g) = graph {
        check_graph_quality(g);
    }
}

fn check_binary() {
    print!("  gm binary .................... ");
    match std::env::current_exe() {
        Ok(path) => {
            println!("OK ({})", path.display());
        }
        Err(e) => {
            println!("FAIL ({e})");
        }
    }
}

fn check_graph(graph_path: Option<&Path>) -> Option<GrapheniumGraph> {
    let path = graph_path.unwrap_or_else(|| Path::new("graphenium-out/graph.json"));
    print!("  graph file ({}) ... ", path.display());
    if !path.exists() {
        println!("MISSING — run `gm run . --no-semantic --no-viz` first");
        return None;
    }
    match json::load_graph(path) {
        Ok(graph) => {
            let community_count: usize =
                std::collections::BTreeSet::from_iter(graph.nodes().filter_map(|n| n.community))
                    .len();
            let ast_note = if graph.is_ast_only() {
                " (AST-only)"
            } else {
                " (semantic)"
            };
            println!(
                "OK — {} nodes, {} edges, {} communities{}",
                graph.node_count(),
                graph.edge_count(),
                community_count,
                ast_note
            );
            Some(graph)
        }
        Err(e) => {
            println!("BROKEN — {e}");
            None
        }
    }
}

fn check_tree_sitter_langs() {
    print!("  tree-sitter languages ........ ");
    let langs: &[&str] = &[
        #[cfg(feature = "lang-rust")]
        "rust",
        #[cfg(feature = "lang-python")]
        "python",
        #[cfg(feature = "lang-go")]
        "go",
        #[cfg(feature = "lang-js")]
        "javascript",
        #[cfg(feature = "lang-ts")]
        "typescript",
        #[cfg(feature = "lang-c")]
        "c",
        #[cfg(feature = "lang-cpp")]
        "cpp",
        #[cfg(feature = "lang-java")]
        "java",
        #[cfg(feature = "lang-csharp")]
        "csharp",
    ];
    if langs.is_empty() {
        println!("NONE — no language features enabled");
    } else {
        println!("{} ({})", langs.len(), langs.join(", "));
    }
}

fn check_api_keys() {
    let providers: &[(&str, &str)] = &[
        ("ANTHROPIC_API_KEY", "Anthropic"),
        ("OPENAI_API_KEY", "OpenAI"),
        ("DEEPSEEK_API_KEY", "DeepSeek"),
        ("OPENROUTER_API_KEY", "OpenRouter"),
    ];
    print!("  API keys ..................... ");
    let found: Vec<&str> = providers
        .iter()
        .filter(|(var, _)| std::env::var(var).map(|v| !v.is_empty()).unwrap_or(false))
        .map(|(_, name)| *name)
        .collect();
    if found.is_empty() {
        println!("NONE — semantic extraction unavailable");
    } else {
        println!("{} found ({})", found.len(), found.join(", "));
    }
}

fn check_graph_metadata(graph: &GrapheniumGraph) {
    if let Some(ref v) = graph.metadata.schema_version {
        print!("  graph schema .................. ");
        println!("{v}");
    }
    if let Some(ref v) = graph.metadata.graphenium_version {
        print!("  built by ...................... ");
        println!("gm {v}");
    }
    if let Some(ref v) = graph.metadata.created_at {
        print!("  created at .................... ");
        println!("{v}");
    }
    if let Some(ref modes) = graph.metadata.extraction_modes {
        print!("  extraction modes ............. ");
        println!("{}", modes.join(", "));
    }
    if let Some(ref langs) = graph.metadata.languages {
        print!("  detected languages ........... ");
        println!("{}", langs.join(", "));
    }
}

fn check_graph_quality(graph: &GrapheniumGraph) {
    print!("  graph quality ................ ");
    let total = graph.edge_count();
    if total == 0 {
        println!("EMPTY (0 edges)");
        return;
    }
    let extracted = graph
        .edges_iter()
        .filter(|e| e.confidence == crate::model::Confidence::Extracted)
        .count();
    let inferred = graph
        .edges_iter()
        .filter(|e| e.confidence == crate::model::Confidence::Inferred)
        .count();
    let ambiguous = total - extracted - inferred;
    println!(
        "{:.0}% extracted, {:.0}% inferred, {:.0}% ambiguous",
        (extracted as f64 / total as f64) * 100.0,
        (inferred as f64 / total as f64) * 100.0,
        (ambiguous as f64 / total as f64) * 100.0,
    );
}

// ── Sub-commands ──────────────────────────────────────────────────────────────

/// Show graph schema information (--schema flag).
pub fn show_schema(graph_path: Option<&Path>) {
    if let Some(graph) = check_graph(graph_path) {
        check_graph_metadata(&graph);
    }
}

/// Show resolution quality report (--resolution flag).
pub fn show_resolution(graph_path: Option<&Path>) {
    let graph = match check_graph(graph_path) {
        Some(g) => g,
        None => return,
    };

    let mut report = crate::trust::ResolutionReport::default();

    for edge in graph.edges_iter() {
        match edge.relation.as_str() {
            "imports" => {
                report.total_import_edges += 1;
                if edge.resolution_status.as_deref() == Some("resolved") {
                    report.resolved_imports += 1;
                }
                if edge.resolution_status.as_deref() == Some("unresolved") {
                    report.unresolved_refs += 1;
                }
            }
            "calls" => {
                report.total_call_edges += 1;
                if edge.resolution_status.as_deref() == Some("resolved") {
                    report.resolved_calls += 1;
                }
            }
            _ => {
                if edge.relation != "contains" && edge.relation != "method" {
                    report.total_method_edges += 1;
                    if edge.resolution_status.as_deref() == Some("resolved") {
                        report.resolved_methods += 1;
                    }
                }
            }
        }
        match edge.confidence {
            crate::model::Confidence::Extracted => {}
            crate::model::Confidence::Inferred => report.heuristic_edges += 1,
            crate::model::Confidence::Ambiguous => report.ambiguous_edges += 1,
        }
    }

    println!("{}", report.format());
}
