pub mod classify;
pub mod corpus;
pub mod paper;
pub mod sensitive;

use std::path::{Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;

use crate::model::FileType;

pub use corpus::{corpus_warnings, CorpusWarning};

/// A detected file together with its resolved `FileType`.
#[derive(Debug, Clone)]
pub struct DetectedFile {
    pub path: PathBuf,
    pub file_type: FileType,
}

/// Options controlling file detection.
#[derive(Debug, Clone, Default)]
pub struct DetectOptions {
    /// If `true`, only re-detect files changed since the last run (stub — full
    /// incremental support lives in Phase 8).
    pub incremental: bool,
}

/// Walk `root`, classify every file, apply the paper heuristic to text files,
/// skip sensitive files, and return the list together with any corpus warnings.
///
/// Respects `.gitignore` automatically (via the `ignore` crate).
/// Additionally layers any `.grapheniumignore` found in `root`.
pub fn detect(
    root: &Path,
    _opts: &DetectOptions,
) -> crate::Result<(Vec<DetectedFile>, Vec<CorpusWarning>)> {
    let graphenium_ignore = load_graphenium_ignore(root);

    let mut files: Vec<DetectedFile> = Vec::new();
    let mut total_words: u64 = 0;

    let walker = WalkBuilder::new(root)
        .hidden(false) // we handle hidden dirs ourselves via skip_dir
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .build();

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                eprintln!("[graphenium] warn: walk error: {e}");
                continue;
            }
        };

        let path = entry.path();

        // Skip directory entries outright.
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }

        // Skip files that live inside a skip-dir anywhere in their path.
        // The iterator-based `ignore::Walk` API cannot prune mid-walk, so we
        // check every path component here instead.
        let inside_skip_dir = path.components().any(|c| {
            if let std::path::Component::Normal(name) = c {
                name.to_str().map(classify::is_skip_dir).unwrap_or(false)
            } else {
                false
            }
        });
        if inside_skip_dir {
            continue;
        }

        // Apply .grapheniumignore patterns.
        if let Some(ref gs) = graphenium_ignore {
            // Match relative path from root.
            if let Ok(rel) = path.strip_prefix(root) {
                // Convert Windows backslashes to forward slashes for cross-platform glob matching
                let normalized_rel = rel.to_string_lossy().replace('\\', "/");
                if gs.is_match(&normalized_rel) {
                    continue;
                }
            }
        }

        // Skip sensitive files by filename.
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if sensitive::is_sensitive_filename(name) {
                eprintln!("[graphenium] skip sensitive: {}", path.display());
                continue;
            }
        }

        // Classify by extension.
        let Some(mut file_type) = classify::classify_extension(path) else {
            continue;
        };

        // Upgrade Document -> Paper via heuristic (PDFs are already Paper).
        if file_type == FileType::Document && paper::looks_like_paper(path) {
            file_type = FileType::Paper;
        }

        // Approximate word count for corpus health check.
        if matches!(
            file_type,
            FileType::Code | FileType::Document | FileType::Paper
        ) {
            if let Ok(content) = std::fs::read_to_string(path) {
                total_words += content.split_whitespace().count() as u64;
            }
        }

        files.push(DetectedFile {
            path: path.to_path_buf(),
            file_type,
        });
    }

    let warnings = corpus_warnings(files.len(), total_words);
    Ok((files, warnings))
}

/// Incremental variant: same as `detect` but scoped to files changed since
/// `since_path` was last modified.  Full incremental support (SHA256 manifest)
/// is Phase 8; this stub just calls `detect` for now.
pub fn detect_incremental(
    root: &Path,
    opts: &DetectOptions,
) -> crate::Result<(Vec<DetectedFile>, Vec<CorpusWarning>)> {
    detect(root, opts)
}

// ── Internals ────────────────────────────────────────────────────────────────

/// Load `.grapheniumignore` from `root`, if present.
fn load_graphenium_ignore(root: &Path) -> Option<GlobSet> {
    let ignore_path = root.join(".grapheniumignore");
    let content = std::fs::read_to_string(&ignore_path).ok()?;

    let mut builder = GlobSetBuilder::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        // Auto-append ** to trailing-slash patterns so users can write
        // `target/` instead of `target/**` (gitignore-style convenience).
        let pattern = if trimmed.ends_with('/') && !trimmed.contains("**") {
            format!("{trimmed}**")
        } else {
            trimmed.to_string()
        };
        match Glob::new(&pattern) {
            Ok(g) => {
                builder.add(g);
            }
            Err(e) => {
                eprintln!("[graphenium] warn: invalid .grapheniumignore pattern '{trimmed}': {e}");
            }
        }
    }

    builder.build().ok()
}

/// Initialize workspace defaults by writing a `.grapheniumignore` file if not present.
pub fn initialize_workspace(root: &Path) -> std::io::Result<bool> {
    let ignore_path = root.join(".grapheniumignore");
    if ignore_path.exists() {
        return Ok(false);
    }

    let default_ignore = "\
# Ignore toolchain and build artifacts
.rust-toolchain/
.cargo/
target/
node_modules/
__pycache__/
.venv/
dist/
build/
graphenium-out/

# C++ and C# build artifacts
obj/
bin/
.vs/
ipch/
*.user
*.suo
*.Designer.cs
*.g.cs
";

    std::fs::write(&ignore_path, default_ignore)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_file(dir: &Path, rel: &str, content: &str) {
        let full = dir.join(rel);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(full, content).unwrap();
    }

    #[test]
    fn detects_code_and_docs() {
        let tmp = TempDir::new().unwrap();
        create_file(tmp.path(), "main.py", "def foo(): pass\n");
        create_file(tmp.path(), "README.md", "# Hello\n");

        let (files, _) = detect(tmp.path(), &DetectOptions::default()).unwrap();
        let mut types: Vec<_> = files.iter().map(|f| &f.file_type).collect();
        types.sort_by_key(|t| format!("{t:?}"));

        assert!(files.iter().any(|f| f.file_type == FileType::Code));
        assert!(files.iter().any(|f| f.file_type == FileType::Document));
    }

    #[test]
    fn skips_sensitive_files() {
        let tmp = TempDir::new().unwrap();
        create_file(tmp.path(), ".env", "SECRET=abc");
        create_file(tmp.path(), "main.py", "pass");

        let (files, _) = detect(tmp.path(), &DetectOptions::default()).unwrap();
        assert!(files.iter().all(|f| !f.path.ends_with(".env")));
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn respects_grapheniumignore() {
        let tmp = TempDir::new().unwrap();
        create_file(tmp.path(), ".grapheniumignore", "*.log\nignored_dir/**");
        create_file(tmp.path(), "app.py", "pass");
        create_file(tmp.path(), "debug.log", "log line");
        create_file(tmp.path(), "ignored_dir/stuff.py", "pass");

        let (files, _) = detect(tmp.path(), &DetectOptions::default()).unwrap();
        assert!(files
            .iter()
            .all(|f| !f.path.to_string_lossy().ends_with(".log")));
    }

    #[test]
    fn skips_node_modules() {
        let tmp = TempDir::new().unwrap();
        create_file(tmp.path(), "index.js", "const x = 1;");
        create_file(tmp.path(), "node_modules/lib/index.js", "module");

        let (files, _) = detect(tmp.path(), &DetectOptions::default()).unwrap();
        assert!(files
            .iter()
            .all(|f| !f.path.to_string_lossy().contains("node_modules")));
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn corpus_warnings_emitted_for_small_corpus() {
        let tmp = TempDir::new().unwrap();
        create_file(tmp.path(), "tiny.py", "x = 1");

        let (_, warnings) = detect(tmp.path(), &DetectOptions::default()).unwrap();
        assert!(!warnings.is_empty());
        assert!(matches!(warnings[0], CorpusWarning::TooSmall { .. }));
    }
}
