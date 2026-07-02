/// Normalize a human-readable node label while preserving semantic separators.
/// Current rules:
/// 1. Trim outer whitespace.
/// 2. Strip matching wrapper quotes/brackets around the entire label.
/// 3. Collapse internal whitespace runs to a single space.
pub fn normalize_label(label: &str) -> String {
    let mut value = label.trim();

    loop {
        let bytes = value.as_bytes();
        let wrapped = bytes.len() >= 2
            && matches!(
                (bytes[0], bytes[bytes.len() - 1]),
                (b'"', b'"')
                    | (b'\'', b'\'')
                    | (b'`', b'`')
                    | (b'<', b'>')
                    | (b'[', b']')
                    | (b'(', b')')
            );
        if !wrapped {
            break;
        }
        value = value[1..value.len() - 1].trim();
    }

    let mut normalized = String::with_capacity(value.len());
    let mut in_whitespace = false;
    for ch in value.chars() {
        if ch.is_whitespace() {
            if !in_whitespace {
                normalized.push(' ');
                in_whitespace = true;
            }
        } else {
            normalized.push(ch);
            in_whitespace = false;
        }
    }

    normalized.trim().to_string()
}

/// Normalize an ID-like value into Graphenium's canonical ID form.
pub fn normalize_id(value: &str) -> String {
    make_id(&[value])
}

/// Generate a stable, normalized node/edge ID from one or more parts.
/// Mirrors the Python `_make_id(*parts)` function exactly:
/// 1. Strip leading/trailing `_` and `.` from each part.
/// 2. Filter out empty parts.
/// 3. Join remaining parts with `_`.
/// 4. Replace every run of non-ASCII-alphanumeric characters with a single `_`.
/// 5. Strip leading/trailing `_`.
/// 6. Lowercase the result.
/// # Examples
/// ```
/// use graphenium::model::make_id;
/// assert_eq!(make_id(&["myfile", "MyClass"]), "myfile_myclass");
/// assert_eq!(make_id(&["models", "UserClass", "create"]), "models_userclass_create");
/// assert_eq!(make_id(&["_leading", ".dots."]), "leading_dots");
/// assert_eq!(make_id(&["foo", "", "bar"]), "foo_bar");
/// ```
pub fn make_id(parts: &[&str]) -> String {
    // Build the combined string from non-empty stripped parts.
    let combined: String = parts
        .iter()
        .map(|p| normalize_label(p))
        .map(|p| p.trim_matches(|c: char| c == '_' || c == '.').to_string())
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("_");

    // Replace each run of non-ASCII-alphanumeric chars with a single `_`.
    let mut result = String::with_capacity(combined.len());
    let mut in_sep = false;
    for c in combined.chars() {
        if c.is_ascii_alphanumeric() {
            result.push(c);
            in_sep = false;
        } else if !in_sep {
            result.push('_');
            in_sep = true;
        }
    }

    // Strip leading/trailing underscores and lowercase.
    result.trim_matches('_').to_lowercase()
}

/// Convenience wrapper accepting owned strings.
pub fn make_id_owned(parts: &[String]) -> String {
    let refs: Vec<&str> = parts.iter().map(|s| s.as_str()).collect();
    make_id(&refs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_join() {
        assert_eq!(make_id(&["myfile", "MyClass"]), "myfile_myclass");
    }

    #[test]
    fn three_parts() {
        assert_eq!(
            make_id(&["models", "UserClass", "create"]),
            "models_userclass_create"
        );
    }

    #[test]
    fn strips_leading_trailing_dots_underscores() {
        assert_eq!(make_id(&["_leading", ".dots."]), "leading_dots");
    }

    #[test]
    fn filters_empty_parts() {
        assert_eq!(make_id(&["foo", "", "bar"]), "foo_bar");
    }

    #[test]
    fn special_chars_become_single_underscore() {
        // "foo::bar" — C++ / Rust scoped name
        assert_eq!(make_id(&["foo::bar"]), "foo_bar");
        // Spaces and hyphens
        assert_eq!(make_id(&["my-class name"]), "my_class_name");
    }

    #[test]
    fn single_part() {
        assert_eq!(make_id(&["Extract"]), "extract");
    }

    #[test]
    fn idempotent() {
        let first = make_id(&["MyModule", "MyClass"]);
        let second = make_id(&[&first]);
        assert_eq!(first, second);
    }

    #[test]
    fn all_special_chars_stripped() {
        assert_eq!(make_id(&["__all__"]), "all");
    }

    #[test]
    fn empty_parts_only_returns_empty() {
        assert_eq!(make_id(&["", ""]), "");
    }

    #[test]
    fn normalize_label_strips_wrappers_and_whitespace() {
        assert_eq!(
            normalize_label("  `System.Collections.Generic`  "),
            "System.Collections.Generic"
        );
        assert_eq!(normalize_label("<stdio.h>"), "stdio.h");
        assert_eq!(normalize_label("foo   bar"), "foo bar");
    }

    #[test]
    fn normalize_id_uses_canonical_make_id() {
        assert_eq!(
            normalize_id(" System.Collections.Generic "),
            "system_collections_generic"
        );
    }
}
