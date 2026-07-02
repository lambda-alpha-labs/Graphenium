/// File-based extraction cache.
///
/// Cache layout inside `<out_dir>/cache/`:
/// ```text
/// cache/
///   <sha256>.json       ← serialised ExtractionResult (atomic write)
/// ```
///
/// The SHA256 hash of the file's raw bytes is used as the cache key, so a
/// file is automatically reused if its content is unchanged, and a new entry
/// is written whenever the content changes.
pub mod manager;
pub mod manifest;
pub mod query;
pub mod semantic_cache;

pub use self::manager::CacheManager;
pub use manifest::Manifest;
pub use semantic_cache::check_semantic_cache;

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::model::ExtractionResult;

// ── Low-level primitives ───────────────────────────────────────────────────────

/// Compute the SHA256 hex digest of the bytes in `path`.
pub fn file_hash(path: &Path) -> crate::Result<String> {
    let bytes = std::fs::read(path)?;
    let digest = Sha256::digest(&bytes);
    Ok(hex::encode(digest))
}

/// Canonical path for a general cache entry (semantic extraction).
pub fn cache_path(cache_dir: &Path, hash: &str) -> PathBuf {
    cache_dir.join(format!("{hash}.json"))
}

/// Canonical path for an AST cache entry, kept under a dedicated subdirectory
/// so it does not collide with semantic/LLM cache entries.
pub fn ast_cache_path(cache_dir: &Path, hash: &str) -> PathBuf {
    cache_dir.join("ast").join(format!("{hash}.json"))
}

/// Load a cached AST [`ExtractionResult`] by hash. Returns `None` on cache
/// miss or if the stored JSON is malformed.
pub fn load_ast_cached(cache_dir: &Path, hash: &str) -> Option<ExtractionResult> {
    let path = ast_cache_path(cache_dir, hash);
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Persist an AST [`ExtractionResult`] under `hash` using atomic write.
pub fn save_ast_cached(
    cache_dir: &Path,
    hash: &str,
    result: &ExtractionResult,
) -> crate::Result<()> {
    let dir = cache_dir.join("ast");
    std::fs::create_dir_all(&dir)?;
    let target = dir.join(format!("{hash}.json"));
    let tmp = target.with_extension("tmp");
    let content = serde_json::to_string(result)?;
    std::fs::write(&tmp, &content)?;
    std::fs::rename(&tmp, &target)?;
    Ok(())
}

/// Load a cached [`ExtractionResult`] by hash.  Returns `None` on cache miss
/// or if the stored JSON is malformed.
pub fn load_cached(cache_dir: &Path, hash: &str) -> Option<ExtractionResult> {
    let content = std::fs::read_to_string(cache_path(cache_dir, hash)).ok()?;
    serde_json::from_str(&content).ok()
}

/// Persist `result` under `hash` using an atomic temp-then-rename write so
/// that a process crash never leaves a partial file in the cache.
pub fn save_cached(cache_dir: &Path, hash: &str, result: &ExtractionResult) -> crate::Result<()> {
    std::fs::create_dir_all(cache_dir)?;
    let target = cache_path(cache_dir, hash);
    let tmp = target.with_extension("tmp");
    let content = serde_json::to_string(result)?;
    std::fs::write(&tmp, &content)?;
    std::fs::rename(&tmp, &target)?;
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ExtractionResult, FileType, Node};
    use tempfile::TempDir;

    fn sample_result() -> ExtractionResult {
        let mut r = ExtractionResult::new();
        r.nodes
            .push(Node::new("a_foo", "Foo", FileType::Code, "a.rs"));
        r.input_tokens = 42;
        r
    }

    // ── file_hash ─────────────────────────────────────────────────────────────

    #[test]
    fn same_file_same_hash() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("f.txt");
        std::fs::write(&p, b"hello world").unwrap();
        let h1 = file_hash(&p).unwrap();
        let h2 = file_hash(&p).unwrap();
        assert_eq!(h1, h2);
    }

    #[test]
    fn different_content_different_hash() {
        let tmp = TempDir::new().unwrap();
        let p1 = tmp.path().join("a.txt");
        let p2 = tmp.path().join("b.txt");
        std::fs::write(&p1, b"aaa").unwrap();
        std::fs::write(&p2, b"bbb").unwrap();
        assert_ne!(file_hash(&p1).unwrap(), file_hash(&p2).unwrap());
    }

    #[test]
    fn hash_is_64_hex_chars() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("x.txt");
        std::fs::write(&p, b"data").unwrap();
        let h = file_hash(&p).unwrap();
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn missing_file_returns_error() {
        let r = file_hash(Path::new("/nonexistent/file.txt"));
        assert!(r.is_err());
    }

    // ── load_cached / save_cached ─────────────────────────────────────────────

    #[test]
    fn save_then_load_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let r = sample_result();
        save_cached(tmp.path(), "abc123", &r).unwrap();
        let loaded = load_cached(tmp.path(), "abc123").unwrap();
        assert_eq!(loaded.nodes.len(), 1);
        assert_eq!(loaded.nodes[0].id, "a_foo");
        assert_eq!(loaded.input_tokens, 42);
    }

    #[test]
    fn cache_miss_returns_none() {
        let tmp = TempDir::new().unwrap();
        assert!(load_cached(tmp.path(), "doesnotexist").is_none());
    }

    #[test]
    fn save_creates_parent_dir() {
        let tmp = TempDir::new().unwrap();
        let nested = tmp.path().join("a").join("b").join("c");
        let r = sample_result();
        save_cached(&nested, "h1", &r).unwrap();
        assert!(cache_path(&nested, "h1").exists());
    }

    #[test]
    fn overwrite_updates_entry() {
        let tmp = TempDir::new().unwrap();
        let r1 = sample_result();
        save_cached(tmp.path(), "key", &r1).unwrap();

        let mut r2 = ExtractionResult::new();
        r2.input_tokens = 99;
        save_cached(tmp.path(), "key", &r2).unwrap();

        let loaded = load_cached(tmp.path(), "key").unwrap();
        assert_eq!(loaded.input_tokens, 99);
    }

    #[test]
    fn corrupted_cache_entry_returns_none() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path()).unwrap();
        std::fs::write(cache_path(tmp.path(), "bad"), b"not json").unwrap();
        assert!(load_cached(tmp.path(), "bad").is_none());
    }
}
