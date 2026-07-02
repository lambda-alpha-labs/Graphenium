pub mod ci;
pub mod config;
pub mod cross_file;
pub mod csharp_project;
pub mod go;
pub mod import_handlers;
pub mod rust_lang;
pub mod walker;

use rayon::prelude::*;

use std::sync::Arc;

use crate::cache::CacheManager;
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
    /// Optional cache manager. When set, extract_file will skip tree-sitter
    /// parsing on cache hits and persist results on cache misses.
    pub cache_manager: Option<Arc<CacheManager>>,
}

/// Extract AST structure from all code files in `files`, using rayon for
/// parallelism, then run Python cross-file import resolution.
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

    let mut results: Vec<ExtractionResult> = code_files
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

    // Resolve cross-file references using Stack Graphs.
    let resolved_count = crate::resolver::resolve_cross_file_calls(&mut results, None);
    if resolved_count > 0 {
        eprintln!(
            "[graphenium] Stack Graphs: resolved {} cross-file references",
            resolved_count
        );
    }

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

    // Try AST cache when a cache manager is available
    if let Some(ref cache) = opts.cache_manager {
        if let Ok(hash) = crate::cache::file_hash(&file.path) {
            if let Some(cached) = cache.load_ast(&hash) {
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

    // Save to AST cache when a cache manager is available
    if let Some(ref cache) = opts.cache_manager {
        if !result.is_empty() {
            if let Ok(hash) = crate::cache::file_hash(&file.path) {
                let _ = cache.save_ast(&hash, &result);
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

    #[cfg(feature = "lang-csharp")]
    #[test]
    fn extract_csharp_file() {
        let src = b"namespace N \n{\n    public class Greeter \n    {\n        public string Greet() { return \"hi\"; }\n    }\n}";
        let r = dispatch(src, "cs", "Greeter.cs");

        // 1. Assert nodes are extracted
        assert!(!r.nodes.is_empty(), "C# extraction produced 0 nodes");

        // 2. Verify class node extraction
        let class_node = r
            .nodes
            .iter()
            .find(|n| n.label == "Greeter")
            .expect("C# class 'Greeter' was not extracted");
        assert_eq!(class_node.file_type, FileType::Code);
        assert!(
            !class_node.source_location.is_empty(),
            "Class source location is empty"
        );

        // 3. Verify method node extraction
        let method_node = r
            .nodes
            .iter()
            .find(|n| n.label == "Greet")
            .expect("C# method 'Greet' was not extracted");
        assert_eq!(method_node.file_type, FileType::Code);
        assert!(
            !method_node.source_location.is_empty(),
            "Method source location is empty"
        );

        // 4. Verify structural edge linking class to its method
        let has_method_edge = r.edges.iter().any(|e| e.relation == "method");
        assert!(has_method_edge, "Missing 'method' edge — C# extraction may not have linked class to method (expected at minimum a 'method' relation edge)");
    }

    #[test]
    fn unsupported_extension_returns_empty() {
        let r = dispatch(b"data", "bin", "data.bin");
        assert!(r.is_empty());
    }
}
