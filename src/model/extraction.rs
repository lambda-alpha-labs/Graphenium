use serde::{Deserialize, Serialize};

use crate::model::{Edge, HyperEdge, Node};

/// The raw output of one extraction pass (AST or semantic) over a set of files.
///
/// Multiple `ExtractionResult` values are merged in the build phase.
/// When merging, duplicate node IDs are resolved by last-write-wins —
/// semantic results intentionally override AST results.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractionResult {
    #[serde(default)]
    pub nodes: Vec<Node>,

    #[serde(default)]
    pub edges: Vec<Edge>,

    #[serde(default)]
    pub hyperedges: Vec<HyperEdge>,

    /// Total input tokens consumed by LLM calls for this extraction.
    #[serde(default)]
    pub input_tokens: u64,

    /// Total output tokens produced by LLM calls for this extraction.
    #[serde(default)]
    pub output_tokens: u64,
}

impl ExtractionResult {
    pub fn new() -> Self {
        Self::default()
    }

    /// Merge `other` into `self`. Nodes/edges are appended; tokens are summed.
    /// Deduplication by node ID happens later in the build phase.
    pub fn merge(&mut self, other: ExtractionResult) {
        self.nodes.extend(other.nodes);
        self.edges.extend(other.edges);
        self.hyperedges.extend(other.hyperedges);
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
    }

    /// Merge a collection of `ExtractionResult` values into one.
    pub fn merge_all(results: impl IntoIterator<Item = ExtractionResult>) -> Self {
        let mut combined = Self::new();
        for r in results {
            combined.merge(r);
        }
        combined
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty() && self.edges.is_empty()
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }
}

/// A software package or crate in the repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub name: String,
    pub version: Option<String>,
    pub path: String,
}

/// A build target (binary, library, workspace member).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildTarget {
    pub name: String,
    pub kind: String,
    pub path: String,
}

/// A test target (unit test, integration test, benchmark).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestTarget {
    pub name: String,
    pub test_type: String,
    pub path: String,
    pub command: String,
}

/// A CI job parsed from CI configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CIJob {
    pub name: String,
    #[serde(default)]
    pub commands: Vec<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    pub yaml_path: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::FileType;

    #[test]
    fn merge_sums_tokens() {
        let mut a = ExtractionResult {
            input_tokens: 100,
            output_tokens: 50,
            ..Default::default()
        };
        let b = ExtractionResult {
            input_tokens: 200,
            output_tokens: 80,
            ..Default::default()
        };
        a.merge(b);
        assert_eq!(a.input_tokens, 300);
        assert_eq!(a.output_tokens, 130);
    }

    #[test]
    fn merge_concatenates_nodes() {
        let mut a = ExtractionResult::new();
        a.nodes
            .push(Node::new("a_foo", "Foo", FileType::Code, "a.py"));

        let mut b = ExtractionResult::new();
        b.nodes
            .push(Node::new("b_bar", "Bar", FileType::Code, "b.py"));

        a.merge(b);
        assert_eq!(a.nodes.len(), 2);
    }

    #[test]
    fn merge_all_combines_everything() {
        let results: Vec<ExtractionResult> = (0..3)
            .map(|i| {
                let mut r = ExtractionResult::new();
                r.input_tokens = i * 10;
                r
            })
            .collect();
        let merged = ExtractionResult::merge_all(results);
        assert_eq!(merged.input_tokens, 30); // 0 + 10 + 20
    }
}
