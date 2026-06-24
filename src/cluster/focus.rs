//! Path-prefix focus labelling for communities.
//!
//! A community's *focus* is the longest directory prefix shared by its
//! members' source files (e.g. `./src/serve`). When members span unrelated
//! directories, focus falls back to the one or two most common file names,
//! and finally `None` for truly scattered communities.

use std::collections::HashMap;

/// Normalize a filesystem path for cross-platform display: strip the Windows
/// `\\?\` verbatim prefix, convert backslashes to forward slashes, and drop
/// any leading slash.
pub fn normalize_display_path(path: &str) -> String {
    path.strip_prefix("\\\\?\\")
        .unwrap_or(path)
        .replace('\\', "/")
        .trim_start_matches('/')
        .to_string()
}

/// Longest directory prefix shared by all `paths`, or `None` if less than
/// two segments are common. The trailing segment is dropped if it looks like
/// a filename (contains a `.`), so we return a directory rather than a file.
pub fn common_path_prefix<'a, I>(paths: I) -> Option<String>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut iter = paths.into_iter();
    let first = iter.next()?;
    let mut prefix: Vec<String> = normalize_display_path(first)
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    for path in iter {
        let segments: Vec<String> = normalize_display_path(path)
            .split('/')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();
        let common_len = prefix
            .iter()
            .zip(segments.iter())
            .take_while(|(a, b)| a.eq_ignore_ascii_case(b))
            .count();
        prefix.truncate(common_len);
        if prefix.is_empty() {
            return None;
        }
    }

    if prefix.last().is_some_and(|segment| segment.contains('.')) {
        prefix.pop();
    }

    if prefix.len() <= 1 {
        None
    } else {
        Some(prefix.join("/"))
    }
}

/// Return the `limit` keys with the highest values, ties broken
/// alphabetically. Used for the file-name focus fallback.
pub fn top_counts(map: &HashMap<String, usize>, limit: usize) -> Vec<(String, usize)> {
    let mut entries: Vec<_> = map.iter().map(|(k, v)| (k.clone(), *v)).collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    entries.truncate(limit);
    entries
}

/// Compute a human-readable focus label for a community described by its
/// member source-file paths. Falls back to the top two most common file
/// names joined with ` | ` when no shared path prefix exists.
pub fn focus_label(paths: &[String]) -> Option<String> {
    if let Some(prefix) = common_path_prefix(paths.iter().map(String::as_str)) {
        return Some(prefix);
    }

    let mut file_counts: HashMap<String, usize> = HashMap::new();
    for path in paths {
        *file_counts.entry(path.clone()).or_default() += 1;
    }
    let top = top_counts(&file_counts, 2);
    if top.is_empty() {
        None
    } else {
        Some(
            top.into_iter()
                .map(|(f, _)| f)
                .collect::<Vec<_>>()
                .join(" | "),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_prefix_detected() {
        let paths = vec![
            "./src/serve/mod.rs".to_string(),
            "./src/serve/handlers.rs".to_string(),
            "./src/serve/traversal.rs".to_string(),
        ];
        assert_eq!(focus_label(&paths).as_deref(), Some("./src/serve"));
    }

    #[test]
    fn filename_stripped_from_prefix() {
        let paths = vec!["./src/a/b.rs".to_string(), "./src/a/b.rs".to_string()];
        assert_eq!(focus_label(&paths).as_deref(), Some("./src/a"));
    }

    #[test]
    fn no_common_prefix_falls_back_to_filenames() {
        let paths = vec!["x/a.rs".to_string(), "y/a.rs".to_string()];
        assert!(focus_label(&paths).is_some());
    }

    #[test]
    fn windows_verbatim_prefix_normalized() {
        let paths = vec![
            "\\\\?\\C:\\Work\\foo\\bar.rs".to_string(),
            "\\\\?\\C:\\Work\\foo\\baz.rs".to_string(),
        ];
        let focus = focus_label(&paths).unwrap();
        assert!(focus.contains("foo"));
    }
}
