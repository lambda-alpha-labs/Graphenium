pub mod ci;
pub mod config;
pub mod cross_file;
pub mod csharp_project;
#[cfg(feature = "lang-go")]
pub mod go;
pub mod import_handlers;
pub mod lean;
pub mod lean_axioms;
#[cfg(feature = "lang-rust")]
pub mod rust_lang;
pub mod walker;

use rayon::prelude::*;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::cache::CacheManager;
use crate::detect::DetectedFile;
use crate::model::{ExtractionResult, FileType};

/// File stems that are not unique on their own and therefore must be qualified
/// with their project-relative directory when used as an ID prefix. Without
/// this, every `mod.rs` / `index.ts` / `__init__.py` in a repository collapses
/// into a single graph node.
const STRUCTURAL_MODULE_STEMS: &[&str] = &["mod", "lib", "main", "index", "__init__", "__main__"];

use crate::cache::ast_cache_key;

/// Derive a stable ID prefix from a file path.
///
/// Ordinary files keep their bare stem (e.g. `src/parser.rs` -> `parser`), which
/// preserves existing IDs. Structural module filenames are qualified with the
/// root-relative path (e.g. `src/extract/mod.rs` -> `src_extract_mod`) so that
/// same-named files in different directories remain distinct.
pub(crate) fn module_stem_from_path(file_path: &str, project_root: Option<&Path>) -> String {
    let path = Path::new(file_path);
    let file_stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(file_path);

    if !STRUCTURAL_MODULE_STEMS
        .iter()
        .any(|s| s.eq_ignore_ascii_case(file_stem))
    {
        return file_stem.to_string();
    }

    let relative = project_root
        .and_then(|root| path.strip_prefix(root).ok())
        .unwrap_or(path);
    let without_ext = relative.with_extension("");
    let normalized = without_ext
        .to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_matches('/')
        .to_string();

    if normalized.is_empty() {
        file_stem.to_string()
    } else {
        crate::model::make_id(&[&normalized])
    }
}

/// Record a short label -> node ID association without overwriting earlier
/// definitions. Duplicate labels become ambiguous and are ignored by the
/// intra-file call resolver instead of binding to an arbitrary definition.
pub(crate) fn register_symbol_label(
    map: &mut std::collections::HashMap<String, Vec<String>>,
    label: &str,
    id: &str,
) {
    let ids = map.entry(label.to_string()).or_default();
    if !ids.iter().any(|existing| existing == id) {
        ids.push(id.to_string());
    }
}

/// Return the node ID for `label` only when it is unambiguous within the file.
pub(crate) fn unique_symbol_id<'a>(
    map: &'a std::collections::HashMap<String, Vec<String>>,
    label: &str,
) -> Option<&'a str> {
    map.get(label)
        .filter(|ids| ids.len() == 1)
        .map(|ids| ids[0].as_str())
}

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
    /// Project root used to derive stable, unique module paths for languages
    /// whose symbols otherwise collide across same-named files (Lean).
    pub project_root: Option<PathBuf>,
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
            if count.is_multiple_of(500) || count == total {
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
            if let Some(cached) = cache.load_ast(&ast_cache_key(&hash)) {
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

    let result = dispatch_with_root(&source, &ext, &path_str, opts.project_root.as_deref());

    // Save to AST cache when a cache manager is available
    if let Some(ref cache) = opts.cache_manager {
        if !result.is_empty() {
            if let Ok(hash) = crate::cache::file_hash(&file.path) {
                let _ = cache.save_ast(&ast_cache_key(&hash), &result);
            }
        }
    }

    result
}

/// Extension-dispatch: choose the right extractor for the given extension.
pub fn dispatch(source: &[u8], ext: &str, file_path: &str) -> ExtractionResult {
    dispatch_with_root(source, ext, file_path, None)
}

/// Extension-dispatch with an optional project root for module-qualified IDs.
pub fn dispatch_with_root(
    source: &[u8],
    ext: &str,
    file_path: &str,
    project_root: Option<&Path>,
) -> ExtractionResult {
    let mut result = match ext {
        // Rust, Go, and Lean 4 have dedicated extractors.
        #[cfg(feature = "lang-rust")]
        "rs" => rust_lang::extract_with_root(source, file_path, project_root),
        #[cfg(feature = "lang-go")]
        "go" => go::extract_with_root(source, file_path, project_root),
        "lean" => lean::extract_with_root(source, file_path, project_root),

        // All other supported languages use the generic walker.
        _ => {
            if let Some(cfg) = config::config_for_extension(ext) {
                walker::walk_file_with_root(source, &cfg, file_path, project_root)
            } else {
                ExtractionResult::new()
            }
        }
    };

    // Persist only repo-relative source paths. Doing this at the dispatch
    // boundary covers every extractor and every AST-cache payload.
    normalize_extraction_paths(&mut result, file_path, project_root);
    result
}

/// Rewrite every node/edge `source_file` to a repo-relative path using the
/// canonical [`crate::detect::path::to_repo_relative`] helper.
pub fn normalize_extraction_paths(
    result: &mut ExtractionResult,
    file_path: &str,
    project_root: Option<&Path>,
) {
    let relative = crate::detect::path::to_repo_relative(Path::new(file_path), project_root);
    if relative.is_empty() {
        return;
    }
    for node in &mut result.nodes {
        node.source_file = relative.clone();
    }
    for edge in &mut result.edges {
        edge.source_file = relative.clone();
    }
}

/// Rewrite each node/edge's existing `source_file` relative to `project_root`.
/// Useful for extractors that emit per-node paths (C#/CI, semantic passes).
pub fn normalize_result_source_paths(result: &mut ExtractionResult, project_root: Option<&Path>) {
    for node in &mut result.nodes {
        if !node.source_file.is_empty() {
            node.source_file =
                crate::detect::path::to_repo_relative(Path::new(&node.source_file), project_root);
        }
    }
    for edge in &mut result.edges {
        if !edge.source_file.is_empty() {
            edge.source_file =
                crate::detect::path::to_repo_relative(Path::new(&edge.source_file), project_root);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "lang-python")]
    use std::io::Write;
    #[cfg(feature = "lang-python")]
    use tempfile::NamedTempFile;

    #[test]
    fn structural_module_stems_are_qualified_by_directory() {
        let root = Path::new("/project");
        assert_eq!(
            module_stem_from_path("/project/src/a/mod.rs", Some(root)),
            "src_a_mod"
        );
        assert_eq!(
            module_stem_from_path("/project/src/b/mod.rs", Some(root)),
            "src_b_mod"
        );
        assert_eq!(
            module_stem_from_path("/project/web/index.ts", Some(root)),
            "web_index"
        );
        assert_eq!(
            module_stem_from_path("/project/pkg/__init__.py", Some(root)),
            "pkg_init"
        );
    }

    #[test]
    fn ordinary_files_keep_bare_stem() {
        let root = Path::new("/project");
        assert_eq!(
            module_stem_from_path("/project/src/parser.rs", Some(root)),
            "parser"
        );
    }

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

    #[cfg(feature = "lang-csharp")]
    #[test]
    fn extract_csharp_inheritance_uses_canonical_node_ids() {
        let src =
            b"namespace N\n{\n    public class Foo : BaseClass, IMyInterface\n    {\n    }\n}";
        let r = dispatch(src, "cs", "Inheritance.cs");

        let class_id = crate::model::make_id(&["Inheritance", "Foo"]);
        assert!(
            r.nodes.iter().any(|n| n.id == class_id && n.label == "Foo"),
            "class node should use the canonical make_id form"
        );
        assert!(
            !r.nodes.iter().any(|n| n.id.contains("::")),
            "no fabricated file_path::kind::name node IDs should be emitted"
        );

        assert!(
            r.edges.iter().any(|e| e.relation == "inherits"
                && e.source == class_id
                && e.target == "baseclass"),
            "class base type should produce an inherits edge from the canonical class node"
        );
        assert!(
            r.edges.iter().any(|e| e.relation == "implements"
                && e.source == class_id
                && e.target == "imyinterface"),
            "I-prefixed base type should produce an implements edge"
        );
    }

    #[test]
    fn unsupported_extension_returns_empty() {
        let r = dispatch(b"data", "bin", "data.bin");
        assert!(r.is_empty());
    }
}
