/// mtime-based manifest for incremental file detection.
///
/// The manifest is persisted as `<out_dir>/manifest.json` and maps each
/// tracked file path (forward-slash, relative or absolute) to its last-seen
/// modification time in UNIX seconds.
///
/// Typical incremental-update flow:
/// 1. Load the manifest from disk.
/// 2. Walk the corpus; call `is_changed(path)` for each file.
/// 3. Extract only the changed files.
/// 4. Call `update(path)` for every file that was (re-)extracted successfully.
/// 5. Call `prune(existing)` to remove stale entries.
/// 6. Save the manifest back to disk.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

// ── Public types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Manifest {
    /// `normalized_path → mtime_unix_secs`
    entries: HashMap<String, u64>,
}

// ── Public API ─────────────────────────────────────────────────────────────────

impl Manifest {
    pub fn new() -> Self {
        Self::default()
    }

    /// Load the manifest from `path`.  Returns an empty manifest if the file
    /// is missing or malformed (both are treated as "everything is new").
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Atomically write the manifest to `path`.
    pub fn save(&self, path: &Path) -> crate::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, &content)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    /// Returns `true` if the file has changed since the manifest was last
    /// updated, or if the file is not yet tracked.
    ///
    /// A file that cannot be stat'd (e.g. deleted) is treated as changed so
    /// the caller can decide what to do with it.
    pub fn is_changed(&self, path: &Path) -> bool {
        let key = normalize(path);
        match self.entries.get(&key) {
            Some(&recorded) => mtime_secs(path).map_or(true, |t| t != recorded),
            None => true, // new file
        }
    }

    /// Record the current mtime for `path`.  Call this after a successful
    /// extraction so the next run skips the file if it hasn't changed.
    pub fn update(&mut self, path: &Path) {
        if let Some(t) = mtime_secs(path) {
            self.entries.insert(normalize(path), t);
        }
    }

    /// Remove entries whose paths are not in `existing`.  Keeps the manifest
    /// from growing unboundedly when files are deleted from the corpus.
    pub fn prune(&mut self, existing: &[PathBuf]) {
        let keys: std::collections::HashSet<String> =
            existing.iter().map(|p| normalize(p)).collect();
        self.entries.retain(|k, _| keys.contains(k));
    }

    /// Number of entries currently tracked.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────────

/// Normalise a path to a forward-slash string for cross-platform consistency.
fn normalize(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Return the file's modification time as UNIX seconds, or `None` on error.
fn mtime_secs(path: &Path) -> Option<u64> {
    std::fs::metadata(path)
        .ok()?
        .modified()
        .ok()?
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tempfile::TempDir;

    fn write_file(dir: &Path, name: &str, content: &[u8]) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, content).unwrap();
        p
    }

    #[test]
    fn new_file_is_changed() {
        let tmp = TempDir::new().unwrap();
        let p = write_file(tmp.path(), "a.py", b"x=1");
        let m = Manifest::new();
        assert!(m.is_changed(&p));
    }

    #[test]
    fn after_update_not_changed() {
        let tmp = TempDir::new().unwrap();
        let p = write_file(tmp.path(), "a.py", b"x=1");
        let mut m = Manifest::new();
        m.update(&p);
        assert!(!m.is_changed(&p));
    }

    #[test]
    fn save_and_load_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let p = write_file(tmp.path(), "a.py", b"x=1");
        let manifest_path = tmp.path().join("manifest.json");

        let mut m = Manifest::new();
        m.update(&p);
        m.save(&manifest_path).unwrap();

        let m2 = Manifest::load(&manifest_path);
        assert!(!m2.is_changed(&p));
    }

    #[test]
    fn missing_manifest_returns_empty() {
        let m = Manifest::load(Path::new("/nonexistent/manifest.json"));
        assert!(m.is_empty());
    }

    #[test]
    fn prune_removes_stale_entries() {
        let tmp = TempDir::new().unwrap();
        let p1 = write_file(tmp.path(), "a.py", b"x=1");
        let p2 = write_file(tmp.path(), "b.py", b"y=2");

        let mut m = Manifest::new();
        m.update(&p1);
        m.update(&p2);
        assert_eq!(m.len(), 2);

        // Prune: only p1 remains in the corpus
        m.prune(&[p1.clone()]);
        assert_eq!(m.len(), 1);
        assert!(!m.is_changed(&p1));
        assert!(m.is_changed(&p2)); // pruned → treated as new
    }

    #[test]
    fn missing_file_is_treated_as_changed() {
        let mut m = Manifest::new();
        let ghost = Path::new("/no/such/file.py");
        m.entries.insert(normalize(ghost), 999_999_999);
        // Can't stat it → changed
        assert!(m.is_changed(ghost));
    }

    #[test]
    fn update_overwrites_old_mtime() {
        let tmp = TempDir::new().unwrap();
        let p = write_file(tmp.path(), "a.py", b"v1");
        let mut m = Manifest::new();
        m.update(&p);
        // Record a deliberately wrong old timestamp
        m.entries.insert(normalize(&p), 0);
        assert!(m.is_changed(&p)); // stale entry → changed

        // Re-update with correct mtime
        m.update(&p);
        assert!(!m.is_changed(&p));
    }
}
