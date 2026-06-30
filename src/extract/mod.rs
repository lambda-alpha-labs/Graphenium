pub mod ci;
pub mod config;
pub mod cross_file;
pub mod go;
pub mod import_handlers;
pub mod rust_lang;
pub mod walker;

use rayon::prelude::*;

use crate::detect::DetectedFile;
use crate::model::{ExtractionResult, FileType};

/// Controls how aggressively the AST extractor infers relationships.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ExtractMode {
    /// Standard extraction: only well-evidenced edges.
    #[default]
    Standard,
    /// Deep mode: also infer structural relationships from naming conventions.
    Deep,
}

/// Options for the AST extraction phase.
#[derive(Debug, Clone, Default)]
pub struct ExtractOptions {
    pub mode: ExtractMode,
    /// Optional path to the cache directory (graphenium-out/cache).
    /// When set, extract_file will skip tree-sitter parsing on cache hits.
    pub cache_dir: Option<std::path::PathBuf>,
}

/// Extract AST structure from all code files in `files`, using rayon for
/// parallelism, then run Python cross-file import resolution.
///
/// Non-code files (Document, Paper, Image) are skipped; they are handled by
/// the semantic extractor in Phase 9.
pub fn extract_all(files: &[DetectedFile], opts: &ExtractOptions) -> ExtractionResult {
    let code_files: Vec<&DetectedFile> = files
        .iter()
        .filter(|f| f.file_type == FileType::Code)
        .collect();

    let total = code_files.len();
    if total >= 500 {
        eprintln!("[graphenium] Extracting AST from {total} files...");
    }

    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    let progress = Arc::new(AtomicUsize::new(0));

    let results: Vec<ExtractionResult> = code_files
        .par_iter()
        .map(|f| {
            let res = extract_file(f, opts);
            let count = progress.fetch_add(1, Ordering::SeqCst) + 1;
            if count % 500 == 0 || count == total {
                eprintln!("[graphenium] Extracting AST: {count} / {total} files completed...");
            }
            res
        })
        .collect();

    let mut combined = ExtractionResult::merge_all(results);

    // Python-only post-processing: upgrade `imports` edges to `uses` edges.
    cross_file::resolve_python_imports(&mut combined);

    combined
}

/// Extract AST structure from a single file.  Returns an empty result on
/// read failure or unsupported extension.
pub fn extract_file(file: &DetectedFile, opts: &ExtractOptions) -> ExtractionResult {
    let Ok(source) = std::fs::read(&file.path) else {
        return ExtractionResult::new();
    };

    // Try AST cache when cache_dir is set
    if let Some(ref cache_dir) = opts.cache_dir {
        if let Ok(hash) = crate::cache::file_hash(&file.path) {
            if let Some(cached) = crate::cache::load_ast_cached(cache_dir, &hash) {
                return cached;
            }
        }
    }

    let path_str = file.path.to_string_lossy();
    let ext = file
        .path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let result = dispatch(&source, &ext, &path_str);

    // Save to AST cache when cache_dir is set
    if let Some(ref cache_dir) = opts.cache_dir {
        if !result.is_empty() {
            if let Ok(hash) = crate::cache::file_hash(&file.path) {
                let _ = crate::cache::save_ast_cached(cache_dir, &hash, &result);
            }
        }
    }

    result
}

/// Extension-dispatch: choose the right extractor for the given extension.
pub fn dispatch(source: &[u8], ext: &str, file_path: &str) -> ExtractionResult {
    match ext {
        // Rust and Go have custom extractors that understand their unique constructs.
        "rs" => rust_lang::extract(source, file_path),
        "go" => go::extract(source, file_path),

        // All other supported languages use the generic walker.
        _ => {
            if let Some(cfg) = config::config_for_extension(ext) {
                walker::walk_file(source, &cfg, file_path)
            } else {
                ExtractionResult::new()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[cfg(feature = "lang-python")]
    #[test]
    fn extract_python_file() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(b"class Foo:\n    def bar(self): pass\n")
            .unwrap();
        // Rename with .py extension so the dispatcher picks it up
        let py_path = f.path().with_extension("py");
        std::fs::copy(f.path(), &py_path).unwrap();

        let r = dispatch(
            &std::fs::read(&py_path).unwrap(),
            "py",
            &py_path.to_string_lossy(),
        );
        std::fs::remove_file(&py_path).ok();

        assert!(!r.nodes.is_empty(), "should extract nodes from Python file");
    }

    #[cfg(feature = "lang-rust")]
    #[test]
    fn extract_rust_file() {
        let src = b"struct Counter { val: u32 }\nimpl Counter {\n    fn inc(&mut self) { self.val += 1; }\n}\n";
        let r = dispatch(src, "rs", "counter.rs");

        assert!(
            r.nodes.iter().any(|n| n.label == "Counter"),
            "missing struct"
        );
        assert!(r.nodes.iter().any(|n| n.label == "inc"), "missing method");
        assert!(
            r.nodes
                .iter()
                .filter(|n| n.label == "Counter" || n.label == "inc")
                .all(|n| n.source_location.starts_with('L')),
            "expected extracted symbols to carry source spans"
        );
    }

    #[test]
    fn unsupported_extension_returns_empty() {
        let r = dispatch(b"data", "bin", "data.bin");
        assert!(r.is_empty());
    }
}
