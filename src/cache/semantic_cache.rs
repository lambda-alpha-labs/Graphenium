/// High-level semantic-extraction cache.
///
/// Wraps the low-level [`super::file_hash`] / [`super::load_cached`] /
/// [`super::save_cached`] primitives to provide a batch-oriented interface
/// suited to the semantic extraction orchestrator in Phase 9.
///
/// Typical usage:
/// ```text
/// let (hits, misses) = check_semantic_cache(&files, cache_dir);
/// // hits  → already extracted; use directly
/// // misses → run through Claude; then call store() for each
/// for (path, hash) in misses {
///     let result = claude_extract(&path);
///     store(cache_dir, &hash, &result).ok();
/// }
/// ```
use std::path::{Path, PathBuf};

use crate::model::ExtractionResult;

use super::{file_hash, load_cached, save_cached};

// ── Public types ───────────────────────────────────────────────────────────────

/// A cache hit: the file's path, its content hash, and the cached result.
#[derive(Debug)]
pub struct CacheHit {
    pub path: PathBuf,
    pub hash: String,
    pub result: ExtractionResult,
}

/// A cache miss: the file's path and its content hash (may be empty if the
/// file could not be read).
#[derive(Debug)]
pub struct CacheMiss {
    pub path: PathBuf,
    /// SHA256 hex digest, or `""` if `file_hash` failed.
    pub hash: String,
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Split `files` into cache hits and misses against `cache_dir`.
///
/// Files that cannot be hashed (e.g. permission error) are returned as misses
/// with an empty hash, and will not be cached on the next store call.
pub fn check_semantic_cache(
    files: &[PathBuf],
    cache_dir: &Path,
) -> (Vec<CacheHit>, Vec<CacheMiss>) {
    let mut hits = Vec::new();
    let mut misses = Vec::new();

    for path in files {
        let hash = match file_hash(path) {
            Ok(h) => h,
            Err(_) => {
                misses.push(CacheMiss {
                    path: path.clone(),
                    hash: String::new(),
                });
                continue;
            }
        };

        match load_cached(cache_dir, &hash) {
            Some(result) => hits.push(CacheHit {
                path: path.clone(),
                hash,
                result,
            }),
            None => misses.push(CacheMiss {
                path: path.clone(),
                hash,
            }),
        }
    }

    (hits, misses)
}

/// Persist a freshly-extracted result for future cache lookups.
///
/// A miss with `hash == ""` (file unreadable at check time) is silently
/// skipped since there is no stable key to store under.
pub fn store(cache_dir: &Path, hash: &str, result: &ExtractionResult) -> crate::Result<()> {
    if hash.is_empty() {
        return Ok(());
    }
    save_cached(cache_dir, hash, result)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{FileType, Node};
    use tempfile::TempDir;

    fn make_result(label: &str) -> ExtractionResult {
        let mut r = ExtractionResult::new();
        r.nodes
            .push(Node::new(label, label, FileType::Code, "f.rs"));
        r
    }

    fn write_file(dir: &Path, name: &str, content: &[u8]) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, content).unwrap();
        p
    }

    #[test]
    fn all_misses_when_cache_empty() {
        let tmp = TempDir::new().unwrap();
        let cache_dir = tmp.path().join("cache");
        let f = write_file(tmp.path(), "a.py", b"x=1");

        let (hits, misses) = check_semantic_cache(&[f], &cache_dir);
        assert!(hits.is_empty());
        assert_eq!(misses.len(), 1);
        assert!(!misses[0].hash.is_empty()); // hash computed successfully
    }

    #[test]
    fn hit_after_store() {
        let tmp = TempDir::new().unwrap();
        let cache_dir = tmp.path().join("cache");
        let f = write_file(tmp.path(), "a.py", b"x=1");

        // First pass: miss
        let (_, misses) = check_semantic_cache(&[f.clone()], &cache_dir);
        assert_eq!(misses.len(), 1);

        // Store the result
        let result = make_result("Foo");
        store(&cache_dir, &misses[0].hash, &result).unwrap();

        // Second pass: hit
        let (hits, misses) = check_semantic_cache(&[f], &cache_dir);
        assert_eq!(hits.len(), 1);
        assert!(misses.is_empty());
        assert_eq!(hits[0].result.nodes[0].label, "Foo");
    }

    #[test]
    fn changed_file_is_a_miss() {
        let tmp = TempDir::new().unwrap();
        let cache_dir = tmp.path().join("cache");
        let f = write_file(tmp.path(), "a.py", b"v1");

        // Cache the v1 result
        let (_, misses) = check_semantic_cache(&[f.clone()], &cache_dir);
        store(&cache_dir, &misses[0].hash, &make_result("V1")).unwrap();

        // Mutate the file
        std::fs::write(&f, b"v2").unwrap();

        // Should be a miss because hash changed
        let (hits, misses) = check_semantic_cache(&[f], &cache_dir);
        assert!(hits.is_empty());
        assert_eq!(misses.len(), 1);
    }

    #[test]
    fn unreadable_file_is_miss_with_empty_hash() {
        let tmp = TempDir::new().unwrap();
        let cache_dir = tmp.path().join("cache");
        let ghost = tmp.path().join("ghost.py"); // does not exist

        let (hits, misses) = check_semantic_cache(&[ghost], &cache_dir);
        assert!(hits.is_empty());
        assert_eq!(misses.len(), 1);
        assert!(misses[0].hash.is_empty());
    }

    #[test]
    fn store_with_empty_hash_is_noop() {
        let tmp = TempDir::new().unwrap();
        let cache_dir = tmp.path().join("cache");
        // Should not panic or error
        store(&cache_dir, "", &make_result("X")).unwrap();
        // Cache dir not even created since hash is empty and we return early
        // (save_cached never called)
    }

    #[test]
    fn multiple_files_split_correctly() {
        let tmp = TempDir::new().unwrap();
        let cache_dir = tmp.path().join("cache");

        let f1 = write_file(tmp.path(), "a.py", b"a");
        let f2 = write_file(tmp.path(), "b.py", b"b");
        let f3 = write_file(tmp.path(), "c.py", b"c");

        // Cache only f1
        let (_, misses) = check_semantic_cache(&[f1.clone()], &cache_dir);
        store(&cache_dir, &misses[0].hash, &make_result("A")).unwrap();

        let (hits, misses) = check_semantic_cache(&[f1, f2, f3], &cache_dir);
        assert_eq!(hits.len(), 1);
        assert_eq!(misses.len(), 2);
        assert_eq!(hits[0].result.nodes[0].label, "A");
    }
}
