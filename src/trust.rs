//! Evidence and claim models for the v3 trust system.
//!
//! EvidenceSpan ties every node and edge to a specific source location.
//! Claim provides agent-facing structured interpretations of graph facts.
//! Evidence validation detects stale data after file changes.

use serde::{Deserialize, Serialize};

// ── Evidence state ───────────────────────────────────────────────────────────

/// Whether an evidence span is still valid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvidenceState {
    Valid,
    Stale,
    Unverified,
    Missing,
}

// ── Evidence span ─────────────────────────────────────────────────────────────

/// A span of source code that a node or edge was extracted from.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceSpan {
    /// Relative file path.
    pub file: String,
    /// Start byte offset.
    pub start_byte: usize,
    /// End byte offset (exclusive).
    pub end_byte: usize,
    /// Start line number (1-based).
    pub start_line: usize,
    /// End line number (1-based).
    pub end_line: usize,
    /// SHA256 of the exact span text.
    pub span_hash: String,
    /// SHA256 of the full file contents at extraction time.
    pub file_hash: String,
    /// Which extractor produced this evidence.
    pub extractor: String,
    /// Whether this span is still current.
    pub state: EvidenceState,
}

impl EvidenceSpan {
    /// Create a new evidence span.
    pub fn new(
        file: impl Into<String>,
        start_byte: usize,
        end_byte: usize,
        start_line: usize,
        end_line: usize,
        span_text: &[u8],
        file_bytes: &[u8],
        extractor: impl Into<String>,
    ) -> Self {
        use sha2::{Digest, Sha256};
        let span_hash = hex::encode(Sha256::digest(span_text));
        let file_hash = hex::encode(Sha256::digest(file_bytes));
        Self {
            file: file.into(),
            start_byte,
            end_byte,
            start_line,
            end_line,
            span_hash,
            file_hash,
            extractor: extractor.into(),
            state: EvidenceState::Valid,
        }
    }

    /// Check whether this evidence span is stale by comparing file hashes.
    pub fn validate(&self, current_file_bytes: &[u8]) -> bool {
        use sha2::{Digest, Sha256};
        let current_hash = hex::encode(Sha256::digest(current_file_bytes));
        current_hash == self.file_hash && self.state == EvidenceState::Valid
    }

    /// Mark this evidence as stale.
    pub fn mark_stale(&mut self) {
        self.state = EvidenceState::Stale;
    }

    /// Mark this evidence as validated.
    pub fn mark_valid(&mut self) {
        self.state = EvidenceState::Valid;
    }
}

/// Format used for compact JSON output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceSpanCompact {
    pub file: String,
    pub sl: usize,
    pub el: usize,
    pub sh: String,
}

// ── Claim model ──────────────────────────────────────────────────────────────

/// The type of claim an agent-facing statement represents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClaimType {
    Dependency,
    Impact,
    Chokepoint,
    Verification,
    Latency,
    RiskGate,
}

/// An agent-facing interpretation of graph evidence.
///
/// Claims separate raw graph facts from reasoned outputs. A claim always
/// includes supporting evidence so agents can verify the claim independently.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claim {
    /// Unique identifier for this claim.
    pub id: String,
    /// Human-readable statement of the claim.
    pub statement: String,
    /// What kind of claim this is.
    pub claim_type: ClaimType,
    /// Confidence in this claim (matches the graph confidence model).
    pub confidence: String,
    /// Node IDs that support this claim.
    pub supporting_nodes: Vec<String>,
    /// Source files that contain the supporting evidence.
    pub supporting_files: Vec<String>,
    /// Whether the agent must read source code to verify this claim.
    pub requires_source_inspection: bool,
}

impl Claim {
    pub fn new(
        id: impl Into<String>,
        statement: impl Into<String>,
        claim_type: ClaimType,
        confidence: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            statement: statement.into(),
            claim_type,
            confidence: confidence.into(),
            supporting_nodes: Vec::new(),
            supporting_files: Vec::new(),
            requires_source_inspection: false,
        }
    }

    pub fn with_support(mut self, nodes: Vec<&str>, files: Vec<&str>) -> Self {
        self.supporting_nodes = nodes.into_iter().map(|s| s.to_string()).collect();
        self.supporting_files = files.into_iter().map(|s| s.to_string()).collect();
        self
    }
}

// ── Resolution report ────────────────────────────────────────────────────────

/// A summary of resolution quality across the graph.
#[derive(Debug, Clone, Default)]
pub struct ResolutionReport {
    pub total_import_edges: usize,
    pub resolved_imports: usize,
    pub heuristic_edges: usize,
    pub ambiguous_edges: usize,
    pub unresolved_refs: usize,
    pub total_call_edges: usize,
    pub resolved_calls: usize,
    pub total_method_edges: usize,
    pub resolved_methods: usize,
    pub evidence_valid: usize,
    pub evidence_stale: usize,
    pub evidence_missing: usize,
}

impl ResolutionReport {
    /// Format as a human-readable string.
    pub fn format(&self) -> String {
        let import_pct = if self.total_import_edges > 0 {
            (self.resolved_imports as f64 / self.total_import_edges as f64) * 100.0
        } else {
            0.0
        };
        let call_pct = if self.total_call_edges > 0 {
            (self.resolved_calls as f64 / self.total_call_edges as f64) * 100.0
        } else {
            0.0
        };
        let method_pct = if self.total_method_edges > 0 {
            (self.resolved_methods as f64 / self.total_method_edges as f64) * 100.0
        } else {
            0.0
        };

        format!(
            "Resolution coverage:\n\
             - Imports resolved: {:.0}% ({}/{})\n\
             - Calls resolved: {:.0}% ({}/{})\n\
             - Methods resolved: {:.0}% ({}/{})\n\
             - Heuristic edges: {}\n\
             - Ambiguous edges: {}\n\
             - Unresolved references: {}\n\
             - Evidence valid: {}\n\
             - Evidence stale: {}\n\
             - Evidence missing: {}",
            import_pct,
            self.resolved_imports,
            self.total_import_edges,
            call_pct,
            self.resolved_calls,
            self.total_call_edges,
            method_pct,
            self.resolved_methods,
            self.total_method_edges,
            self.heuristic_edges,
            self.ambiguous_edges,
            self.unresolved_refs,
            self.evidence_valid,
            self.evidence_stale,
            self.evidence_missing,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evidence_span_created_with_hashes() {
        let span = EvidenceSpan::new(
            "src/main.rs",
            0, 100, 1, 5,
            b"fn main() {}",
            b"fn main() {}\n",
            "tree-sitter",
        );
        assert_eq!(span.file, "src/main.rs");
        assert_eq!(span.state, EvidenceState::Valid);
        assert!(!span.span_hash.is_empty());
        assert!(!span.file_hash.is_empty());
    }

    #[test]
    fn evidence_validation_detects_stale() {
        let span = EvidenceSpan::new(
            "src/main.rs",
            0, 100, 1, 5,
            b"fn main() {}",
            b"fn main() {}\n",
            "tree-sitter",
        );
        assert!(span.validate(b"fn main() {}\n"));
        assert!(!span.validate(b"fn main() { println!(\"changed\"); }\n"));
    }

    #[test]
    fn claim_created_with_defaults() {
        let c = Claim::new("c1", "This is a test", ClaimType::Verification, "EXTRACTED");
        assert_eq!(c.id, "c1");
        assert!(c.supporting_nodes.is_empty());
        assert!(!c.requires_source_inspection);
    }

    #[test]
    fn resolution_report_formats_correctly() {
        let r = ResolutionReport {
            total_import_edges: 100,
            resolved_imports: 94,
            heuristic_edges: 9,
            ambiguous_edges: 4,
            unresolved_refs: 312,
            ..Default::default()
        };
        let s = r.format();
        assert!(s.contains("94%"));
        assert!(s.contains("9"));
    }
}
