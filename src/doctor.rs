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

    // 3. Tree-sitter languages
    check_tree_sitter_langs();

    // 4. API keys
    check_api_keys();

    // 5. Graph quality (if loaded)
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
