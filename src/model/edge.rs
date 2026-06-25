use serde::{Deserialize, Serialize};

use crate::model::id::normalize_id;

/// How certain we are that a relationship actually exists.
///
/// Serializes as SCREAMING_SNAKE_CASE to match the Python output format.
///
/// | Variant    | Score | Meaning                                         |
/// |------------|-------|-------------------------------------------------|
/// | Extracted  | 1.0   | Explicit in source (import, call, citation)     |
/// | Inferred   | 0.5   | Reasonable inference with documented reasoning  |
/// | Ambiguous  | 0.2   | Uncertain — flagged for manual review           |
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Confidence {
    Extracted,
    Inferred,
    Ambiguous,
}

impl Confidence {
    /// Default numeric confidence score for JSON export.
    /// Matches the Python export's `conf_score_map`.
    pub fn default_score(&self) -> f64 {
        match self {
            Confidence::Extracted => 1.0,
            Confidence::Inferred => 0.5,
            Confidence::Ambiguous => 0.2,
        }
    }

    /// Bonus used in the surprise-scoring algorithm (analyze phase).
    /// AMBIGUOUS edges score highest because unexpected relationships are
    /// more surprising than explicitly stated ones.
    pub fn surprise_bonus(&self) -> i32 {
        match self {
            Confidence::Ambiguous => 3,
            Confidence::Inferred => 2,
            Confidence::Extracted => 1,
        }
    }
}

impl std::fmt::Display for Confidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Confidence::Extracted => "EXTRACTED",
            Confidence::Inferred => "INFERRED",
            Confidence::Ambiguous => "AMBIGUOUS",
        };
        write!(f, "{s}")
    }
}

/// A directed relationship between two nodes, stored in an undirected graph.
///
/// The `src_original` / `tgt_original` fields preserve the intended direction
/// even after petgraph normalizes the endpoints of the undirected edge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    /// ID of the source node.
    pub source: String,

    /// ID of the target node.
    pub target: String,

    /// Relation type, e.g. `"imports"`, `"calls"`, `"contains"`,
    /// `"semantically_similar_to"`, `"rationale_for"`.
    pub relation: String,

    /// Confidence level for this relationship.
    pub confidence: Confidence,

    /// Numeric confidence in [0.0, 1.0]. Defaults to `confidence.default_score()`
    /// if not explicitly set by the extractor.
    pub confidence_score: f64,

    /// Relative path of the file this edge was extracted from.
    pub source_file: String,

    /// Edge weight (used in traversal). 1.0 for EXTRACTED, 0.8 for inferred
    /// calls (call-graph pass), 0.5–0.95 for semantic edges.
    #[serde(default = "default_weight")]
    pub weight: f64,

    /// Original source node ID before petgraph normalization.
    #[serde(default)]
    pub src_original: String,

    /// Original target node ID before petgraph normalization.
    #[serde(default)]
    pub tgt_original: String,

    /// Optional source location hint, e.g. `"L72"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_location: Option<String>,

    /// Which extractor produced this edge: "tree-sitter", "tree-sitter-stack-graphs",
    /// "llm-anthropic", "llm-openai", "heuristic-string-match", "manual-mcp-write", etc.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extractor: Option<String>,

    /// Resolution status: "resolved", "unresolved", "ambiguous", "heuristic", "inferred".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution_status: Option<String>,
}

fn default_weight() -> f64 {
    1.0
}

impl Edge {
    pub fn new(
        source: impl Into<String>,
        target: impl Into<String>,
        relation: impl Into<String>,
        confidence: Confidence,
        source_file: impl Into<String>,
    ) -> Self {
        let source_raw = source.into();
        let target_raw = target.into();
        let source = normalize_id(&source_raw);
        let target = normalize_id(&target_raw);
        let score = confidence.default_score();
        Self {
            src_original: source_raw,
            tgt_original: target_raw,
            source,
            target,
            relation: relation.into().trim().to_lowercase(),
            confidence_score: score,
            confidence,
            source_file: source_file.into(),
            weight: default_weight(),
            source_location: None,
            extractor: None,
            resolution_status: None,
        }
    }

    /// Construct an EXTRACTED edge (weight 1.0, score 1.0).
    /// Sets extractor to "tree-sitter" and resolution_status to "resolved".
    pub fn extracted(
        source: impl Into<String>,
        target: impl Into<String>,
        relation: impl Into<String>,
        source_file: impl Into<String>,
    ) -> Self {
        let mut e = Self::new(source, target, relation, Confidence::Extracted, source_file);
        e.extractor = Some("tree-sitter".to_string());
        e.resolution_status = Some("resolved".to_string());
        e
    }

    /// Construct an INFERRED call-graph edge (weight 0.8, score 0.5).
    /// Resolution is considered "heuristic" since the AST makes a best guess.
    pub fn inferred_call(
        caller: impl Into<String>,
        callee: impl Into<String>,
        source_file: impl Into<String>,
    ) -> Self {
        let mut e = Self::new(caller, callee, "calls", Confidence::Inferred, source_file);
        e.weight = 0.8;
        e.extractor = Some("tree-sitter".to_string());
        e.resolution_status = Some("heuristic".to_string());
        e
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confidence_serializes_uppercase() {
        assert_eq!(
            serde_json::to_string(&Confidence::Extracted).unwrap(),
            r#""EXTRACTED""#
        );
        assert_eq!(
            serde_json::to_string(&Confidence::Inferred).unwrap(),
            r#""INFERRED""#
        );
        assert_eq!(
            serde_json::to_string(&Confidence::Ambiguous).unwrap(),
            r#""AMBIGUOUS""#
        );
    }

    #[test]
    fn confidence_deserializes_uppercase() {
        let c: Confidence = serde_json::from_str(r#""EXTRACTED""#).unwrap();
        assert_eq!(c, Confidence::Extracted);
    }

    #[test]
    fn edge_default_weight_is_one() {
        let e = Edge::extracted("a", "b", "imports", "src/a.py");
        assert_eq!(e.weight, 1.0);
        assert_eq!(e.confidence_score, 1.0);
    }

    #[test]
    fn inferred_call_has_reduced_weight() {
        let e = Edge::inferred_call("foo", "bar", "src/x.py");
        assert_eq!(e.weight, 0.8);
        assert_eq!(e.confidence, Confidence::Inferred);
        assert_eq!(e.relation, "calls");
    }

    #[test]
    fn edge_normalizes_endpoints_and_relation() {
        let e = Edge::new(
            " Foo::Bar ",
            " `Baz` ",
            " Calls ",
            Confidence::Extracted,
            "src/a.py",
        );
        assert_eq!(e.source, "foo_bar");
        assert_eq!(e.target, "baz");
        assert_eq!(e.relation, "calls");
        assert_eq!(e.src_original, " Foo::Bar ");
    }
}
