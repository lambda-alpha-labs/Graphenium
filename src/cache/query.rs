//! Salsa-style demand-driven incremental computation engine for Graphenium.
//!
//! This module defines a Salsa-based query database that replaces Graphenium's
//! eager, batch-oriented extraction pipeline with a demand-driven incrementally
//! computed model.
//!
//! Database structure:
//!   SourceFile (input) -> parse_ast (tracked) -> resolve_imports (tracked)
//!                                              -> compute_local_subgraph (tracked)

use std::path::PathBuf;
use std::sync::Arc;

use crate::model::{ExtractionResult, FileType};

// ── Phase 1.2: Base Input ──────────────────────────────────────────────────

/// A tracked input representing a source file on disk.
/// When the file's content changes, Salsa marks all downstream queries as dirty.
#[salsa::input]
pub struct SourceFile {
    pub path: PathBuf,
    pub text: String,
    /// Categorized file type.
    pub file_type: FileType,
}

// ── Phase 2: Tracked Query Functions ───────────────────────────────────────

/// Parse raw source text into a local ExtractionResult.
/// Memoized by Salsa — if `text` is unchanged, the AST is never re-parsed.
#[salsa::tracked]
pub fn parse_ast(db: &dyn salsa::Database, file: SourceFile) -> ExtractionResult {
    let text = file.text(db);
    let path = file.path(db);
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let path_str = path.to_string_lossy();
    crate::extract::dispatch(text.as_bytes(), &ext, &path_str)
}

/// Extract imported symbol names from a file's AST.
#[salsa::tracked]
pub fn resolve_imports_query(db: &dyn salsa::Database, file: SourceFile) -> Arc<Vec<String>> {
    let ast = parse_ast(db, file);
    let imports: Vec<String> = ast
        .edges
        .iter()
        .filter(|e| e.relation == "imports")
        .map(|e| e.target.clone())
        .collect();
    Arc::new(imports)
}

/// Combine AST and resolved imports into a structured local subgraph.
#[salsa::tracked]
pub fn compute_local_subgraph(db: &dyn salsa::Database, file: SourceFile) -> ExtractionResult {
    parse_ast(db, file)
}

// ── Phase 4: Graph Materialization ─────────────────────────────────────────

// ── Phase 3: Salsa-backed extraction for watch.rs ──────────────────────────

use std::collections::HashMap;

thread_local! {
    /// Per-thread Salsa database for persistent memoization across event batches.
    /// Created lazily on first access by `salsa_extract_file`.
    static SALSA_DB: std::cell::RefCell<Option<salsa::DatabaseImpl>> =
        std::cell::RefCell::new(None);

    /// Per-thread map of known source file inputs, indexed by path.
    static SALSA_INPUTS: std::cell::RefCell<std::collections::HashMap<PathBuf, SourceFile>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Extract a file using Salsa's memoized tracked queries.
///
/// On first call, creates a persistent `salsa::DatabaseImpl` and a `SourceFile`
/// input. On subsequent calls with the same content, Salsa returns the cached
/// `parse_ast` result instantly without re-parsing. When content changes, Salsa
/// automatically re-runs only the affected queries.
///
/// Falls back to direct extraction when Salsa is unavailable (unlikely).
pub fn salsa_extract_file(path: &std::path::Path, file_type: FileType) -> ExtractionResult {
    // Read current file content
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return ExtractionResult::new(),
    };
    let path_buf = path.to_path_buf();

    // Phase 1: Get or create the Salsa database (separate RefCell from inputs)
    SALSA_DB.with(|db_cell| {
        let mut db_borrow = db_cell.borrow_mut();
        let db = db_borrow.get_or_insert_with(|| salsa::DatabaseImpl::new());

        // Phase 2: Check if the file is already known and if it changed
        let (is_new, is_changed) = SALSA_INPUTS.with(|inputs_cell| {
            let inputs = inputs_cell.borrow();
            match inputs.get(&path_buf) {
                None => (true, false),
                Some(existing) => {
                    let cached = existing.text(db);
                    (false, cached != text)
                }
            }
        });

        // Phase 3: Create or reuse the source file input
        SALSA_INPUTS.with(|inputs_cell| {
            let mut inputs = inputs_cell.borrow_mut();
            if is_new || is_changed {
                let sf = SourceFile::new(db, path_buf.clone(), text, file_type);
                inputs.insert(path_buf.clone(), sf);
            }
        });

        // Phase 4: Extract using the (possibly new) SourceFile
        SALSA_INPUTS.with(|inputs_cell| {
            let inputs = inputs_cell.borrow();
            let source_file = *inputs.get(&path_buf).unwrap();
            compute_local_subgraph(db, source_file)
        })
    })
}

/// Extract all files using Salsa-backed extraction with memoization.
///
/// On the first call, each file is parsed and cached. On subsequent calls,
/// Salsa re-parses only files whose content has changed. This is the Salsa
/// equivalent of `extract::extract_all()` for use in `full_rebuild`.
///
/// Files are processed in parallel via rayon for performance; the Salsa DB
/// is thread-local so each rayon worker gets its own DB instance.
pub fn salsa_extract_all(files: &[crate::detect::DetectedFile]) -> ExtractionResult {
    use rayon::prelude::*;

    let results: Vec<ExtractionResult> = files
        .par_iter()
        .filter(|f| f.file_type == FileType::Code)
        .map(|f| salsa_extract_file(&f.path, f.file_type.clone()))
        .collect();

    let mut merged = ExtractionResult::new();
    for r in results {
        merged.merge(r);
    }
    merged
}

/// Materialize the full workspace graph from the Salsa database.
pub fn materialize_full_graph(
    db: &salsa::DatabaseImpl,
    inputs: &HashMap<PathBuf, SourceFile>,
) -> crate::model::GrapheniumGraph {
    let mut graph = crate::model::GrapheniumGraph::new();
    for (_path, &source_file) in inputs {
        let local = compute_local_subgraph(db, source_file);
        let path_str = source_file.path(db).to_string_lossy().to_string();
        graph.replace_file_extraction(&path_str, &local);
    }
    graph
}
