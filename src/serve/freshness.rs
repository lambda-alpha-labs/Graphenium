//! Detect when a loaded graph.json is older than the serving binary or source tree.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Result of comparing graph file mtimes against the binary and project sources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StalenessReport {
    pub graph_path: PathBuf,
    pub is_stale: bool,
    pub reasons: Vec<String>,
}

/// Return filesystem mtime for `path`, if readable.
pub fn file_mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

/// True when `candidate` is strictly newer than `baseline`.
fn is_newer_than(candidate: SystemTime, baseline: SystemTime) -> bool {
    candidate > baseline
}

/// Walk `root` recursively and return the newest mtime among regular files.
fn newest_mtime_under(root: &Path) -> Option<(SystemTime, PathBuf)> {
    let mut best: Option<(SystemTime, PathBuf)> = None;
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if let Ok(meta) = entry.metadata() {
                if let Ok(mtime) = meta.modified() {
                    if best.as_ref().is_none_or(|(t, _)| mtime > *t) {
                        best = Some((mtime, path));
                    }
                }
            }
        }
    }

    best
}

/// Check whether `graph_path` is older than the running binary or project sources.
pub fn check_staleness(graph_path: &Path, project_root: Option<&Path>) -> StalenessReport {
    let mut reasons = Vec::new();
    let graph_mtime = file_mtime(graph_path);

    if let Some(gmtime) = graph_mtime {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(bmtime) = file_mtime(&exe) {
                if is_newer_than(bmtime, gmtime) {
                    reasons.push(format!(
                        "gm binary ({}) is newer than the graph file",
                        exe.display()
                    ));
                }
            }
        }

        if let Some(root) = project_root {
            let src_root = root.join("src");
            if src_root.is_dir() {
                if let Some((smtime, spath)) = newest_mtime_under(&src_root) {
                    if is_newer_than(smtime, gmtime) {
                        reasons.push(format!(
                            "source file {} is newer than the graph file",
                            spath.display()
                        ));
                    }
                }
            }
        }
    }

    StalenessReport {
        graph_path: graph_path.to_path_buf(),
        is_stale: !reasons.is_empty(),
        reasons,
    }
}

impl StalenessReport {
    /// Human-readable warning for MCP tools and startup logs.
    pub fn warning_message(&self) -> Option<String> {
        if !self.is_stale {
            return None;
        }
        let mut out = String::from(
            "**Graph may be stale.** Re-run `gm run . --no-semantic --no-viz`, then `reload_graph`.\n",
        );
        for reason in &self.reasons {
            out.push_str(&format!("- {reason}\n"));
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn detects_newer_source_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let graph_path = root.join("graphenium-out/graph.json");
        std::fs::create_dir_all(graph_path.parent().unwrap()).unwrap();
        std::fs::File::create(&graph_path).unwrap();

        let src = root.join("src");
        std::fs::create_dir_all(&src).unwrap();
        thread::sleep(Duration::from_millis(50));
        let src_file = src.join("lib.rs");
        let mut f = std::fs::File::create(&src_file).unwrap();
        f.write_all(b"fn main() {}").unwrap();
        drop(f);

        let report = check_staleness(&graph_path, Some(root));
        assert!(report.is_stale, "expected stale report: {report:?}");
        assert!(report.reasons.iter().any(|r| r.contains("lib.rs")));
    }

    #[test]
    fn fresh_graph_has_no_reasons() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let graph_path = root.join("graph.json");

        let src = root.join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::File::create(src.join("old.rs")).unwrap();
        thread::sleep(Duration::from_millis(50));
        std::fs::File::create(&graph_path).unwrap();

        let report = check_staleness(&graph_path, Some(root));
        assert!(!report.is_stale, "expected fresh report: {report:?}");
        assert!(report.reasons.is_empty());
    }
}