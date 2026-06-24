/// Corpus health thresholds.
const WARN_TOO_FEW_WORDS: u64 = 50_000;
const WARN_TOO_MANY_WORDS: u64 = 500_000;
const WARN_TOO_MANY_FILES: usize = 200;

/// A non-fatal warning about corpus quality.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorpusWarning {
    /// Very small corpus — graph may not be useful.
    TooSmall { word_count: u64 },
    /// Very large corpus — processing may be slow and expensive.
    TooLarge { word_count: u64 },
    /// Many files — LLM batching costs may be high.
    ManyFiles { file_count: usize },
}

impl std::fmt::Display for CorpusWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CorpusWarning::TooSmall { word_count } => write!(
                f,
                "Corpus is small ({word_count} words). \
                 The graph may have limited connectivity."
            ),
            CorpusWarning::TooLarge { word_count } => write!(
                f,
                "Corpus is large ({word_count} words). \
                 Semantic extraction may be slow and expensive. \
                 Hint: add non-source directories (target/, node_modules/, \
                 .rust-toolchain/) to .grapheniumignore."
            ),
            CorpusWarning::ManyFiles { file_count } => write!(
                f,
                "Corpus has many files ({file_count}). \
                 LLM batching costs may be high. \
                 Hint: consider adding build/toolchain directories to \
                 .grapheniumignore."
            ),
        }
    }
}

/// Produce any applicable warnings given corpus statistics.
pub fn corpus_warnings(file_count: usize, word_count: u64) -> Vec<CorpusWarning> {
    let mut warnings = Vec::new();

    if word_count < WARN_TOO_FEW_WORDS {
        warnings.push(CorpusWarning::TooSmall { word_count });
    } else if word_count > WARN_TOO_MANY_WORDS {
        warnings.push(CorpusWarning::TooLarge { word_count });
    }

    if file_count > WARN_TOO_MANY_FILES {
        warnings.push(CorpusWarning::ManyFiles { file_count });
    }

    warnings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_corpus_warns() {
        let w = corpus_warnings(5, 1000);
        assert_eq!(w.len(), 1);
        assert!(matches!(w[0], CorpusWarning::TooSmall { .. }));
    }

    #[test]
    fn large_corpus_warns() {
        let w = corpus_warnings(10, 600_000);
        assert_eq!(w.len(), 1);
        assert!(matches!(w[0], CorpusWarning::TooLarge { .. }));
    }

    #[test]
    fn many_files_warns() {
        let w = corpus_warnings(250, 100_000);
        assert_eq!(w.len(), 1);
        assert!(matches!(w[0], CorpusWarning::ManyFiles { .. }));
    }

    #[test]
    fn both_large_and_many_files() {
        let w = corpus_warnings(300, 600_000);
        assert_eq!(w.len(), 2);
    }

    #[test]
    fn healthy_corpus_no_warnings() {
        let w = corpus_warnings(50, 100_000);
        assert!(w.is_empty());
    }
}
