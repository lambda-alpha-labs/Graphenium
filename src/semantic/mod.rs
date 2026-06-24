/// Semantic extraction orchestrator.
///
/// Flow:
/// ```text
/// check cache ──► all cached? ──► return merged cached results
///                     │
///                     ▼ (uncached files)
///              split into batches of `batch_size`
///                     │
///                     ▼
///         tokio tasks gated by Semaphore(max_concurrency)
///                     │
///              call Claude API
///                     │
///              parse + validate
///                     │
///         cache per-file, merge results
/// ```
pub mod client;
pub mod parse;
pub mod prompt;
pub mod provider;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::Semaphore;

use crate::cache::semantic_cache::{self, CacheMiss};
use crate::detect::DetectedFile;
use crate::extract::ExtractMode;
use crate::model::{ExtractionResult, FileType};
use crate::validate;

pub use client::{ClaudeClient, ContentBlock, LlmClient};
pub use provider::AiProvider;

// ── Public types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SemanticOptions {
    /// AI provider for semantic extraction. Defaults to Anthropic.
    pub provider: AiProvider,
    /// API key for the selected provider. An empty string disables semantic extraction.
    pub api_key: String,
    /// Model to call. If empty, the provider's default model is used.
    pub model: String,
    /// Extraction aggressiveness.
    pub mode: ExtractMode,
    /// Files per API call (default 20).
    pub batch_size: usize,
    /// Maximum simultaneous in-flight API calls (default 3).
    pub max_concurrency: usize,
}

impl Default for SemanticOptions {
    fn default() -> Self {
        let provider = AiProvider::Anthropic;
        let api_key = std::env::var(provider.env_var_name()).unwrap_or_default();
        Self {
            provider,
            api_key,
            model: String::new(),
            mode: ExtractMode::Standard,
            batch_size: 20,
            max_concurrency: 3,
        }
    }
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Run semantic extraction over `files`, returning a merged `ExtractionResult`.
///
/// Files whose content hash is already present in `cache_dir` are returned
/// directly from cache without calling the API.
///
/// If `opts.api_key` is empty, returns an empty result immediately.
pub async fn extract_semantic(
    files: &[DetectedFile],
    opts: &SemanticOptions,
    cache_dir: &Path,
) -> crate::Result<ExtractionResult> {
    if files.is_empty() || opts.api_key.is_empty() {
        return Ok(ExtractionResult::new());
    }

    let file_paths: Vec<PathBuf> = files.iter().map(|f| f.path.clone()).collect();
    let (hits, misses) = semantic_cache::check_semantic_cache(&file_paths, cache_dir);

    // Accumulate cached results.
    let mut combined = ExtractionResult::new();
    for hit in hits {
        combined.merge(hit.result);
    }

    if misses.is_empty() {
        return Ok(combined);
    }

    // Pair each miss with its DetectedFile so we know the file_type.
    let miss_files: Vec<(&DetectedFile, &CacheMiss)> = misses
        .iter()
        .filter_map(|m| files.iter().find(|f| f.path == m.path).map(|df| (df, m)))
        .collect();

    let model = if opts.model.is_empty() {
        opts.provider.default_model().to_string()
    } else {
        opts.model.clone()
    };
    if model.is_empty() {
        eprintln!(
            "[graphenium] warn: no model configured for provider {}; skipping semantic extraction.",
            opts.provider
        );
        return Ok(ExtractionResult::new());
    }

    let client = Arc::new(LlmClient::new(opts.provider.clone(), &opts.api_key, &model));
    let semaphore = Arc::new(Semaphore::new(opts.max_concurrency));
    let system = Arc::new(prompt::system_prompt(&opts.mode));

    let mut handles = Vec::new();

    for batch in miss_files.chunks(opts.batch_size) {
        // Collect the data we need to move into the task.
        let batch_files: Vec<(PathBuf, FileType)> = batch
            .iter()
            .map(|(df, _)| (df.path.clone(), df.file_type.clone()))
            .collect();
        let batch_hashes: Vec<(PathBuf, String)> = batch
            .iter()
            .map(|(df, m)| (df.path.clone(), m.hash.clone()))
            .collect();

        let client = Arc::clone(&client);
        let sem = Arc::clone(&semaphore);
        let sys = Arc::clone(&system);
        let cache_dir = cache_dir.to_path_buf();

        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire().await;
            process_batch(&client, &sys, &batch_files, &batch_hashes, &cache_dir).await
        }));
    }

    for handle in handles {
        match handle.await {
            Ok(Ok(result)) => combined.merge(result),
            Ok(Err(e)) => eprintln!("[graphenium] semantic batch error: {e}"),
            Err(e) => eprintln!("[graphenium] semantic batch join error: {e}"),
        }
    }

    Ok(combined)
}

// ── Batch processing ──────────────────────────────────────────────────────────

async fn process_batch(
    client: &LlmClient,
    system: &str,
    files: &[(PathBuf, FileType)],
    hashes: &[(PathBuf, String)], // (path, sha256_hash)
    cache_dir: &Path,
) -> crate::Result<ExtractionResult> {
    let content = prompt::build_user_content(files);

    let (text, input_tokens, output_tokens) = client.messages(system, &content).await?;

    let mut result = parse::parse_extraction(&text);
    result.input_tokens = input_tokens;
    result.output_tokens = output_tokens;

    // Validate — strip malformed items in-place.
    validate::validate(&mut result);

    // Cache a per-file slice of the result so future runs can reuse it.
    for (path, hash) in hashes {
        if hash.is_empty() {
            continue;
        }
        let file_result = filter_by_file(&result, path);
        if let Err(e) = semantic_cache::store(cache_dir, hash, &file_result) {
            eprintln!("[graphenium] cache write error for {}: {e}", path.display());
        }
    }

    Ok(result)
}

/// Extract the subset of `result` whose `source_file` field matches `path`.
///
/// Path matching is intentionally lenient: we try exact normalised match,
/// suffix match (result path is a suffix of the corpus path or vice-versa),
/// and finally filename-only match.  This accommodates the LLM writing
/// relative paths that don't perfectly match the absolute corpus paths.
fn filter_by_file(result: &ExtractionResult, path: &Path) -> ExtractionResult {
    let target = path.to_string_lossy().replace('\\', "/");
    let target_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();

    let matches = |source_file: &str| -> bool {
        let sf = source_file.replace('\\', "/");
        sf == target
            || target.ends_with(&sf)
            || sf.ends_with(&target)
            || sf
                .split('/')
                .last()
                .map(|n| n.to_lowercase() == target_name)
                .unwrap_or(false)
    };

    ExtractionResult {
        nodes: result
            .nodes
            .iter()
            .filter(|n| matches(&n.source_file))
            .cloned()
            .collect(),
        edges: result
            .edges
            .iter()
            .filter(|e| matches(&e.source_file))
            .cloned()
            .collect(),
        // Hyperedges reference multiple files; store them under the first
        // matching file.
        hyperedges: result
            .hyperedges
            .iter()
            .filter(|h| matches(&h.source_file))
            .cloned()
            .collect(),
        input_tokens: result.input_tokens,
        output_tokens: result.output_tokens,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::semantic_cache;
    use crate::model::{Confidence, Edge, FileType, Node};
    use tempfile::TempDir;

    fn make_detected(path: PathBuf) -> DetectedFile {
        DetectedFile {
            path,
            file_type: FileType::Code,
        }
    }

    // ── filter_by_file ────────────────────────────────────────────────────────

    #[test]
    fn filter_matches_exact_path() {
        let mut r = ExtractionResult::new();
        r.nodes
            .push(Node::new("a", "A", FileType::Code, "src/a.py"));
        r.nodes
            .push(Node::new("b", "B", FileType::Code, "src/b.py"));

        let filtered = filter_by_file(&r, Path::new("src/a.py"));
        assert_eq!(filtered.nodes.len(), 1);
        assert_eq!(filtered.nodes[0].id, "a");
    }

    #[test]
    fn filter_matches_filename_only() {
        let mut r = ExtractionResult::new();
        r.nodes.push(Node::new("a", "A", FileType::Code, "a.py"));

        // Absolute corpus path vs. filename-only source_file in LLM response
        let filtered = filter_by_file(&r, Path::new("/home/user/project/a.py"));
        assert_eq!(filtered.nodes.len(), 1);
    }

    // ── Empty / no-API-key paths ──────────────────────────────────────────────

    #[tokio::test]
    async fn empty_files_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let opts = SemanticOptions {
            api_key: "key".to_string(),
            ..Default::default()
        };
        let result = extract_semantic(&[], &opts, tmp.path()).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn empty_api_key_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("f.py");
        std::fs::write(&p, b"x=1").unwrap();
        let opts = SemanticOptions {
            api_key: String::new(), // no key
            ..Default::default()
        };
        let result = extract_semantic(&[make_detected(p)], &opts, tmp.path())
            .await
            .unwrap();
        assert!(result.is_empty());
    }

    // ── All-cached path (no API call) ─────────────────────────────────────────

    #[tokio::test]
    async fn all_cached_returns_merged_without_api_call() {
        let tmp = TempDir::new().unwrap();
        let cache_dir = tmp.path().join("cache");

        // Write a file and pre-populate the cache for it.
        let p = tmp.path().join("a.py");
        std::fs::write(&p, b"class Foo: pass").unwrap();
        let hash = crate::cache::file_hash(&p).unwrap();

        let mut cached = ExtractionResult::new();
        cached
            .nodes
            .push(Node::new("a_foo", "Foo", FileType::Code, "a.py"));
        crate::cache::save_cached(&cache_dir, &hash, &cached).unwrap();

        let opts = SemanticOptions {
            api_key: "dummy-key-would-fail-if-called".to_string(),
            ..Default::default()
        };

        let result = extract_semantic(&[make_detected(p)], &opts, &cache_dir)
            .await
            .unwrap();

        assert_eq!(result.nodes.len(), 1);
        assert_eq!(result.nodes[0].id, "a_foo");
    }
}
