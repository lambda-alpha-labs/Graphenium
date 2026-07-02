/// Validation of `ExtractionResult` values before graph construction.
/// Validation is a **mutation + reporting** step: invalid nodes and edges are
/// removed in-place from the result, and a `ValidationReport` is returned
/// describing what was stripped.
/// The confidence enum (`Confidence`) is guaranteed valid by Rust's type
/// system, so only the numeric `confidence_score` and required string fields
/// need runtime checking.
use crate::model::ExtractionResult;

// ── Report ────────────────────────────────────────────────────────────────────

/// A single validation problem found in an `ExtractionResult`.
#[derive(Debug, Clone)]
pub enum ValidationIssue {
    /// Node with empty `id` field (un-addressable in the graph).
    NodeEmptyId,
    /// Node `label` is empty.
    NodeEmptyLabel { id: String },
    /// Node `source_file` is empty.
    NodeEmptySourceFile { id: String },
    /// Edge `source` ID is empty.
    EdgeEmptySource { index: usize },
    /// Edge `target` ID is empty.
    EdgeEmptyTarget { index: usize },
    /// `confidence_score` is outside `[0.0, 1.0]`.
    EdgeInvalidScore {
        source: String,
        target: String,
        score: f64,
    },
    /// HyperEdge references fewer than 3 nodes.
    HyperEdgeTooFew { id: String, count: usize },
    /// HyperEdge `id` is empty.
    HyperEdgeEmptyId,
}

/// Summary of what the validator found and removed.
#[derive(Debug, Default)]
pub struct ValidationReport {
    pub issues: Vec<ValidationIssue>,
    pub nodes_removed: usize,
    pub edges_removed: usize,
    pub hyperedges_removed: usize,
}

impl ValidationReport {
    pub fn is_clean(&self) -> bool {
        self.nodes_removed == 0 && self.edges_removed == 0 && self.hyperedges_removed == 0
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Validate `result` in-place, stripping invalid entries and returning a
/// report of everything that was removed.
/// Designed to run before `build::build_from_extraction`.
pub fn validate(result: &mut ExtractionResult) -> ValidationReport {
    let mut report = ValidationReport::default();

    // ── Nodes ──────────────────────────────────────────────────────────────
    result.nodes.retain(|node| {
        if node.id.is_empty() {
            report.issues.push(ValidationIssue::NodeEmptyId);
            report.nodes_removed += 1;
            return false;
        }
        let mut ok = true;
        if node.label.is_empty() {
            report.issues.push(ValidationIssue::NodeEmptyLabel {
                id: node.id.clone(),
            });
            ok = false;
        }
        if node.source_file.is_empty() {
            report.issues.push(ValidationIssue::NodeEmptySourceFile {
                id: node.id.clone(),
            });
            ok = false;
        }
        if !ok {
            report.nodes_removed += 1;
        }
        ok
    });

    // ── Edges ──────────────────────────────────────────────────────────────
    let mut idx = 0usize;
    result.edges.retain(|edge| {
        let mut ok = true;
        if edge.source.is_empty() {
            report
                .issues
                .push(ValidationIssue::EdgeEmptySource { index: idx });
            ok = false;
        }
        if edge.target.is_empty() {
            report
                .issues
                .push(ValidationIssue::EdgeEmptyTarget { index: idx });
            ok = false;
        }
        if !(0.0..=1.0).contains(&edge.confidence_score) {
            report.issues.push(ValidationIssue::EdgeInvalidScore {
                source: edge.source.clone(),
                target: edge.target.clone(),
                score: edge.confidence_score,
            });
            ok = false;
        }
        idx += 1;
        if !ok {
            report.edges_removed += 1;
        }
        ok
    });

    // ── HyperEdges ─────────────────────────────────────────────────────────
    result.hyperedges.retain(|he| {
        if he.id.is_empty() {
            report.issues.push(ValidationIssue::HyperEdgeEmptyId);
            report.hyperedges_removed += 1;
            return false;
        }
        if he.nodes.len() < 3 {
            report.issues.push(ValidationIssue::HyperEdgeTooFew {
                id: he.id.clone(),
                count: he.nodes.len(),
            });
            report.hyperedges_removed += 1;
            return false;
        }
        true
    });

    report
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Confidence, Edge, FileType, HyperEdge, Node};

    fn good_node(id: &str) -> Node {
        Node::new(id, id, FileType::Code, "file.rs")
    }

    fn good_edge(src: &str, tgt: &str) -> Edge {
        Edge::extracted(src, tgt, "calls", "file.rs")
    }

    #[test]
    fn clean_result_passes() {
        let mut r = ExtractionResult::new();
        r.nodes.push(good_node("a"));
        r.nodes.push(good_node("b"));
        r.edges.push(good_edge("a", "b"));
        let report = validate(&mut r);
        assert!(report.is_clean());
        assert_eq!(r.nodes.len(), 2);
        assert_eq!(r.edges.len(), 1);
    }

    #[test]
    fn node_with_empty_id_removed() {
        let mut r = ExtractionResult::new();
        r.nodes.push(good_node("ok"));
        let mut bad = good_node("");
        bad.id = String::new();
        r.nodes.push(bad);
        let report = validate(&mut r);
        assert_eq!(report.nodes_removed, 1);
        assert_eq!(r.nodes.len(), 1);
    }

    #[test]
    fn node_with_empty_label_removed() {
        let mut r = ExtractionResult::new();
        let mut bad = good_node("x");
        bad.label = String::new();
        r.nodes.push(bad);
        let report = validate(&mut r);
        assert_eq!(report.nodes_removed, 1);
        assert!(r.nodes.is_empty());
    }

    #[test]
    fn edge_with_invalid_score_removed() {
        let mut r = ExtractionResult::new();
        r.nodes.push(good_node("a"));
        r.nodes.push(good_node("b"));
        let mut bad = good_edge("a", "b");
        bad.confidence_score = 1.5; // out of range
        r.edges.push(bad);
        let report = validate(&mut r);
        assert_eq!(report.edges_removed, 1);
        assert!(r.edges.is_empty());
    }

    #[test]
    fn edge_with_empty_source_removed() {
        let mut r = ExtractionResult::new();
        let mut bad = good_edge("a", "b");
        bad.source = String::new();
        r.edges.push(bad);
        let report = validate(&mut r);
        assert_eq!(report.edges_removed, 1);
    }

    #[test]
    fn hyperedge_with_too_few_nodes_removed() {
        let mut r = ExtractionResult::new();
        let he = HyperEdge {
            id: "he1".into(),
            label: "test".into(),
            nodes: vec!["a".into(), "b".into()], // only 2 — below threshold
            relation: "related".into(),
            confidence: Confidence::Inferred,
            confidence_score: 0.5,
            source_file: "f.py".into(),
        };
        r.hyperedges.push(he);
        let report = validate(&mut r);
        assert_eq!(report.hyperedges_removed, 1);
        assert!(r.hyperedges.is_empty());
    }

    #[test]
    fn valid_hyperedge_kept() {
        let mut r = ExtractionResult::new();
        let he = HyperEdge {
            id: "he1".into(),
            label: "test".into(),
            nodes: vec!["a".into(), "b".into(), "c".into()],
            relation: "related".into(),
            confidence: Confidence::Inferred,
            confidence_score: 0.5,
            source_file: "f.py".into(),
        };
        r.hyperedges.push(he);
        let report = validate(&mut r);
        assert_eq!(report.hyperedges_removed, 0);
        assert_eq!(r.hyperedges.len(), 1);
    }
}
