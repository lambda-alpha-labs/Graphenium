use std::path::{Path, PathBuf};
use std::process;

use clap::{Parser, Subcommand};

use graphenium::analyze;
use graphenium::build;
use graphenium::cache::Manifest;
use graphenium::cluster::{self, ClusterOptions};
use graphenium::detect::{self, DetectOptions};
use graphenium::export;
use graphenium::export::json::load_graph;
use graphenium::extract::{self, ExtractMode, ExtractOptions};
use graphenium::model::ExtractionResult;
use graphenium::ranking;
use graphenium::report::{self, ReportInput};
use graphenium::semantic::{self, AiProvider, SemanticOptions};
use graphenium::serve::traversal as serve_traversal;

// ── CLI definition ─────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "gm",
    about = "Graphenium (gm) — the elemental knowledge graph engine for your codebase",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the full pipeline on a directory
    Run {
        /// Directory to analyze (default: current directory)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Extraction mode: `deep` for aggressive inference (default: standard)
        #[arg(long)]
        mode: Option<String>,

        /// Re-extract only new or modified files (uses mtime manifest)
        #[arg(long)]
        update: bool,

        /// Skip LLM semantic extraction; use AST-only results
        #[arg(long)]
        no_semantic: bool,

        /// Skip HTML visualization generation
        #[arg(long)]
        no_viz: bool,

        /// AI provider: anthropic, openai, openrouter, deepseek, or openai-compatible
        #[arg(long, default_value = "anthropic")]
        provider: String,

        /// API base URL for openai-compatible provider
        #[arg(long)]
        api_base: Option<String>,

        /// Model to use (defaults to provider-specific default)
        #[arg(long)]
        model: Option<String>,

        /// API key (overrides the provider-specific env var)
        #[arg(long)]
        api_key: Option<String>,
    },

    /// Query the knowledge graph with keywords
    Query {
        /// Keywords or question to match against the graph
        question: String,

        /// Use depth-first search instead of the default BFS
        #[arg(long)]
        dfs: bool,

        /// Maximum output token budget (rough estimate)
        #[arg(long, default_value = "2000")]
        budget: usize,

        /// Path to graph.json produced by `gm run`
        #[arg(long, default_value = "graphenium-out/graph.json")]
        graph: PathBuf,

        /// Restrict results to nodes whose source path contains this fragment
        #[arg(long)]
        path_prefix: Option<String>,

        /// Exclude nodes whose source path contains this fragment
        #[arg(long)]
        exclude_path: Option<String>,

        /// Generated-like code filter: include, exclude, or only
        #[arg(long, default_value = "include")]
        generated_code_mode: String,

        /// AST-only tuning mode: auto, on, or off
        #[arg(long, default_value = "auto")]
        ast_only_tuning: String,
    },

    /// Start the MCP server for agent/tool integration (stdio JSON-RPC)
    Serve {
        /// Path to graph.json
        #[arg(long, default_value = "graphenium-out/graph.json")]
        graph: PathBuf,
    },

    /// Run diagnostic checks on the Graphenium installation
    Doctor {
        /// Optional path to graph.json
        #[arg(long)]
        graph: Option<PathBuf>,
    },

    /// Print MCP setup instructions for an AI assistant
    Setup {
        /// Target assistant: claude, cursor, codewhale
        target: String,

        /// Path to the gm binary (default: auto-detect)
        #[arg(long)]
        gm_path: Option<PathBuf>,

        /// Path to the graph.json file
        #[arg(long, default_value = "graphenium-out/graph.json")]
        graph: PathBuf,
    },

    /// Watch a directory for changes and auto-rebuild the graph
    Watch {
        /// Directory to watch (default: current directory)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Debounce interval in seconds before triggering a rebuild
        #[arg(long, default_value = "3.0")]
        debounce: f64,

        /// Enable incremental patching: only re-extract changed files (default: true)
        #[arg(long, default_value = "true")]
        incremental: bool,
    },
}

// ── Entry point ────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Run {
            path,
            mode,
            update,
            no_semantic,
            no_viz,
            provider,
            api_base,
            model,
            api_key,
        } => {
            cmd_run(
                path,
                mode,
                update,
                no_semantic,
                no_viz,
                provider,
                api_base,
                model,
                api_key,
            )
            .await
        }

        Commands::Query {
            question,
            dfs,
            budget,
            graph,
            path_prefix,
            exclude_path,
            generated_code_mode,
            ast_only_tuning,
        } => cmd_query(
            question,
            dfs,
            budget,
            graph,
            path_prefix,
            exclude_path,
            generated_code_mode,
            ast_only_tuning,
        ),

        Commands::Serve { graph } => graphenium::serve::serve(&graph).await,

        Commands::Doctor { graph } => {
            graphenium::doctor::run_doctor(graph.as_deref());
            Ok(())
        }

        Commands::Setup {
            target,
            gm_path,
            graph,
        } => cmd_setup(&target, gm_path, &graph),

        Commands::Watch {
            path,
            debounce,
            incremental,
        } => {
            match tokio::task::spawn_blocking(move || {
                graphenium::watch::watch(&path, debounce, incremental)
            })
            .await
            {
                Ok(result) => result,
                Err(e) => Err(graphenium::GrapheniumError::Watch(format!("{e}"))),
            }
        }
    };

    if let Err(e) = result {
        eprintln!("[graphenium] error: {e}");
        process::exit(1);
    }
}

// ── `run` command ──────────────────────────────────────────────────────────────

async fn cmd_run(
    path: PathBuf,
    mode: Option<String>,
    update: bool,
    no_semantic: bool,
    no_viz: bool,
    provider: String,
    api_base: Option<String>,
    model: Option<String>,
    api_key: Option<String>,
) -> graphenium::Result<()> {
    let root = path.canonicalize().unwrap_or(path);
    let out_dir = root.join("graphenium-out");
    let cache_dir = out_dir.join("cache");
    let manifest_path = out_dir.join("manifest.json");

    let title = root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Knowledge Graph")
        .to_string();

    let extract_mode = match mode.as_deref() {
        Some("deep") => ExtractMode::Deep,
        _ => ExtractMode::Standard,
    };

    // ── 1. Detect files ────────────────────────────────────────────────────────
    eprintln!("[graphenium] Detecting files in: {}", root.display());

    let (all_files, corpus_warnings) = detect::detect(&root, &DetectOptions::default())?;

    for w in &corpus_warnings {
        eprintln!("[graphenium] warn: {w}");
    }

    eprintln!("[graphenium] Found {} file(s)", all_files.len());

    // ── 2. Incremental filtering (--update) ────────────────────────────────────
    let mut manifest = if update {
        Manifest::load(&manifest_path)
    } else {
        Manifest::new()
    };

    let files_to_process: Vec<_> = if update {
        let changed: Vec<_> = all_files
            .iter()
            .filter(|f| manifest.is_changed(&f.path))
            .cloned()
            .collect();
        eprintln!(
            "[graphenium] Incremental: {}/{} file(s) changed",
            changed.len(),
            all_files.len()
        );
        changed
    } else {
        all_files.clone()
    };

    if files_to_process.is_empty() {
        eprintln!("[graphenium] Nothing to do — all files are up to date.");
        return Ok(());
    }

    // ── 3. AST extraction ──────────────────────────────────────────────────────
    eprintln!("[graphenium] Extracting AST structure...");
    let ast_opts = ExtractOptions {
        mode: extract_mode.clone(),
    };
    let ast_result = extract::extract_all(&files_to_process, &ast_opts);
    eprintln!(
        "[graphenium] AST: {} nodes, {} edges",
        ast_result.nodes.len(),
        ast_result.edges.len()
    );

    // ── 4. Semantic extraction ─────────────────────────────────────────────────
    let (semantic_result, ast_only_graph) = if no_semantic {
        (ExtractionResult::new(), true)
    } else {
        let parsed_provider: AiProvider = match provider.parse() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("[graphenium] warn: {e}");
                return Ok(()); // bail out of cmd_run gracefully
            }
        };

        let provider = if let (AiProvider::OpenAICompatible { .. }, Some(base)) =
            (&parsed_provider, api_base.as_deref())
        {
            AiProvider::OpenAICompatible {
                base_url: base.to_string(),
            }
        } else {
            parsed_provider
        };

        let key = match api_key {
            Some(ref k) if !k.is_empty() => k.clone(),
            _ => std::env::var(provider.env_var_name()).unwrap_or_default(),
        };

        if key.is_empty() {
            eprintln!(
                "[graphenium] warn: no API key found for provider {}; skipping semantic extraction.\n\
                 Set {} or pass --api-key.",
                provider,
                provider.env_var_name()
            );
            (ExtractionResult::new(), true)
        } else {
            eprintln!(
                "[graphenium] Running semantic extraction via {} (LLM)...",
                provider
            );
            let sem_opts = SemanticOptions {
                provider,
                api_key: key,
                model: model.unwrap_or_default(),
                mode: extract_mode.clone(),
                ..SemanticOptions::default()
            };
            let result =
                semantic::extract_semantic(&files_to_process, &sem_opts, &cache_dir).await?;
            eprintln!(
                "[graphenium] Semantic: {} nodes, {} edges \
                 (tokens in={}, out={})",
                result.nodes.len(),
                result.edges.len(),
                result.input_tokens,
                result.output_tokens
            );
            (result, false)
        }
    };

    // Save token counts before consuming the results.
    let total_input_tokens = ast_result.input_tokens + semantic_result.input_tokens;
    let total_output_tokens = ast_result.output_tokens + semantic_result.output_tokens;

    // ── 5. Build graph ─────────────────────────────────────────────────────────
    eprintln!("[graphenium] Building graph...");
    let (mut graph, build_stats) = build::build_merged([ast_result, semantic_result]);
    graph.set_ast_only(ast_only_graph);
    eprintln!(
        "[graphenium] Graph: {} nodes, {} edges ({} dangling, {} hyperedges)",
        graph.node_count(),
        graph.edge_count(),
        build_stats.edges_dropped_dangling,
        graph.hyperedges.len()
    );
    if let Some(msg) = label_collision_report(&graph) {
        eprintln!("[graphenium] {msg}");
    }

    // ── 6. Cluster ─────────────────────────────────────────────────────────────
    eprintln!("[graphenium] Clustering communities...");
    let community_stats = cluster::cluster(&mut graph, &ClusterOptions::default());
    eprintln!(
        "[graphenium] {} communities detected",
        community_stats.len()
    );

    // ── 7. Analyze ─────────────────────────────────────────────────────────────
    let analysis = analyze::analyze(&graph, &community_stats);
    eprintln!(
        "[graphenium] Analysis: {} god nodes, {} surprising connections, {} questions",
        analysis.god_nodes.len(),
        analysis.surprising.len(),
        analysis.questions.len()
    );

    // ── 8. Export ──────────────────────────────────────────────────────────────
    std::fs::create_dir_all(&out_dir)?;

    if no_viz {
        let json_path = out_dir.join("graph.json");
        std::fs::write(&json_path, export::json::to_json(&graph)?)?;
        eprintln!("[graphenium] Wrote: {}", json_path.display());
    } else {
        let paths = export::export(&graph, &out_dir, &title)?;
        eprintln!("[graphenium] Wrote: {}", paths.json.display());
        eprintln!("[graphenium] Wrote: {}", paths.html.display());
    }

    // ── 9. Report ──────────────────────────────────────────────────────────────
    let report_path = report::write_report(
        &ReportInput {
            graph: &graph,
            community_stats: &community_stats,
            analysis: &analysis,
            corpus_warnings: &corpus_warnings,
            input_tokens: total_input_tokens,
            output_tokens: total_output_tokens,
            title,
        },
        &out_dir,
    )?;
    eprintln!("[graphenium] Wrote: {}", report_path.display());

    // ── 10. Update manifest ────────────────────────────────────────────────────
    let all_paths: Vec<_> = all_files.iter().map(|f| f.path.clone()).collect();
    manifest.prune(&all_paths);
    for f in &files_to_process {
        manifest.update(&f.path);
    }
    manifest.save(&manifest_path)?;

    eprintln!("[graphenium] Done. Open {} to explore.", out_dir.display());
    Ok(())
}

// ── `query` command ────────────────────────────────────────────────────────────

fn cmd_query(
    question: String,
    dfs: bool,
    budget: usize,
    graph_path: PathBuf,
    path_prefix: Option<String>,
    exclude_path: Option<String>,
    generated_code_mode: String,
    ast_only_tuning: String,
) -> graphenium::Result<()> {
    let graph = load_graph(&graph_path)?;

    if graph.node_count() == 0 {
        eprintln!("[graphenium] Graph is empty. Run `gm run <path>` first.");
        return Ok(());
    }

    let ast_only_tuning = match ast_only_tuning.trim().to_lowercase().as_str() {
        "auto" => graph.is_ast_only(),
        "on" => true,
        "off" => false,
        other => {
            eprintln!(
                "[graphenium] Unknown ast-only tuning mode '{other}'. Expected auto, on, or off."
            );
            return Ok(());
        }
    };
    let generated_code_mode_value =
        if generated_code_mode.eq_ignore_ascii_case("include") && ast_only_tuning {
            "exclude"
        } else {
            generated_code_mode.as_str()
        };
    let generated_code_mode =
        match serve_traversal::parse_generated_code_mode(Some(generated_code_mode_value)) {
            Ok(mode) => mode,
            Err(err) => {
                eprintln!("[graphenium] {err}");
                return Ok(());
            }
        };
    let scoped = serve_traversal::filtered_node_ids(
        &graph,
        path_prefix.as_deref(),
        exclude_path.as_deref(),
        None,
        generated_code_mode,
    );
    let ranked = ranking::score_query_nodes_detailed_in_scope(&graph, &question, scoped.as_ref());
    let seeds: Vec<String> = ranked.iter().take(5).map(|node| node.id.clone()).collect();
    let exclude_relations = if ast_only_tuning {
        vec!["imports".to_string()]
    } else {
        Vec::new()
    };

    if seeds.is_empty() {
        eprintln!("[graphenium] No nodes found in the selected query scope.");
        return Ok(());
    }

    // Traverse the graph.
    let max_nodes = (budget / 40).max(5).min(200);
    let visited = if dfs {
        serve_traversal::dfs_with_filters(
            &graph,
            &seeds,
            max_nodes,
            3,
            scoped.as_ref(),
            &[],
            &exclude_relations,
        )
    } else {
        serve_traversal::bfs_with_filters(
            &graph,
            &seeds,
            max_nodes,
            3,
            scoped.as_ref(),
            &[],
            &exclude_relations,
        )
    };

    let scope_node_count = scoped
        .as_ref()
        .map(|allowed| allowed.len())
        .unwrap_or_else(|| graph.node_count());

    // Format output within the token budget (rough: 4 chars ≈ 1 token).
    let chars_budget = budget * 4;
    let mut output = format!(
        "# Graph Query: {question}\n\nFound {} relevant nodes (of {})\n\n",
        visited.len(),
        scope_node_count
    );
    if ast_only_tuning {
        output.push_str(
            "AST-only tuning active: suppressing common import/generated-code noise by default\n\n",
        );
    }
    match generated_code_mode {
        serve_traversal::GeneratedCodeMode::Include => {}
        serve_traversal::GeneratedCodeMode::Exclude => {
            output.push_str("Filter: generated/template/vendor paths excluded\n\n");
        }
        serve_traversal::GeneratedCodeMode::Only => {
            output.push_str("Filter: only generated/template/vendor paths included\n\n");
        }
    }
    output.push_str(&serve_traversal::subgraph_to_text_with_match_details(
        &graph,
        &visited,
        chars_budget.saturating_sub(output.len()),
        &[],
        &exclude_relations,
        &ranked,
    ));

    print!("{output}");
    Ok(())
}

// ── `setup` command ────────────────────────────────────────────────────────────

fn cmd_setup(target: &str, gm_path: Option<PathBuf>, graph: &Path) -> graphenium::Result<()> {
    let gm =
        gm_path.unwrap_or_else(|| std::env::current_exe().unwrap_or_else(|_| PathBuf::from("gm")));
    let gm_str = gm.display();
    let graph_abs = graph.canonicalize().unwrap_or_else(|_| graph.to_path_buf());
    let graph_str = graph_abs.display();

    match target.to_lowercase().as_str() {
        "claude" | "claude-desktop" | "claude_code" | "claude-code" => {
            println!("Add this to your Claude Desktop config (claude_desktop_config.json):");
            println!();
            println!("{{\n  \"mcpServers\": {{\n    \"graphenium\": {{\n      \"command\": \"{gm_str}\",\n      \"args\": [\"serve\", \"--graph\", \"{graph_str}\"]\n    }}\n  }}\n}}");
        }
        "cursor" => {
            println!("Add this to ~/.cursor/mcp.json:");
            println!();
            println!("{{\n  \"mcpServers\": {{\n    \"graphenium\": {{\n      \"command\": \"{gm_str}\",\n      \"args\": [\"serve\", \"--graph\", \"{graph_str}\"]\n    }}\n  }}\n}}");
        }
        "codewhale" | "codex" => {
            println!("Add this to ~/.codewhale/mcp.json:");
            println!();
            println!("{{\n  \"servers\": {{\n    \"graphenium\": {{\n      \"command\": \"{gm_str}\",\n      \"args\": [\"serve\", \"--graph\", \"{graph_str}\"],\n      \"env\": {{}}\n    }}\n  }}\n}}");
        }
        other => {
            eprintln!("Unknown target '{other}'. Supported: claude, cursor, codewhale");
        }
    }

    println!();
    println!("After updating the config, restart your AI tool completely (Cmd+Q on macOS).");
    Ok(())
}

/// Build a one-line label-collision summary for the pipeline log.
///
/// Counts how many distinct `label` values appear on two or more nodes, and
/// calls out the three worst offenders. Returns `None` when there are no
/// collisions — no news is good news. Qualified labels do *not* shrink this
/// count: the motivating question is "how ambiguous are the short names
/// users see first?"
fn label_collision_report(graph: &graphenium::model::GrapheniumGraph) -> Option<String> {
    use std::collections::HashMap;

    let mut counts: HashMap<&str, usize> = HashMap::new();
    let mut total_nodes = 0usize;
    for node in graph.nodes() {
        total_nodes += 1;
        *counts.entry(node.label.as_str()).or_default() += 1;
    }

    let colliding: Vec<(&str, usize)> = counts.into_iter().filter(|(_, c)| *c >= 2).collect();
    if colliding.is_empty() {
        return None;
    }

    let colliding_nodes: usize = colliding.iter().map(|(_, c)| *c).sum();
    let pct = if total_nodes > 0 {
        (colliding_nodes as f64 / total_nodes as f64) * 100.0
    } else {
        0.0
    };

    let mut worst: Vec<(&str, usize)> = colliding.clone();
    worst.sort_unstable_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    worst.truncate(3);
    let worst_str = worst
        .iter()
        .map(|(l, c)| format!("{l}={c}x"))
        .collect::<Vec<_>>()
        .join(", ");

    Some(format!(
        "Label collisions: {distinct} label(s) appear ≥2x ({affected} of {total} nodes, {pct:.1}%), worst: {worst_str}",
        distinct = colliding.len(),
        affected = colliding_nodes,
        total = total_nodes,
    ))
}
