/// File-watch mode: rebuild the knowledge graph automatically on code changes.
///
/// ## Behaviour
///
/// | Change type          | Action                                          |
/// |----------------------|-------------------------------------------------|
/// | Code file modified   | Fast rebuild: detect → AST extract → build → cluster → JSON export + report |
/// | Non-code file changed| Write `graphenium-out/needs_update` flag + print notification |
/// | Both in one batch    | Run fast rebuild AND write flag                 |
///
/// The `graphenium-out/` directory and hidden/skip directories are excluded from
/// the watcher so that writes made by the rebuild itself do not re-trigger
/// another rebuild.
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebouncedEvent};

use crate::analyze;
use crate::build;
use crate::cluster::{self, ClusterOptions};
use crate::detect::{self, classify, DetectOptions};
use crate::export;
use crate::export::json;
use crate::extract::{self, ExtractOptions};
use crate::model::{FileType, GrapheniumGraph};
use crate::report::{self, ReportInput};

// ── Public entry point ────────────────────────────────────────────────────────

/// Start watching `root` for file-system changes.
///
/// Blocks until the watcher is torn down (process receives a signal or the
/// parent tokio runtime drops the task).  Intended to be called inside
/// `tokio::task::spawn_blocking`.
pub fn watch(root: &Path, debounce_secs: f64, incremental: bool) -> crate::Result<()> {
    let root = root.to_path_buf();
    let out_dir = root.join("graphenium-out");

    let timeout = Duration::from_secs_f64(debounce_secs.max(0.1));

    // Use mpsc::Sender directly — notify-debouncer-mini implements
    // DebounceEventHandler for std::sync::mpsc::Sender.
    let (tx, rx) = mpsc::channel();

    let mut debouncer =
        new_debouncer(timeout, tx).map_err(|e| crate::GrapheniumError::Watch(e.to_string()))?;

    debouncer
        .watcher()
        .watch(&root, RecursiveMode::Recursive)
        .map_err(|e| crate::GrapheniumError::Watch(e.to_string()))?;

    eprintln!(
        "[graphenium] Watching {} (debounce {:.1}s) — Ctrl+C to stop",
        root.display(),
        debounce_secs
    );

    // Initial build so the user has a graph immediately.
    eprintln!("[graphenium] Running initial build...");
    full_rebuild_and_log(&root, &out_dir, Instant::now());

    loop {
        match rx.recv() {
            Ok(Ok(events)) => {
                handle_events(&root, &out_dir, &events, incremental);
            }
            Ok(Err(e)) => {
                eprintln!("[graphenium] watcher error: {e}");
            }
            // Channel disconnected — debouncer was dropped; exit cleanly.
            Err(_) => break,
        }
    }

    Ok(())
}

// ── Event handling ────────────────────────────────────────────────────────────

fn handle_events(root: &Path, out_dir: &Path, events: &[DebouncedEvent], incremental: bool) {
    let mut code_paths: Vec<&Path> = Vec::new();
    let mut non_code_paths: Vec<&Path> = Vec::new();

    for event in events {
        let path = &event.path;

        if should_skip(path, out_dir) {
            continue;
        }

        match classify::classify_extension(path) {
            Some(FileType::Code) => code_paths.push(path),
            Some(_) => non_code_paths.push(path),
            None => {} // unrecognised extension — ignore
        }
    }

    if code_paths.is_empty() && non_code_paths.is_empty() {
        return;
    }

    // Print what triggered this batch.
    for p in &code_paths {
        eprintln!("[graphenium] changed (code): {}", p.display());
    }
    for p in &non_code_paths {
        eprintln!("[graphenium] changed (non-code): {}", p.display());
    }

    if !code_paths.is_empty() {
        if incremental {
            incremental_rebuild(root, out_dir, &code_paths);
        } else {
            full_rebuild_and_log(root, out_dir, Instant::now());
        }
    }

    if !non_code_paths.is_empty() {
        write_needs_update(out_dir);
    }
}

// ── Path filtering ────────────────────────────────────────────────────────────

/// Returns `true` if an event path should be ignored.
///
/// Ignored paths:
/// - Anything inside the `graphenium-out/` output directory.
/// - Paths whose components include a hidden entry (starts with `.`) or a
///   known skip directory (e.g. `node_modules`, `target`).
fn should_skip(path: &Path, out_dir: &Path) -> bool {
    if path.starts_with(out_dir) {
        return true;
    }
    path.components().any(|c| {
        if let std::path::Component::Normal(name) = c {
            if let Some(s) = name.to_str() {
                return s.starts_with('.') || classify::is_skip_dir(s);
            }
        }
        false
    })
}

// ── Build (AST-only, no semantic) ────────────────────────────────────────────

/// Full re-detect → AST extract → build → cluster → export JSON + report.
///
/// Used on initial startup and as fallback when incremental patching is
/// not applicable.  Semantic results are intentionally skipped to keep the
/// rebuild fast (< 2s for most projects).
fn full_rebuild(root: &Path, out_dir: &Path) -> crate::Result<(usize, usize, usize)> {
    // 1. Detect files
    let (files, corpus_warnings) = detect::detect(root, &DetectOptions::default())?;

    // 2. AST extraction (rayon-parallel, code files only)
    let ast_result = extract::extract_all(
        &files,
        &ExtractOptions {
            mode: extract::ExtractMode::default(),
            cache_manager: Some(std::sync::Arc::new(crate::cache::CacheManager::new(
                out_dir.join("cache"),
            ))),
        },
    );

    // 3. Build graph
    let (mut graph, _stats) = build::build_merged([ast_result]);
    graph.set_ast_only(true);

    // 4. Cluster
    let community_stats = cluster::cluster(&mut graph, &ClusterOptions::default());

    // 5. Analyze
    let analysis = analyze::analyze(&graph, &community_stats);

    // 6. Export JSON + report
    write_graph_output(
        root,
        out_dir,
        &graph,
        &community_stats,
        &analysis,
        &corpus_warnings,
    )?;

    Ok((
        graph.node_count(),
        graph.edge_count(),
        community_stats.len(),
    ))
}

/// Incrementally patch the graph: re-extract only changed files, remove
/// their old contributions, and insert the new ones. If the existing graph
/// cannot be loaded, falls back to a full rebuild.
fn incremental_rebuild(root: &Path, out_dir: &Path, changed_paths: &[&Path]) {
    let start = Instant::now();

    match try_incremental(root, out_dir, changed_paths) {
        Ok(stats) => {
            eprintln!(
                "[graphenium] Patched in {:.2}s — {} file(s), {} nodes removed, {} inserted, \
                 {} edges inserted",
                start.elapsed().as_secs_f64(),
                changed_paths.len(),
                stats.total_nodes_removed,
                stats.total_nodes_inserted,
                stats.total_edges_inserted,
            );
        }
        Err(e) => {
            eprintln!(
                "[graphenium] Incremental patch failed ({}), falling back to full rebuild...",
                e
            );
            full_rebuild_and_log(root, out_dir, start);
        }
    }
}

struct IncrementalStats {
    total_nodes_removed: usize,
    total_nodes_inserted: usize,
    total_edges_inserted: usize,
}

fn try_incremental(
    root: &Path,
    out_dir: &Path,
    changed_paths: &[&Path],
) -> crate::Result<IncrementalStats> {
    let json_path = out_dir.join("graph.json");
    let manifest_path = out_dir.join("manifest.json");

    // Load the existing graph and manifest.
    let mut graph = json::load_graph(&json_path)?;
    let mut manifest = crate::cache::Manifest::load(&manifest_path);
    let prev_node_count = graph.node_count();

    let mut total_nodes_removed = 0usize;
    let mut total_nodes_inserted = 0usize;
    let mut total_edges_inserted = 0usize;

    // Compute the full invalidation set: directly changed files plus any
    // files that import them (so cross-file resolution stays current).
    let changed_bufs: Vec<std::path::PathBuf> =
        changed_paths.iter().map(|p| p.to_path_buf()).collect();
    let paths_to_extract = manifest.invalidation_set(&changed_bufs);

    for path_str in &paths_to_extract {
        let file_path = std::path::Path::new(path_str);
        let file = detect::DetectedFile {
            file_type: FileType::Code,
            path: file_path.to_path_buf(),
        };
        let result = crate::cache::query::salsa_extract_file(file_path, FileType::Code);
        if result.is_empty() {
            continue; // unrecognised or empty file — skip
        }

        let source_file = path_str.to_string();
        let stats = graph.replace_file_extraction(&source_file, &result);
        total_nodes_removed += stats.nodes_removed;
        total_nodes_inserted += stats.nodes_inserted;
        total_edges_inserted += stats.edges_inserted;

        // Record imports detected in this file for future invalidation.
        let imported: Vec<String> = result
            .edges
            .iter()
            .filter(|e| e.relation == "imports")
            .map(|e| e.target.clone())
            .collect();
        manifest.set_imports(file_path, imported);
        manifest.update(file_path);
    }

    // Persist the updated manifest.
    let _ = manifest.save(&manifest_path);

    let nodes_changed = total_nodes_removed.max(total_nodes_inserted);
    let pct_changed = if prev_node_count > 0 {
        (nodes_changed as f64 / prev_node_count as f64) * 100.0
    } else {
        100.0
    };

    // Re-cluster only when ≥5% of nodes changed.
    if pct_changed >= 5.0 || prev_node_count == 0 {
        let community_stats = cluster::cluster(&mut graph, &ClusterOptions::default());
        let analysis = analyze::analyze(&graph, &community_stats);
        let corpus_warnings: Vec<crate::detect::CorpusWarning> = Vec::new();
        write_graph_output(
            root,
            out_dir,
            &graph,
            &community_stats,
            &analysis,
            &corpus_warnings,
        )?;
    } else {
        // Write JSON only (no re-clustering, no report rewrite).
        std::fs::create_dir_all(out_dir)?;
        std::fs::write(&json_path, json::to_json(&graph)?)?;
    }

    Ok(IncrementalStats {
        total_nodes_removed,
        total_nodes_inserted,
        total_edges_inserted,
    })
}

fn full_rebuild_and_log(root: &Path, out_dir: &Path, start: Instant) {
    // Snapshot the old graph before rebuilding (for blast radius display)
    let old_json_path = out_dir.join("graph.json");
    let old_graph = export::json::load_graph(&old_json_path).ok();

    match full_rebuild(root, out_dir) {
        Ok((nodes, edges, communities)) => {
            eprintln!(
                "[graphenium] Full rebuild in {:.2}s — {} nodes, {} edges, {} communities",
                start.elapsed().as_secs_f64(),
                nodes,
                edges,
                communities
            );

            // Show blast radius if we have a snapshot to compare
            if let Ok(new_graph) = export::json::load_graph(&old_json_path) {
                if let Some(ref old) = old_graph {
                    let mut changed_labels: Vec<String> = Vec::new();
                    for n in new_graph.nodes() {
                        if !old.contains_node(&n.id) {
                            changed_labels.push(format!("+{}", n.label));
                        }
                    }
                    for n in old.nodes() {
                        if !new_graph.contains_node(&n.id) {
                            changed_labels.push(format!("-{}", n.label));
                        }
                    }
                    if !changed_labels.is_empty() {
                        eprintln!(
                            "[graphenium] Blast radius: {} symbols changed",
                            changed_labels.len()
                        );
                        if changed_labels.len() <= 20 {
                            for label in &changed_labels {
                                eprintln!("  {label}");
                            }
                        }
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("[graphenium] Full rebuild failed: {e}");
        }
    }
}

fn write_graph_output(
    root: &Path,
    out_dir: &Path,
    graph: &GrapheniumGraph,
    community_stats: &[crate::cluster::CommunityStats],
    analysis: &crate::analyze::AnalysisResult,
    corpus_warnings: &[crate::detect::CorpusWarning],
) -> crate::Result<()> {
    std::fs::create_dir_all(out_dir)?;
    let json_path = out_dir.join("graph.json");
    std::fs::write(&json_path, json::to_json(graph)?)?;

    let title = root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Knowledge Graph")
        .to_string();

    report::write_report(
        &ReportInput {
            graph,
            community_stats,
            analysis,
            corpus_warnings,
            input_tokens: 0,
            output_tokens: 0,
            title,
        },
        out_dir,
    )?;
    Ok(())
}

// ── needs_update flag ─────────────────────────────────────────────────────────

/// Write `graphenium-out/needs_update` to signal that non-code files (docs,
/// PDFs, images) have changed and a full `gm run` is needed to
/// incorporate them via semantic extraction.
fn write_needs_update(out_dir: &Path) {
    let flag = out_dir.join("needs_update");
    if let Err(e) = std::fs::create_dir_all(out_dir).and_then(|_| std::fs::write(&flag, b"")) {
        eprintln!("[graphenium] warn: could not write needs_update flag: {e}");
    } else {
        eprintln!(
            "[graphenium] Non-code files changed — run `gm run` for full semantic update.\n\
             [graphenium] Flag: {}",
            flag.display()
        );
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn path(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    // ── should_skip ───────────────────────────────────────────────────────────

    #[test]
    fn skip_output_dir() {
        let out = path("/project/graphenium-out");
        assert!(should_skip(
            &path("/project/graphenium-out/graph.json"),
            &out
        ));
        assert!(should_skip(
            &path("/project/graphenium-out/cache/abc.json"),
            &out
        ));
    }

    #[test]
    fn skip_hidden_file() {
        let out = path("/project/graphenium-out");
        assert!(should_skip(&path("/project/.env"), &out));
        assert!(should_skip(&path("/project/.git/HEAD"), &out));
    }

    #[test]
    fn skip_known_dirs() {
        let out = path("/project/graphenium-out");
        assert!(should_skip(&path("/project/node_modules/lib.js"), &out));
        assert!(should_skip(&path("/project/target/debug/main"), &out));
        assert!(should_skip(&path("/project/__pycache__/mod.pyc"), &out));
    }

    #[test]
    fn keep_normal_files() {
        let out = path("/project/graphenium-out");
        assert!(!should_skip(&path("/project/src/main.rs"), &out));
        assert!(!should_skip(&path("/project/README.md"), &out));
        assert!(!should_skip(&path("/project/docs/guide.md"), &out));
    }

    // ── fast_rebuild end-to-end ────────────────────────────────────────────────

    #[test]
    fn rebuild_creates_output_files() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let out_dir = root.join("graphenium-out");

        // Write a minimal Python file so the extractor has something to work with.
        std::fs::write(root.join("main.py"), b"def hello(): pass\n").unwrap();

        let result = full_rebuild(root, &out_dir);
        assert!(result.is_ok(), "rebuild failed: {:?}", result.err());
        assert!(
            out_dir.join("graph.json").exists(),
            "graph.json not written"
        );
        assert!(
            out_dir.join("GRAPH_REPORT.md").exists(),
            "report not written"
        );
    }

    #[test]
    fn rebuild_on_empty_dir_does_not_panic() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let out_dir = root.join("graphenium-out");
        // No source files — should succeed with an empty graph.
        let result = full_rebuild(root, &out_dir);
        assert!(result.is_ok());
    }

    // ── write_needs_update ────────────────────────────────────────────────────

    #[test]
    fn needs_update_flag_written() {
        let tmp = TempDir::new().unwrap();
        let out_dir = tmp.path().join("graphenium-out");
        write_needs_update(&out_dir);
        assert!(out_dir.join("needs_update").exists());
    }

    #[test]
    fn needs_update_creates_out_dir() {
        let tmp = TempDir::new().unwrap();
        let out_dir = tmp.path().join("nested").join("graphenium-out");
        write_needs_update(&out_dir);
        assert!(out_dir.join("needs_update").exists());
    }
}
