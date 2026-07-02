use std::path::{Path, PathBuf};
use std::process;

use clap::{Parser, Subcommand};

use graphenium::analyze;
use graphenium::analyze::query;
use graphenium::build;
use graphenium::cache::Manifest;
use graphenium::cluster::{self, ClusterOptions};
use graphenium::detect::{self, DetectOptions};
use graphenium::export;
use graphenium::export::json::load_graph;
use graphenium::extract::ci;
use graphenium::extract::{self, ExtractMode, ExtractOptions};
use graphenium::harness;
use graphenium::model::graph::GrapheniumGraph;
use graphenium::model::{Confidence, ExtractionResult};
use graphenium::ranking;
use graphenium::report::{self, ReportInput};
use graphenium::semantic::{self, AiProvider, SemanticOptions};
use graphenium::serve::traversal as serve_traversal;
use graphenium::trust;

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

// ── Graph sub-commands ──────────────────────────────────────────────────────────

#[derive(Subcommand)]
enum GraphCommands {
    /// Migrate a graph from an older schema version
    Migrate {
        /// Path to the graph.json file
        graph: PathBuf,
    },
    /// Load and print graph metadata / schema
    Schema {
        /// Path to graph.json (default: graphenium-out/graph.json)
        #[arg(default_value = "graphenium-out/graph.json")]
        graph: PathBuf,
    },
    /// Print build targets from CI extraction
    BuildMap {
        /// Path to graph.json (default: graphenium-out/graph.json)
        #[arg(default_value = "graphenium-out/graph.json")]
        graph: PathBuf,
    },
    /// Print test targets from CI extraction
    TestMap {
        /// Path to graph.json (default: graphenium-out/graph.json)
        #[arg(default_value = "graphenium-out/graph.json")]
        graph: PathBuf,
    },
}

// ── Snapshot sub-commands ───────────────────────────────────────────────────────

#[derive(Subcommand)]
enum SnapshotCommands {
    /// Create a new snapshot
    Create {
        /// Name for the snapshot
        #[arg(long)]
        name: String,
    },
    /// List available snapshots
    List,
}

// ── Top-level commands ──────────────────────────────────────────────────────────

#[derive(Subcommand)]
enum Commands {
    /// Initialize a Graphenium workspace with default config files
    Init {
        /// Directory to initialize (default: current directory)
        #[arg(default_value = ".")]
        path: PathBuf,
    },

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

        /// Comma-separated list of directories to exclude (e.g. target,node_modules,.git)
        #[arg(long)]
        exclude_dirs: Option<String>,

        /// Skip GRAPH_REPORT.md generation
        #[arg(long)]
        no_report: bool,

        /// Only plan: scan directory and report file statistics without running extraction
        #[arg(long)]
        plan: bool,
    },

    /// Query the knowledge graph with keywords
    Query {
        /// Keywords or question to match against the graph
        question: String,

        /// Use depth-first search instead of the default BFS
        #[arg(long)]
        dfs: bool,

        /// Safe mode: use structural query mode for safer results
        #[arg(long)]
        safe: bool,
        #[arg(long)]
        datalog: Option<String>,

        /// Maximum output token budget (rough estimate)
        #[arg(long, default_value = "2000")]
        budget: usize,

        /// Path to graph.json produced by `gm run`
        #[arg(long, default_value = "graphenium-out/graph.json")]
        graph: PathBuf,

        /// Query mode: lexical (default), structural, or hybrid
        #[arg(long, default_value = "lexical")]
        mode: String,

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

        /// Output query results as JSON (machine-parseable)
        #[arg(long)]
        json: bool,
    },

    /// Start the MCP server for agent/tool integration (stdio JSON-RPC)
    Serve {
        /// Path to graph.json
        #[arg(long, default_value = "graphenium-out/graph.json")]
        graph: PathBuf,

        /// Watch graph file for changes and auto-reload
        #[arg(long)]
        watch: bool,
    },

    /// Run diagnostic checks on the Graphenium installation
    Doctor {
        /// Optional path to graph.json
        #[arg(long)]
        graph: Option<PathBuf>,

        /// Show graph schema information
        #[arg(long)]
        schema: bool,

        /// Show resolution quality report
        #[arg(long)]
        resolution: bool,

        /// Show repository info from graph metadata
        #[arg(long)]
        repository: bool,
    },

    /// Run trust quality checks and enforce gates for CI
    Check {
        /// Path to the graph to check
        #[arg(long, default_value = "graphenium-out/graph.json")]
        graph: PathBuf,

        /// Minimum resolution percentage (default: 80)
        #[arg(long, default_value_t = 80.0)]
        min_resolution: f64,

        /// Maximum number of ambiguous edges allowed (default: 10)
        #[arg(long, default_value_t = 10)]
        max_ambiguous: usize,

        /// Exit with non-zero if any check fails
        #[arg(long)]
        strict: bool,

        /// [k.4] Plan ID for plan compliance verification
        #[arg(long)]
        plan: Option<String>,
    },

    /// Generate a pre-edit orientation overview for a symbol
    Explain {
        /// Symbol to explain
        symbol: String,

        /// Path to the graph to use
        #[arg(long, default_value = "graphenium-out/graph.json")]
        graph: PathBuf,
    },

    /// Diff two graph snapshots and show symbol-level changes
    Diff {
        /// Path to the old (before) graph.json
        /// If omitted, reads the last snapshot from the current out dir
        #[arg(long)]
        before: Option<PathBuf>,

        /// Path to the new (after) graph.json
        /// Defaults to the current graphenium-out/graph.json
        #[arg(long, default_value = "graphenium-out/graph.json")]
        after: PathBuf,

        /// Show detailed impact analysis
        #[arg(long)]
        impact: bool,

        /// Show review plan with prioritized verification steps
        #[arg(long)]
        review_plan: bool,
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

        /// Show blast radius impact after rebuilds
        #[arg(long)]
        impact: bool,
    },

    /// Inspect and manage the knowledge graph
    Graph {
        #[command(subcommand)]
        command: GraphCommands,
    },

    /// Create and manage graph snapshots
    Snapshot {
        #[command(subcommand)]
        command: SnapshotCommands,
    },

    /// Run gate checks and quality gates for CI
    Gate {
        /// Diff two graph snapshots: --diff <before> <after>
        #[arg(long, num_args = 2, value_names = ["before", "after"])]
        diff: Option<Vec<PathBuf>>,
    },
}

// ── Entry point ────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let _result = match cli.command {
        Commands::Init { path } => {
            match graphenium::detect::initialize_workspace(&path) {
                Ok(true) => println!(
                    "Initialized Graphenium workspace at '{}'. Created .grapheniumignore.",
                    path.display()
                ),
                Ok(false) => println!(
                    "Workspace at '{}' already initialized (.grapheniumignore exists).",
                    path.display()
                ),
                Err(e) => eprintln!("Failed to initialize workspace: {e}"),
            }
            Ok(())
        }

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
            exclude_dirs,
            no_report,
            plan,
        } => {
            if plan {
                cmd_plan(&path)
            } else {
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
                    exclude_dirs,
                    no_report,
                )
                .await
            }
        }

        Commands::Query {
            question,
            datalog,
            dfs,
            safe,
            budget,
            graph,
            mode,
            path_prefix,
            exclude_path,
            generated_code_mode,
            ast_only_tuning,
            json,
        } => {
            if let Some(ref dl) = datalog {
                let graph_path = graph.to_str().unwrap_or("graphenium-out/graph.json");
                match graphenium::export::json::load_graph(graph_path) {
                    Ok(g) => match query::run_datalog_query(&g, dl, 1000) {
                        Ok(r) => println!("{}", r),
                        Err(e) => eprintln!("Datalog error: {}", e),
                    },
                    Err(e) => eprintln!("Failed to load graph: {}", e),
                }
                cmd_query(question, dfs, safe, budget, graph, &mode,
                    path_prefix, exclude_path, generated_code_mode, ast_only_tuning, json)
            }
        }

        Commands::Mint { num_tokens, .. } => 
                question,
                dfs,
                safe,
                budget,
                graph,
                &mode,
                path_prefix,
                exclude_path,
                generated_code_mode,
                ast_only_tuning,
                json,
            )
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
        }

        Commands::Serve { graph, watch } => {
            if watch {
                graphenium::serve::serve_with_watch(&graph, true).await
            } else {
                graphenium::serve::serve(&graph).await
            }
        }

        Commands::Doctor {
            graph,
            schema,
            resolution,
            repository,
        } => {
            let g = graph.as_deref();
            if schema {
                graphenium::doctor::show_schema(g);
            } else if resolution {
                graphenium::doctor::show_resolution(g);
            } else if repository {
                cmd_doctor_repository(g);
            } else {
                graphenium::doctor::run_doctor(g);
            }
            Ok(())
        }

        Commands::Check {
            graph,
            min_resolution,
            max_ambiguous,
            strict,
            plan,
        } => cmd_check(&graph, min_resolution, max_ambiguous, strict, plan),

        Commands::Explain { symbol, graph } => cmd_explain(&symbol, &graph),

        Commands::Diff {
            before,
            after,
            impact,
            review_plan,
        } => {
            if review_plan {
                cmd_review_plan(before.as_deref(), &after)
            } else {
                cmd_diff(before.as_deref(), &after, impact)
            }
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
            impact,
        } => {
            let show_impact = impact;
            match tokio::task::spawn_blocking(move || {
                graphenium::watch::watch(&path, debounce, incremental)
            })
            .await
            {
                Ok(result) => result,
                Err(e) => Err(graphenium::GrapheniumError::Watch(format!("{e}"))),
            }
        }

        Commands::Graph { command } => match command {
            GraphCommands::Migrate { graph } => {
                eprintln!("not yet implemented: migrate {}", graph.display());
                Ok(())
            }
            GraphCommands::Schema { graph } => cmd_graph_schema(&graph),
            GraphCommands::BuildMap { graph } => cmd_graph_build_map(&graph),
            GraphCommands::TestMap { graph } => cmd_graph_test_map(&graph),
        },

        Commands::Snapshot { command } => match command {
            SnapshotCommands::Create { name } => cmd_snapshot_create(&name),
            SnapshotCommands::List => cmd_snapshot_list(),
        },

        Commands::Gate { diff } => {
            if let Some(paths) = diff {
                eprintln!(
                    "not yet implemented: gate --diff {} {}",
                    paths[0].display(),
                    paths[1].display()
                );
            } else {
                eprintln!("not yet implemented: gate");
            }
            Ok(())
        }
    };

    if let Err(e) = _result {
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
    exclude_dirs: Option<String>,
    no_report: bool,
) -> graphenium::Result<()> {
    let root = path.canonicalize().unwrap_or_else(|_| path.clone());
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

    let (mut all_files, corpus_warnings) = detect::detect(&root, &DetectOptions::default())?;

    // Apply --exclude-dirs filter
    if let Some(ref dirs) = exclude_dirs {
        let excluded: Vec<&str> = dirs.split(',').map(|s| s.trim()).collect();
        all_files.retain(|f| {
            let path_str = f.path.to_string_lossy();
            !excluded
                .iter()
                .any(|d| path_str.contains(&format!("/{}/", d)) || path_str.ends_with(d))
        });
        if !excluded.is_empty() {
            eprintln!(
                "[graphenium] Excluded {} dir(s), {} file(s) remaining",
                excluded.len(),
                all_files.len()
            );
        }
    }

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
        cache_manager: Some(std::sync::Arc::new(graphenium::cache::CacheManager::new(
            out_dir.join("cache"),
        ))),
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

    // ── 4b. C#/CI build config extraction ──────────────────────────────────────
    let mut ci_result = ExtractionResult::default();
    // Scan for C# project files and CI configs in the project root
    if let Ok(entries) = std::fs::read_dir(&path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                if let Ok(sub) = std::fs::read_dir(&p) {
                    for s in sub.flatten() {
                        let sp = s.path();
                        let ext = sp.extension().and_then(|e| e.to_str()).unwrap_or("");
                        if ext == "csproj" || ext == "sln" {
                            eprintln!("[graphenium] Parsing C# project: {}", sp.display());
                            if ext == "sln" {
                                let ws = graphenium::extract::csharp_project::CSharpWorkspace::parse_solution(&sp);
                                ci_result.merge(ci::csproj_to_extraction(&ws));
                            } else {
                                let ws = graphenium::extract::csharp_project::CSharpWorkspace::parse_csproj(&sp);
                                let mut single =
                                    graphenium::extract::csharp_project::CSharpWorkspace::default();
                                single.projects.insert(sp.clone(), ws);
                                ci_result.merge(ci::csproj_to_extraction(&single));
                            }
                        }
                    }
                }
            }
        }
    }

    // ── 5. Build graph ─────────────────────────────────────────────────────────
    eprintln!("[graphenium] Building graph...");
    let (mut graph, build_stats) = if ci_result.nodes.is_empty() {
        build::build_merged([ast_result, semantic_result])
    } else {
        eprintln!(
            "[graphenium] CI/C# extraction: {} nodes, {} edges",
            ci_result.nodes.len(),
            ci_result.edges.len()
        );
        build::build_merged([ast_result, semantic_result, ci_result])
    };
    graph.set_ast_only(ast_only_graph);

    // Populate graph metadata.
    graph.metadata.schema_version = Some("0.2.0".to_string());
    let now: String = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| {
            let secs = d.as_secs();
            // ISO 8601 approximation: YYYY-MM-DDTHH:MM:SSZ
            let days = secs / 86400;
            let time_secs = secs % 86400;
            let h = time_secs / 3600;
            let m = (time_secs % 3600) / 60;
            let s = time_secs % 60;
            // Simple date calculation from Unix epoch (1970-01-01)
            // Works for dates up to ~2100
            let mut y = 1970i64;
            let mut remaining = days as i64;
            loop {
                let days_in_year = if is_leap(y) { 366 } else { 365 };
                if remaining < days_in_year {
                    break;
                }
                remaining -= days_in_year;
                y += 1;
            }
            let leap = is_leap(y);
            let month_days = [
                31,
                if leap { 29 } else { 28 },
                31,
                30,
                31,
                30,
                31,
                31,
                30,
                31,
                30,
                31,
            ];
            let mut mo = 1usize;
            for &md in month_days.iter() {
                if remaining < md {
                    break;
                }
                remaining -= md;
                mo += 1;
            }
            let day = remaining + 1;
            format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, mo, day, h, m, s)
        })
        .unwrap_or_else(|_| String::new());
    graph.metadata.created_at = Some(now);
    graph.metadata.project_root = Some(root.display().to_string());
    graph.metadata.extraction_modes = Some(if no_semantic || ast_only_graph {
        vec!["ast".to_string()]
    } else {
        vec!["ast".to_string(), "semantic".to_string()]
    });
    // Collect detected languages from the files.
    let mut lang_set: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for file in &all_files {
        if let Some(ext) = file.path.extension().and_then(|e| e.to_str()) {
            let lang = match ext {
                "rs" => "rust",
                "py" => "python",
                "go" => "go",
                "js" | "mjs" | "cjs" => "javascript",
                "ts" | "tsx" => "typescript",
                "c" | "h" => "c",
                "cpp" | "cc" | "cxx" | "hpp" => "cpp",
                "java" => "java",
                "cs" => "csharp",
                _ => continue,
            };
            lang_set.insert(lang.to_string());
        }
    }
    if !lang_set.is_empty() {
        graph.metadata.languages = Some(lang_set.into_iter().collect());
    }
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

    // ── 8b. Quality report ─────────────────────────────────────────────────
    let quality_path = out_dir.join("quality.json");
    let quality = export::json::generate_quality_report(&graph);
    let quality_json = serde_json::to_string_pretty(&quality)?;
    std::fs::write(&quality_path, &quality_json)?;
    eprintln!("[graphenium] Wrote: {}", quality_path.display());

    // ── 9. Report ──────────────────────────────────────────────────────────────
    if !no_report {
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
    }

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

// ── `plan` command ─────────────────────────────────────────────────────────────

/// Scan a directory and report file statistics without running the full extraction.
/// Used via `gm run --plan <path>`.
fn cmd_plan(root: &Path) -> graphenium::Result<()> {
    eprintln!("[graphenium] Planning scan for: {}", root.display());

    let (files, _corpus_warnings) = detect::detect(root, &DetectOptions::default())?;
    let total = files.len();

    // Group by extension
    let mut ext_counts: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    // Directories that look vendored / standard-library-ish
    let mut vendor_dirs: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();

    let vendor_keywords = [
        "Lib",
        "lib",
        "third_party",
        "third-party",
        "vendor",
        "vendored",
        "bin",
        "obj",
        "packages",
        "deps",
        "external",
    ];

    for f in &files {
        // Count by extension
        let ext = f
            .path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_else(|| "no-ext".to_string());
        *ext_counts.entry(ext).or_insert(0) += 1;

        // Flag vendor/lib directories
        if let Some(parent) = f.path.parent() {
            for component in parent.components() {
                if let std::path::Component::Normal(c) = component {
                    let name = c.to_string_lossy();
                    if vendor_keywords.iter().any(|vk| name == *vk) {
                        *vendor_dirs.entry(name.to_string()).or_insert(0) += 1;
                    }
                }
            }
        }
    }

    println!("═══════════════════════════════════════════");
    println!("  Graphenium Plan — {}", root.display());
    println!("═══════════════════════════════════════════");
    println!("  Total detected files: {}", total);
    println!();

    if !ext_counts.is_empty() {
        println!("  Files by extension:");
        for (ext, count) in &ext_counts {
            println!("    .{ext}: {count}");
        }
        println!();
    }

    if !vendor_dirs.is_empty() {
        println!("  Vendor / standard-library directories detected:");
        for (dir, count) in &vendor_dirs {
            println!("    {dir}/ — {count} file(s)");
        }
        eprintln!();
        eprintln!("[graphenium] note: vendor/lib dirs shown above. These may contain");
        eprintln!("  third-party code that inflates the graph. Consider adding them");
        eprintln!("  to .grapheniumignore or using --exclude-dirs if they are not");
        eprintln!("  part of your own source.");
    }

    Ok(())
}

// ── `query` command ────────────────────────────────────────────────────────────

fn cmd_query(
    question: String,
    dfs: bool,
    safe: bool,
    budget: usize,
    graph_path: PathBuf,
    mode: &str,
    path_prefix: Option<String>,
    exclude_path: Option<String>,
    generated_code_mode: String,
    ast_only_tuning: String,
    json: bool,
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
        true,
    );
    let qmode = if safe {
        ranking::QueryMode::Structural
    } else {
        ranking::QueryMode::from_str(mode)
    };
    let ranked = ranking::score_query_nodes_with_mode(&graph, &question, qmode, scoped.as_ref());
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

    // ── JSON output (machine-parseable) ────────────────────────────────────
    if json {
        let entries: Vec<serde_json::Value> = visited
            .iter()
            .filter_map(|id| {
                let node = graph.node_data(id)?;
                Some(serde_json::json!({
                    "node_id": node.id,
                    "label": node.label,
                    "degree": graph.degree(id),
                    "source_file": node.source_file,
                    "community": node.community,
                }))
            })
            .collect();
        let output = serde_json::to_string_pretty(&entries)?;
        println!("{output}");
        return Ok(());
    }

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

// ── `check` command ────────────────────────────────────────────────────────────

fn cmd_check(
    graph_path: &Path,
    min_resolution: f64,
    max_ambiguous: usize,
    strict: bool,
    plan: Option<String>,
) -> graphenium::Result<()> {
    let graph = export::json::load_graph(graph_path)?;

    // Handle plan compliance verification
    if let Some(plan_id) = plan {
        println!("=== Graphenium Plan Compliance Gate ===");
        let report = harness::verify_plan(&graph, &plan_id);
        println!("Plan ID: {}", report.plan_id);
        println!(
            "Compliance: {}",
            if report.passes_compliance {
                "PASS"
            } else {
                "FAIL"
            }
        );
        if !report.implemented_nodes.is_empty() {
            println!("\nImplemented: {:?}", report.implemented_nodes);
        }
        if !report.missing_nodes.is_empty() {
            println!("Missing: {:?}", report.missing_nodes);
        }
        if !report.unplanned_modified_files.is_empty() {
            println!("Unplanned changes: {:?}", report.unplanned_modified_files);
        }
        if !report.passes_compliance && strict {
            std::process::exit(1);
        }
        return Ok(());
    }

    // Check if this is an AST-only pre-resolver graph
    let has_resolver_edges: bool = graph
        .edges_iter()
        .any(|e| e.extractor.as_deref() == Some("resolver"));

    if !has_resolver_edges && (graph.edges_iter().count() > 0) {
        println!(
            "NOTE: Graph has no resolver annotations. Resolution checks apply only\n\
                  to graphs built with the full pipeline (including import resolution).\n\
                  Skipping resolution gate — checking confidence and ambiguous edges only.\n"
        );
    }

    let mut report = trust::ResolutionReport::default();

    // Build resolution report from graph edges
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
                // method edges: contains, etc.
                if edge.relation != "contains" && edge.relation != "method" {
                    report.total_method_edges += 1;
                    if edge.resolution_status.as_deref() == Some("resolved") {
                        report.resolved_methods += 1;
                    }
                }
            }
        }
        match edge.confidence {
            Confidence::Extracted => {}
            Confidence::Inferred => report.heuristic_edges += 1,
            Confidence::Ambiguous => report.ambiguous_edges += 1,
        }
    }

    let ast_only = !has_resolver_edges && graph.edges_iter().count() > 0;

    if ast_only {
        // AST-only graphs: skip resolution gate, only check ambiguous count
        let ambiguous_count = report.ambiguous_edges;
        if report.ambiguous_edges > max_ambiguous {
            eprintln!(
                "\n⚠ Trust check FAILED: {max_ambiguous} max ambiguous edges allowed, found {ambiguous_count}."
            );
            if strict {
                std::process::exit(1);
            }
        } else {
            println!(
                "\n✓ Trust check PASSED (ambiguous edges: {ambiguous_count}, max: {max_ambiguous})"
            );
        }
        return Ok(());
    }

    let result = harness::check_resolution_quality(&graph, &report, min_resolution, max_ambiguous);

    for detail in &result.details {
        println!("{detail}");
    }

    if !result.passed {
        eprintln!("\n⚠ Trust check FAILED");
        if strict {
            std::process::exit(1);
        }
    } else {
        println!("\n✓ Trust check PASSED");
    }

    Ok(())
}

// ── `explain` command ─────────────────────────────────────────────────────────

fn cmd_explain(symbol: &str, graph_path: &Path) -> graphenium::Result<()> {
    let graph = export::json::load_graph(graph_path)?;
    let (resolved, _) = serve_traversal::resolve_symbols_to_ids(&graph, symbol);
    if let Some(target_id) = resolved.first() {
        if let Some(explanation) = serve_traversal::explain_subsystem(&graph, target_id) {
            let report = serve_traversal::format_explanation_report(&graph, &explanation);
            println!("{}", report);
        } else {
            eprintln!("Error: Failed to generate subsystem explanation.");
        }
    } else {
        eprintln!("Error: Symbol '{}' not found in the graph.", symbol);
    }
    Ok(())
}

// ── `diff` command ─────────────────────────────────────────────────────────────

fn cmd_diff(before: Option<&Path>, after: &Path, show_impact: bool) -> graphenium::Result<()> {
    // Load the "after" graph (the new one)
    let new = export::json::load_graph(after)?;

    // Load the "before" graph (if provided, otherwise use an empty graph)
    let old = match before {
        Some(p) => export::json::load_graph(p)?,
        None => GrapheniumGraph::new(),
    };

    // Compute diffs
    let diff = analyze::diff::diff(&old, &new);
    let symbol_changes = analyze::impact::symbol_inventory_diff(&old, &new);
    let impact = analyze::impact::downstream_impact(&new, &symbol_changes);

    println!("# Graph Diff\n");

    if diff.added_nodes.is_empty()
        && diff.removed_nodes.is_empty()
        && diff.added_edges.is_empty()
        && diff.removed_edges.is_empty()
    {
        println!("No changes detected.");
        return Ok(());
    }

    if !diff.removed_nodes.is_empty() {
        println!("## Removed Symbols ({})", diff.removed_nodes.len());
        for id in &diff.removed_nodes {
            println!("  - {id}");
        }
        println!();
    }

    if !diff.added_nodes.is_empty() {
        println!("## Added Symbols ({})", diff.added_nodes.len());
        for id in &diff.added_nodes {
            println!("  - {id}");
        }
        println!();
    }

    if !diff.removed_edges.is_empty() {
        println!("## Removed Edges ({})", diff.removed_edges.len());
        for (s, t, r) in &diff.removed_edges {
            println!("  - {s} `{r}` {t}");
        }
        println!();
    }

    if !diff.added_edges.is_empty() {
        println!("## Added Edges ({})", diff.added_edges.len());
        for (s, t, r) in &diff.added_edges {
            println!("  - {s} `{r}` {t}");
        }
        println!();
    }

    // Community changes
    let community_changes: Vec<_> = symbol_changes
        .iter()
        .filter(|c| matches!(c, analyze::impact::SymbolChange::CommunityChanged { .. }))
        .collect();
    if !community_changes.is_empty() {
        println!("## Community Changes ({})", community_changes.len());
        for change in &community_changes {
            if let analyze::impact::SymbolChange::CommunityChanged {
                id,
                label,
                old_community,
                new_community,
                ..
            } = change
            {
                println!(
                    "  - {label} ({id}): community {old:?} -> {new:?}",
                    old = old_community,
                    new = new_community
                );
            }
        }
        println!();
    }

    // Impact analysis
    if show_impact && !impact.downstream_nodes.is_empty() {
        println!("## Downstream Impact");
        println!("  - {} affected nodes", impact.downstream_nodes.len());
        println!(
            "  - {} affected communities",
            impact.affected_communities.len()
        );
        println!(
            "  - {} EXTRACTED, {} INFERRED, {} AMBIGUOUS edges",
            impact.extracted_edges, impact.inferred_edges, impact.ambiguous_edges
        );

        // Review order
        let order = analyze::impact::review_order(&impact);
        if !order.is_empty() {
            println!("\n## Recommended Review Order");
            for (i, change) in order.iter().enumerate() {
                match change {
                    analyze::impact::SymbolChange::Removed { id, label, .. } => {
                        println!("  {}. REMOVED {label} ({id})", i + 1);
                    }
                    analyze::impact::SymbolChange::Added { id, label, .. } => {
                        println!("  {}. ADDED {label} ({id})", i + 1);
                    }
                    analyze::impact::SymbolChange::CommunityChanged { id, label, .. } => {
                        println!("  {}. COMMUNITY CHANGED {label} ({id})", i + 1);
                    }
                }
            }
        }
    }

    Ok(())
}

// ── `review-plan` command ──────────────────────────────────────────────────────

fn cmd_review_plan(before: Option<&Path>, after: &Path) -> graphenium::Result<()> {
    let new = export::json::load_graph(after)?;
    let old = match before {
        Some(p) => export::json::load_graph(p)?,
        None => GrapheniumGraph::new(),
    };

    let symbol_changes = analyze::impact::symbol_inventory_diff(&old, &new);
    let _impact = analyze::impact::downstream_impact(&new, &symbol_changes);

    // Collect changed node IDs
    let changed_ids: Vec<String> = symbol_changes
        .iter()
        .map(|c| match c {
            analyze::impact::SymbolChange::Added { id, .. } => id.clone(),
            analyze::impact::SymbolChange::Removed { id, .. } => id.clone(),
            analyze::impact::SymbolChange::CommunityChanged { id, .. } => id.clone(),
        })
        .collect();

    let plan = analyze::verifier::plan_verification(&new, &changed_ids, None);
    println!("{}", analyze::verifier::format_plan(&plan));
    Ok(())
}

// ── `snapshot` commands ────────────────────────────────────────────────────────

/// Create a snapshot of the current graph by copying it to graphenium-snapshots/.
fn cmd_snapshot_create(name: &str) -> graphenium::Result<()> {
    let snap_dir = PathBuf::from("graphenium-snapshots");
    std::fs::create_dir_all(&snap_dir)?;

    let src = PathBuf::from("graphenium-out/graph.json");
    if !src.exists() {
        eprintln!("No graph found at {} — run `gm run` first.", src.display());
        return Ok(());
    }

    let dest = snap_dir.join(format!("{name}.json"));
    std::fs::copy(&src, &dest).map_err(|e| graphenium::GrapheniumError::Io(e))?;

    println!("Snapshot '{name}' saved to {}.", dest.display());
    Ok(())
}

/// List available snapshots.
fn cmd_snapshot_list() -> graphenium::Result<()> {
    let snap_dir = PathBuf::from("graphenium-snapshots");
    if !snap_dir.exists() {
        println!("No snapshots found. Use `gm snapshot create --name <name>` to create one.");
        return Ok(());
    }

    let entries = std::fs::read_dir(&snap_dir)?;
    let mut names: Vec<String> = Vec::new();
    for entry in entries.flatten() {
        if let Some(name) = entry.file_name().to_str() {
            if name.ends_with(".json") {
                names.push(name.trim_end_matches(".json").to_string());
            }
        }
    }

    if names.is_empty() {
        println!("No snapshots found in {}.", snap_dir.display());
    } else {
        println!("Available snapshots ({}):", names.len());
        for name in &names {
            let path = snap_dir.join(format!("{name}.json"));
            let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            println!("  {name} ({size} bytes)");
        }
    }

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
        "claude" => {
            println!("Available Claude targets:");
            println!("  claude-desktop   — Claude Desktop app config (claude_desktop_config.json)");
            println!("  claude-code      — Claude Code CLI (uses `claude mcp add` command)");
            println!();
            println!("Run `gm setup claude-desktop` or `gm setup claude-code` for details.");
        }
        "claude-desktop" | "claude_desktop" => {
            println!("Claude Desktop MCP Configuration");
            println!("================================");
            println!();
            println!("Add this to your claude_desktop_config.json:");
            println!();
            println!("{{\n  \"mcpServers\": {{\n    \"graphenium\": {{\n      \"command\": \"{gm_str}\",\n      \"args\": [\"serve\", \"--graph\", \"{graph_str}\"]\n    }}\n  }}\n}}");
            println!();
            println!("Config file locations:");
            println!("  macOS:   ~/Library/Application Support/Claude/claude_desktop_config.json");
            println!("  Windows: %APPDATA%\\Claude\\claude_desktop_config.json");
            println!("  Linux:   ~/.config/Claude/claude_desktop_config.json");
        }
        "claude-code" | "claude_code" => {
            println!("Claude Code MCP Configuration");
            println!("=============================");
            println!();
            println!(
                "Claude Code uses the `claude mcp add` command instead of a JSON config file."
            );
            println!();
            println!("Register Graphenium as an MCP server:");
            println!("  claude mcp add graphenium --scope user -- {gm_str} serve");
            println!();
            println!("Verify registration:");
            println!("  claude mcp list");
            println!();
            println!("The server will start with an empty graph. Run `gm run . --no-semantic`");
            println!("in your project directory to generate the codebase map.");
            println!();
            println!("You can also install the Graphenium skill for Claude Code:");
            println!("  Copy skills/graphenium/SKILL.md to ~/.claude/skills/graphenium/SKILL.md");
        }
        "cursor" => {
            println!("Add this to ~/.cursor/mcp.json:");
            println!();
            println!("{{\n  \"mcpServers\": {{\n    \"graphenium\": {{\n      \"command\": \"{gm_str}\",\n      \"args\": [\"serve\", \"--graph\", \"{graph_str}\"]\n    }}\n  }}\n}}");
        }
        "codewhale" | "codex" => {
            println!("Add this to ~/.codewhale/config.toml:");
            println!();
            println!("[mcp_servers.graphenium]");
            println!("command = \"{gm_str}\"");
            println!("args = [\"serve\", \"--graph\", \"{graph_str}\"]");
        }
        other => {
            eprintln!("Unknown target '{other}'. Supported: claude, cursor, codewhale");
        }
    }

    println!();
    println!("After updating the config, restart your AI tool completely (Cmd+Q on macOS).");
    Ok(())
}

/// Check if a year is a leap year (ISO 8601 / Gregorian).
/// Used to compute ISO 8601 timestamps without external dependencies.
fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
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

// ── `graph` sub-commands ────────────────────────────────────────────────────────

/// Load and print graph schema / metadata (`graph schema`).
fn cmd_graph_schema(graph_path: &Path) -> graphenium::Result<()> {
    let graph = load_graph(graph_path)?;
    println!("Graph Metadata:");
    if let Some(ref v) = graph.metadata.schema_version {
        println!("  schema_version: {v}");
    }
    if let Some(ref v) = graph.metadata.graphenium_version {
        println!("  graphenium_version: {v}");
    }
    if let Some(ref v) = graph.metadata.created_at {
        println!("  created_at: {v}");
    }
    if let Some(ref v) = graph.metadata.project_root {
        println!("  project_root: {v}");
    }
    if let Some(ref modes) = graph.metadata.extraction_modes {
        println!("  extraction_modes: {}", modes.join(", "));
    }
    if let Some(ref langs) = graph.metadata.languages {
        println!("  languages: {}", langs.join(", "));
    }
    println!("  ast_only: {}", graph.metadata.ast_only);
    println!("  node_count: {}", graph.node_count());
    println!("  edge_count: {}", graph.edge_count());
    Ok(())
}

/// Load graph and print build targets from CI extraction (`graph build-map`).
fn cmd_graph_build_map(graph_path: &Path) -> graphenium::Result<()> {
    let graph = load_graph(graph_path)?;
    let root = graph
        .metadata
        .project_root
        .as_deref()
        .map(Path::new)
        .unwrap_or_else(|| Path::new("."));

    let ci_files = discover_ci_configs(root);
    if ci_files.is_empty() {
        println!("No CI configuration files found under {}", root.display());
        return Ok(());
    }
    for file in &ci_files {
        let content = match std::fs::read_to_string(file) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("warn: could not read {}: {e}", file.display());
                continue;
            }
        };
        let targets = ci::parse_ci_config(&file.to_string_lossy(), &content);
        for t in &targets {
            if !t.built_files.is_empty() {
                println!("[build] {} ({:?})", t.name, t.kind);
                for bf in &t.built_files {
                    println!("  -> {bf}");
                }
            }
        }
    }

    // Scan for C# project files
    println!("\n=== C# Projects ===");
    let mut csharp_found = false;
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Ok(sub_entries) = std::fs::read_dir(&path) {
                    for sub in sub_entries.flatten() {
                        let sub_path = sub.path();
                        if sub_path
                            .extension()
                            .map_or(false, |e| e == "csproj" || e == "sln")
                        {
                            if sub_path.extension().unwrap() == "sln" {
                                let ws = graphenium::extract::csharp_project::CSharpWorkspace::parse_solution(&sub_path);
                                println!("Solution: {}", sub_path.display());
                                for (_p, proj) in &ws.projects {
                                    println!(
                                        "  Project: {} (ns: {})",
                                        proj.name, proj.root_namespace
                                    );
                                    for ref_path in &proj.project_references {
                                        println!("    -> depends on: {}", ref_path.display());
                                    }
                                }
                            } else {
                                let proj = graphenium::extract::csharp_project::CSharpWorkspace::parse_csproj(&sub_path);
                                println!("Project: {} (ns: {})", proj.name, proj.root_namespace);
                                for ref_path in &proj.project_references {
                                    println!("  -> depends on: {}", ref_path.display());
                                }
                            }
                            csharp_found = true;
                        }
                    }
                }
            }
        }
    }
    if !csharp_found {
        println!("  (none found)");
    }
    Ok(())
}

/// Load graph and print test targets from CI extraction (`graph test-map`).
fn cmd_graph_test_map(graph_path: &Path) -> graphenium::Result<()> {
    let graph = load_graph(graph_path)?;
    let root = graph
        .metadata
        .project_root
        .as_deref()
        .map(Path::new)
        .unwrap_or_else(|| Path::new("."));

    let ci_files = discover_ci_configs(root);
    if ci_files.is_empty() {
        println!("No CI configuration files found under {}", root.display());
        return Ok(());
    }
    for file in &ci_files {
        let content = match std::fs::read_to_string(file) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("warn: could not read {}: {e}", file.display());
                continue;
            }
        };
        let targets = ci::parse_ci_config(&file.to_string_lossy(), &content);
        for t in &targets {
            if !t.tested_files.is_empty() {
                println!("[test] {} ({:?})", t.name, t.kind);
                for tf in &t.tested_files {
                    println!("  tests {tf}");
                }
            }
        }
    }
    Ok(())
}

/// Helper: find known CI configuration files under a root directory.
fn discover_ci_configs(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();

    // Cargo.toml
    let cargo = root.join("Cargo.toml");
    if cargo.exists() {
        files.push(cargo);
    }
    // package.json
    let pkg = root.join("package.json");
    if pkg.exists() {
        files.push(pkg);
    }
    // Makefile
    let makefile = root.join("Makefile");
    if makefile.exists() {
        files.push(makefile);
    }
    // GitHub Actions workflow files
    let workflows = root.join(".github/workflows");
    if workflows.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&workflows) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path
                    .extension()
                    .map_or(false, |e| e == "yml" || e == "yaml")
                {
                    files.push(path);
                }
            }
        }
    }

    files
}

/// Print repository info from graph metadata (`doctor --repository`).
fn cmd_doctor_repository(graph_path: Option<&Path>) {
    let load_path = graph_path.unwrap_or_else(|| Path::new("graphenium-out/graph.json"));
    match load_graph(load_path) {
        Ok(graph) => {
            println!("Repository Information");
            println!("======================");
            println!(
                "  Project root ........... {}",
                graph
                    .metadata
                    .project_root
                    .as_deref()
                    .unwrap_or("(unknown)")
            );
            println!(
                "  Schema version ......... {}",
                graph
                    .metadata
                    .schema_version
                    .as_deref()
                    .unwrap_or("(unknown)")
            );
            println!(
                "  Graphenium version ..... {}",
                graph
                    .metadata
                    .graphenium_version
                    .as_deref()
                    .unwrap_or("(unknown)")
            );
            println!(
                "  Created at ............. {}",
                graph.metadata.created_at.as_deref().unwrap_or("(unknown)")
            );
            let modes = graph
                .metadata
                .extraction_modes
                .as_deref()
                .map(|m| m.join(", "))
                .unwrap_or_else(|| "(none)".to_string());
            println!("  Extraction modes ....... {modes}");
            let langs = graph
                .metadata
                .languages
                .as_deref()
                .map(|l| l.join(", "))
                .unwrap_or_else(|| "(none)".to_string());
            println!("  Languages .............. {langs}");
            println!("  AST only ............... {}", graph.metadata.ast_only);
            println!("  Nodes .................. {}", graph.node_count());
            println!("  Edges .................. {}", graph.edge_count());
        }
        Err(e) => {
            eprintln!(
                "[graphenium] Could not load graph at {}: {e}",
                load_path.display()
            );
        }
    }
}
