use std::path::Path;

/// Number of academic signals required to reclassify a document as a Paper.
const PAPER_THRESHOLD: usize = 3;

/// Maximum bytes to read for the paper heuristic.
const HEADER_BYTES: usize = 3000;

/// Signals that strongly suggest the file is an academic paper.
static PAPER_SIGNALS: &[&str] = &[
    "arxiv",
    "doi:",
    "doi.org",
    "abstract",
    "introduction",
    r"\cite{",
    r"\begin{",
    "proceedings",
    "conference",
    "journal",
    "ieee",
    "acm ",
    "springer",
    "preprint",
    "submitted to",
    "accepted to",
    "under review",
    "keywords:",
    "acknowledgements",
    "acknowledgments",
    "references\n",
    "bibliography",
    "et al.",
    "fig.",
    "table ",
    "equation",
    "theorem",
    "lemma",
    "proof",
    "corollary",
    "proposition",
    "algorithm",
    "neural network",
    "machine learning",
    "deep learning",
    "language model",
];

/// Heuristically determines whether a plain-text or Markdown file is an
/// academic paper by counting how many paper signals appear in the first
/// `HEADER_BYTES` bytes.
///
/// PDF files are already classified as `Paper` by extension; this function
/// applies to `.md`, `.rst`, `.txt`, and `.tex` files.
pub fn looks_like_paper(path: &Path) -> bool {
    let Ok(bytes) = std::fs::read(path) else {
        return false;
    };
    let snippet = &bytes[..bytes.len().min(HEADER_BYTES)];
    // Lossy conversion: non-UTF8 bytes become replacement chars, which won't
    // match any signal string, so the heuristic degrades gracefully.
    let text = String::from_utf8_lossy(snippet).to_lowercase();

    let count = PAPER_SIGNALS
        .iter()
        .filter(|&&signal| text.contains(signal))
        .count();

    count >= PAPER_THRESHOLD
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn make_temp(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    #[test]
    fn paper_with_many_signals() {
        let content = "arxiv preprint\n\
            Abstract: We present...\n\
            Keywords: machine learning, deep learning\n\
            \\cite{bengio2012}\n\
            IEEE Proceedings";
        let f = make_temp(content);
        assert!(looks_like_paper(f.path()));
    }

    #[test]
    fn regular_readme_not_paper() {
        let content = "# My Project\n\nThis is a README file with installation instructions.\n";
        let f = make_temp(content);
        assert!(!looks_like_paper(f.path()));
    }

    #[test]
    fn two_signals_not_enough() {
        // Only "introduction" and "doi:" — exactly 2, below the threshold of 3.
        let content = "Introduction\ndoi: 10.1234/example";
        let f = make_temp(content);
        assert!(!looks_like_paper(f.path()));
    }
}
