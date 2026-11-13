use std::path::{Path, PathBuf};

/// Convert a path into a portable, repository-relative string.
///
/// This is the single canonical helper used at every emission/storage boundary
/// (graph nodes, graph edges, AST cache payloads, and manifests). It:
/// 1. strips the Windows `\\?\` verbatim prefix,
/// 2. strips `root` when `path` is inside it,
/// 3. converts `\` to `/`, and
/// 4. trims a leading `./` plus trailing separators.
///
/// When `root` is `None`, or `path` is outside the root, the path is only
/// separator-normalized. Callers that know the project root should always pass
/// it so absolute paths never leak into persisted artifacts.
pub fn to_repo_relative(path: &Path, root: Option<&Path>) -> String {
    let mut candidate = path.to_path_buf();

    // Normalize Windows-style roots on non-Windows hosts so lexical stripping
    // still works for cross-platform manifests (e.g. CI artifacts).
    let normalized_root = root.map(|r| {
        let text = r.to_string_lossy();
        if !cfg!(windows) && text.contains('\\') {
            PathBuf::from(text.replace('\\', "/"))
        } else {
            r.to_path_buf()
        }
    });
    let root = normalized_root.as_deref();

    // Strip verbatim prefixes on Windows (and no-op on other platforms).
    if let Some(stripped) = candidate.to_str().and_then(|s| {
        s.strip_prefix(r"\\?\UNC\")
            .map(|rest| format!(r"\\{rest}"))
            .or_else(|| s.strip_prefix(r"\\?\").map(str::to_string))
    }) {
        // On non-Windows hosts the stripped path still uses `\` separators;
        // normalize them before lexical root stripping.
        candidate = PathBuf::from(stripped.replace('\\', "/"));
    }

    if let Some(root) = root {
        // Try a lexical strip first (cheap and works for non-existent paths).
        if let Ok(rel) = candidate.strip_prefix(root) {
            candidate = rel.to_path_buf();
        } else if let (Ok(canon_path), Ok(canon_root)) = (
            std::fs::canonicalize(&candidate),
            std::fs::canonicalize(root),
        ) {
            if let Ok(rel) = canon_path.strip_prefix(&canon_root) {
                candidate = rel.to_path_buf();
            }
        }
    }

    candidate
        .to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_end_matches('/')
        .to_string()
}

/// Identify test/benchmark/spec modules by analyzing file path fragments.
pub fn is_test_like_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/").to_lowercase();
    normalized.contains("/tests/")
        || normalized.contains("/test/")
        || normalized.contains("_test.")
        || normalized.contains(".test.")
        || normalized.contains("/spec/")
        || normalized.contains("/specs/")
        || normalized.contains("_spec.")
        || normalized.contains("tests.rs")
        || normalized.contains("test_bench")
        || normalized.contains("_test.rs")
        || normalized.contains("_tests.rs")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_test_and_spec_paths() {
        assert!(is_test_like_path("src/tests/foo.rs"));
        assert!(is_test_like_path("src/foo_test.rs"));
        assert!(is_test_like_path("spec/models_spec.rb"));
        assert!(!is_test_like_path("src/main.rs"));
    }

    #[test]
    fn repo_relative_strips_root_and_normalizes_separators() {
        let root = Path::new("/project");
        assert_eq!(
            to_repo_relative(Path::new("/project/src/main.rs"), Some(root)),
            "src/main.rs"
        );
        assert_eq!(
            to_repo_relative(Path::new("/project/src\\main.rs"), Some(root)),
            "src/main.rs"
        );
    }

    #[test]
    fn repo_relative_keeps_outside_paths_but_normalizes() {
        assert_eq!(
            to_repo_relative(Path::new("/elsewhere/./lib.rs"), None),
            "/elsewhere/./lib.rs"
        );
        assert_eq!(
            to_repo_relative(Path::new("./src/lib.rs"), None),
            "src/lib.rs"
        );
    }

    #[test]
    fn repo_relative_strips_windows_verbatim_prefix() {
        assert_eq!(
            to_repo_relative(
                Path::new(r"\\?\C:\repo\src\main.rs"),
                Some(Path::new(r"C:\repo"))
            ),
            "src/main.rs"
        );
    }
}
