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

use std::collections::HashMap;

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
