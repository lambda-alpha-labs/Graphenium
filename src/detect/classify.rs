use std::path::Path;

use crate::model::FileType;

/// Map a file extension to a `FileType`.
///
/// Returns `None` for extensions that should be skipped entirely
/// (e.g. compiled artefacts, lock files).
pub fn classify_extension(path: &Path) -> Option<FileType> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match ext.as_str() {
        // ── Code ──────────────────────────────────────────────────────────
        "py" | "pyw"
        | "js" | "mjs" | "cjs"
        | "ts" | "tsx" | "jsx"
        | "go"
        | "rs"
        | "java"
        | "c" | "h"
        | "cpp" | "cc" | "cxx" | "hh" | "hpp" | "hxx"
        | "cs"
        | "rb"
        | "php"
        | "swift"
        | "kt" | "kts"
        | "scala"
        | "r"
        | "sh" | "bash" | "zsh" | "fish"
        | "ps1" | "psm1"
        | "lua"
        | "ex" | "exs"
        | "ml" | "mli"
        | "hs"
        | "clj" | "cljs"
        | "erl" | "hrl"
        | "sql"
        | "tf" | "hcl"
        | "yaml" | "yml"
        | "toml"
        | "json" | "jsonc"
        | "xml"
        | "html" | "htm"
        | "css" | "scss" | "sass" | "less"
        | "vue"
        | "svelte"
        | "dockerfile" => Some(FileType::Code),

        // ── Documents ─────────────────────────────────────────────────────
        "md" | "markdown"
        | "rst"
        | "txt"
        | "adoc" | "asciidoc"
        | "org"
        | "tex" | "latex" => Some(FileType::Document),

        // ── PDFs – may be papers or docs (paper heuristic applied later) ──
        "pdf" => Some(FileType::Paper),

        // ── Images ────────────────────────────────────────────────────────
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "tiff" | "tif" | "svg" => {
            Some(FileType::Image)
        }

        // ── Skip: binaries, lock files, artefacts ─────────────────────────
        "pyc" | "pyo" | "pyd"
        | "o" | "a" | "so" | "dylib" | "dll" | "lib" | "exe"
        | "wasm"
        | "class" | "jar"
        | "lock"          // Cargo.lock, package-lock.json, etc.
        | "sum"           // go.sum
        | "map"           // source maps
        | "min"           // minified
        | "gz" | "zip" | "tar" | "bz2" | "xz" | "7z" | "rar"
        | "iso" | "img"
        | "mp3" | "mp4" | "wav" | "ogg" | "avi" | "mov" | "mkv"
        | "ttf" | "woff" | "woff2" | "eot"
        | "ico" => None,

        _ => None,
    }
}

/// Returns `true` if the directory name should be skipped entirely.
pub fn is_skip_dir(name: &str) -> bool {
    matches!(
        name,
        "venv"
            | ".venv"
            | "env"
            | ".env"                // common Python virtualenv
            | "node_modules"
            | "__pycache__"
            | ".git"
            | ".svn"
            | ".hg"
            | "dist"
            | "build"
            | "target"             // Rust / Maven build output
            | "site-packages"
            | ".tox"
            | ".pytest_cache"
            | ".mypy_cache"
            | ".ruff_cache"
            | "coverage"
            | ".coverage"
            | ".DS_Store"
            | "Thumbs.db"
            | ".idea"
            | ".vscode"
            | "graphenium-out"       // our own output dir
            | ".rust-toolchain"     // local Rust toolchain (sandbox installs)
            | ".cargo" // local Cargo home (sandbox installs)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn code_extensions() {
        for ext in &["foo.py", "bar.rs", "baz.ts", "q.java", "m.cs"] {
            assert_eq!(
                classify_extension(Path::new(ext)),
                Some(FileType::Code),
                "expected Code for {ext}"
            );
        }
    }

    #[test]
    fn document_extensions() {
        for ext in &["README.md", "notes.rst", "plain.txt"] {
            assert_eq!(
                classify_extension(Path::new(ext)),
                Some(FileType::Document),
                "expected Document for {ext}"
            );
        }
    }

    #[test]
    fn pdf_is_paper() {
        assert_eq!(
            classify_extension(Path::new("paper.pdf")),
            Some(FileType::Paper)
        );
    }

    #[test]
    fn image_extensions() {
        for ext in &["photo.png", "icon.svg", "img.jpg"] {
            assert_eq!(
                classify_extension(Path::new(ext)),
                Some(FileType::Image),
                "expected Image for {ext}"
            );
        }
    }

    #[test]
    fn skip_extensions() {
        for ext in &["main.pyc", "lib.so", "package-lock.json.lock", "out.o"] {
            assert!(
                classify_extension(Path::new(ext)).is_none(),
                "expected None for {ext}"
            );
        }
    }

    #[test]
    fn skip_dirs() {
        assert!(is_skip_dir("node_modules"));
        assert!(is_skip_dir("__pycache__"));
        assert!(is_skip_dir("target"));
        assert!(!is_skip_dir("src"));
        assert!(!is_skip_dir("docs"));
    }
}
