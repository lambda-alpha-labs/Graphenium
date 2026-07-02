/// Prompt construction for semantic extraction.
/// Builds the system prompt and per-batch user content blocks sent to Claude.
/// Text files are included verbatim (truncated at 50 KB).  Image files are
/// base64-encoded and sent as `image` content blocks.
use std::path::Path;

use base64::Engine as _;

use crate::extract::ExtractMode;
use crate::model::FileType;

use super::client::ContentBlock;

// ── System prompt ─────────────────────────────────────────────────────────────

/// Build the system prompt string.  Deep mode appends extra instructions for
/// aggressive inference.
pub fn system_prompt(mode: &ExtractMode) -> String {
    let base = r#"You are a knowledge graph extraction assistant. Analyze the provided files and extract entities and relationships.

Return ONLY a JSON object (no other text, no markdown, no explanation) with this exact structure:
{
  "nodes": [
    {"id": "stem_name", "label": "DisplayName", "file_type": "code", "source_file": "relative/path.ext"}
  ],
  "edges": [
    {"source": "node_id", "target": "node_id", "relation": "calls", "confidence": "EXTRACTED", "confidence_score": 1.0, "source_file": "relative/path.ext"}
  ],
  "hyperedges": [
    {"id": "he_id", "label": "Description", "nodes": ["id1", "id2", "id3"], "relation": "participate_in", "confidence": "INFERRED", "confidence_score": 0.7, "source_file": "relative/path.ext"}
  ]
}

Node ID rules: lowercase, underscores only, format "<file_stem>_<entity_name>".
File types: "code", "document", "paper", "image", "rationale".
Confidence:
  EXTRACTED  — explicitly stated in source (import, call, citation).
  INFERRED   — reasonable inference from context, naming, or structure.
  AMBIGUOUS  — the relationship may or may not exist; flag for review.
Relation types: imports, calls, contains, uses, references, extends, implements,
  depends_on, semantically_similar_to, rationale_for, participate_in.

Instructions:
1. Identify key entities: classes, functions, modules, concepts, design decisions.
2. Extract explicit relationships: imports, function calls, inheritance.
3. Infer implicit relationships: shared concepts, similar names, design patterns.
4. Create "rationale" nodes for architectural decisions documented in comments.
5. Create semantically_similar_to edges for conceptually related entities across files.
6. Create hyperedges (3+ nodes) for group relationships; at most 3 per file.
7. Return at most 50 nodes and 100 edges. Prioritise the most important."#;

    if *mode == ExtractMode::Deep {
        format!(
            "{base}\n\nDEEP MODE: Aggressively infer from naming conventions. \
             Identify design patterns (Factory, Observer, Strategy, etc.) and create edges. \
             Flag surprising cross-module coupling. Surface implicit architectural dependencies."
        )
    } else {
        base.to_string()
    }
}

// ── User content ──────────────────────────────────────────────────────────────

const MAX_TEXT_BYTES: usize = 50_000;
const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024; // 5 MB

/// Build the `content` array for the user message.
/// `files` is a slice of `(absolute_path, file_type)` pairs.
pub fn build_user_content(files: &[(impl AsRef<Path>, FileType)]) -> Vec<ContentBlock> {
    let mut blocks = Vec::new();

    blocks.push(ContentBlock::text(format!(
        "Analyze the following {} file(s) and extract a knowledge graph.\n",
        files.len()
    )));

    for (path, file_type) in files {
        let path = path.as_ref();
        let path_str = path.to_string_lossy();

        if *file_type == FileType::Image {
            if let Some((media_type, data)) = encode_image(path) {
                blocks.push(ContentBlock::text(format!("\n=== IMAGE: {path_str} ===")));
                blocks.push(ContentBlock::image(media_type, data));
            } else {
                blocks.push(ContentBlock::text(format!(
                    "\n=== BINARY IMAGE (not displayable): {path_str} ==="
                )));
            }
        } else {
            let content = read_truncated(path, MAX_TEXT_BYTES);
            blocks.push(ContentBlock::text(format!(
                "\n=== FILE: {path_str} ===\n{content}"
            )));
        }
    }

    blocks
}

// ── Helpers ────────────────────────────────────────────────────────────────────

/// Read a file as UTF-8, replacing invalid sequences, truncated at `max_bytes`.
fn read_truncated(path: &Path, max_bytes: usize) -> String {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(_) => return "[file unreadable]".to_string(),
    };
    let truncated = if bytes.len() > max_bytes {
        &bytes[..max_bytes]
    } else {
        &bytes
    };
    let text = String::from_utf8_lossy(truncated).into_owned();
    if bytes.len() > max_bytes {
        format!("{text}\n[… truncated at {max_bytes} bytes]")
    } else {
        text
    }
}

/// Base64-encode an image file.  Returns `None` if the file is too large,
/// unreadable, or has an unsupported extension.
fn encode_image(path: &Path) -> Option<(String, String)> {
    let media_type = image_media_type(path)?.to_string();
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() > MAX_IMAGE_BYTES {
        eprintln!(
            "[graphenium] skipping large image ({} MB): {}",
            bytes.len() / 1_048_576,
            path.display()
        );
        return None;
    }
    let data = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Some((media_type, data))
}

/// Map an image extension to its MIME type.
fn image_media_type(path: &Path) -> Option<&'static str> {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .as_deref()
    {
        Some("png") => Some("image/png"),
        Some("jpg") | Some("jpeg") => Some("image/jpeg"),
        Some("gif") => Some("image/gif"),
        Some("webp") => Some("image/webp"),
        _ => None,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn system_prompt_standard_contains_key_terms() {
        let p = system_prompt(&ExtractMode::Standard);
        assert!(p.contains("EXTRACTED"));
        assert!(p.contains("INFERRED"));
        assert!(p.contains("AMBIGUOUS"));
        assert!(p.contains("hyperedges"));
        assert!(!p.contains("DEEP MODE"));
    }

    #[test]
    fn system_prompt_deep_adds_instructions() {
        let p = system_prompt(&ExtractMode::Deep);
        assert!(p.contains("DEEP MODE"));
        assert!(p.contains("naming conventions"));
    }

    #[test]
    fn build_user_content_text_file() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(b"def hello(): pass").unwrap();
        let path = f.path().to_path_buf();

        let blocks = build_user_content(&[(path.clone(), FileType::Code)]);
        // Should have: intro block + file block
        assert_eq!(blocks.len(), 2);
        if let ContentBlock::Text { text } = &blocks[1] {
            assert!(text.contains("hello"));
            assert!(text.contains("FILE:"));
        } else {
            panic!("expected text block");
        }
    }

    #[test]
    fn build_user_content_multiple_files() {
        let mut f1 = NamedTempFile::new().unwrap();
        f1.write_all(b"class A: pass").unwrap();
        let mut f2 = NamedTempFile::new().unwrap();
        f2.write_all(b"class B: pass").unwrap();

        let blocks = build_user_content(&[
            (f1.path().to_path_buf(), FileType::Code),
            (f2.path().to_path_buf(), FileType::Code),
        ]);
        // intro + 2 file blocks
        assert_eq!(blocks.len(), 3);
    }

    #[test]
    fn truncation_applied_to_large_file() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(&vec![b'x'; MAX_TEXT_BYTES + 100]).unwrap();

        let blocks = build_user_content(&[(f.path().to_path_buf(), FileType::Code)]);
        if let ContentBlock::Text { text } = &blocks[1] {
            assert!(text.contains("truncated"));
        } else {
            panic!("expected text block");
        }
    }

    #[test]
    fn unreadable_file_produces_placeholder() {
        let blocks = build_user_content(&[(
            std::path::PathBuf::from("/nonexistent/file.py"),
            FileType::Code,
        )]);
        if let ContentBlock::Text { text } = &blocks[1] {
            assert!(text.contains("unreadable"));
        } else {
            panic!("expected text block");
        }
    }

    #[test]
    fn intro_block_mentions_file_count() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(b"x=1").unwrap();

        let blocks = build_user_content(&[(f.path().to_path_buf(), FileType::Code)]);
        if let ContentBlock::Text { text } = &blocks[0] {
            assert!(text.contains("1 file"));
        } else {
            panic!("expected text block");
        }
    }
}
