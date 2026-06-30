/// Centralized cache directory abstraction for AST and semantic extraction caches.
///
/// Replaces ad-hoc path construction with a tested, single-responsibility struct.
/// Use `CacheManager::new(cache_root)` where `cache_root` is `graphenium-out/cache`.
use std::path::{Path, PathBuf};

use crate::model::ExtractionResult;

/// Manages disk-backed extraction caches under a single root directory.
///
/// Layout inside `root`:
/// ```text
/// root/
///   ast/       ← Tree-sitter AST extraction results, keyed by SHA256
///   semantic/  ← LLM semantic enrichment results, keyed by SHA256
/// ```
#[derive(Debug)]
pub struct CacheManager {
    root_dir: PathBuf,
}

impl CacheManager {
    /// Create a new manager rooted at `root_dir` (typically `graphenium-out/cache`).
    pub fn new(root_dir: PathBuf) -> Self {
        Self { root_dir }
    }

    // ── Directory accessors ──────────────────────────────────────────────────

    pub fn root(&self) -> &Path {
        &self.root_dir
    }

    pub fn ast_dir(&self) -> PathBuf {
        self.root_dir.join("ast")
    }

    pub fn semantic_dir(&self) -> PathBuf {
        self.root_dir.join("semantic")
    }

    // ── AST cache ─────────────────────────────────────────────────────────────

    /// Try to load a cached AST extraction result by content hash.
    pub fn load_ast(&self, hash: &str) -> Option<ExtractionResult> {
        let path = self.ast_dir().join(format!("{hash}.json"));
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    /// Persist an AST extraction result under its content hash.
    /// Uses atomic temp-then-rename to avoid partial writes on crash.
    pub fn save_ast(&self, hash: &str, result: &ExtractionResult) -> crate::Result<()> {
        std::fs::create_dir_all(self.ast_dir())?;
        let target = self.ast_dir().join(format!("{hash}.json"));
        let tmp = target.with_extension("tmp");
        let content = serde_json::to_string(result)?;
        std::fs::write(&tmp, &content)?;
        std::fs::rename(&tmp, &target)?;
        Ok(())
    }

    // ── Semantic cache ────────────────────────────────────────────────────────

    /// Try to load a cached semantic extraction result by content hash.
    pub fn load_semantic(&self, hash: &str) -> Option<ExtractionResult> {
        let path = self.semantic_dir().join(format!("{hash}.json"));
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    /// Persist a semantic extraction result under its content hash.
    pub fn save_semantic(&self, hash: &str, result: &ExtractionResult) -> crate::Result<()> {
        std::fs::create_dir_all(self.semantic_dir())?;
        let target = self.semantic_dir().join(format!("{hash}.json"));
        let tmp = target.with_extension("tmp");
        let content = serde_json::to_string(result)?;
        std::fs::write(&tmp, &content)?;
        std::fs::rename(&tmp, &target)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{FileType, Node};
    use tempfile::TempDir;

    fn sample_result() -> ExtractionResult {
        let mut r = ExtractionResult::new();
        r.nodes
            .push(Node::new("a_foo", "Foo", FileType::Code, "a.rs"));
        r.input_tokens = 42;
        r
    }

    #[test]
    fn save_then_load_ast_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let cm = CacheManager::new(tmp.path().to_path_buf());
        let r = sample_result();
        cm.save_ast("abc123", &r).unwrap();
        let loaded = cm.load_ast("abc123").unwrap();
        assert_eq!(loaded.nodes.len(), 1);
        assert_eq!(loaded.nodes[0].id, "a_foo");
    }

    #[test]
    fn miss_returns_none() {
        let tmp = TempDir::new().unwrap();
        let cm = CacheManager::new(tmp.path().to_path_buf());
        assert!(cm.load_ast("doesnotexist").is_none());
    }

    #[test]
    fn creates_ast_subdirectory() {
        let tmp = TempDir::new().unwrap();
        let cm = CacheManager::new(tmp.path().to_path_buf());
        let r = sample_result();
        cm.save_ast("h1", &r).unwrap();
        assert!(cm.ast_dir().join("h1.json").exists());
    }

    #[test]
    fn save_then_load_semantic_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let cm = CacheManager::new(tmp.path().to_path_buf());
        let r = sample_result();
        cm.save_semantic("xyz", &r).unwrap();
        let loaded = cm.load_semantic("xyz").unwrap();
        assert_eq!(loaded.input_tokens, 42);
    }
}
