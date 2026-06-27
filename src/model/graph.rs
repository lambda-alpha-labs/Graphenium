use std::collections::HashMap;

use petgraph::graph::{Graph, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::Undirected;
use serde::{Deserialize, Serialize};

use crate::model::{Edge, HyperEdge, Node};

/// Type alias for the underlying petgraph structure.
pub type PetGraph = Graph<Node, Edge, Undirected>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphMetadata {
    #[serde(default)]
    pub ast_only: bool,
    /// Schema version of the graph.json format (e.g. "0.2.0").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_version: Option<String>,
    /// Version of Graphenium that produced this graph.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graphenium_version: Option<String>,
    /// ISO 8601 timestamp of when the graph was built.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    /// Absolute path to the project root that was analyzed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_root: Option<String>,
    /// Extraction modes used: "ast", "semantic", etc.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extraction_modes: Option<Vec<String>>,
    /// Languages detected in the source tree.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub languages: Option<Vec<String>>,
}

impl Default for GraphMetadata {
    fn default() -> Self {
        Self {
            ast_only: false,
            schema_version: None,
            graphenium_version: Some(env!("CARGO_PKG_VERSION").to_string()),
            created_at: None,
            project_root: None,
            extraction_modes: None,
            languages: None,
        }
    }
}

/// The central knowledge graph structure.
///
/// Wraps a `petgraph::Graph` (undirected) and adds:
/// - O(1) node lookup by string ID via `id_index`.
/// - A side-car `Vec<HyperEdge>` for N-ary relationships.
///
/// Edge direction is preserved logically through `Edge::src_original` and
/// `Edge::tgt_original` even though the underlying graph is undirected.
#[derive(Clone)]
pub struct GrapheniumGraph {
    pub(crate) inner: PetGraph,
    /// Maps node IDs to their petgraph `NodeIndex`.
    pub(crate) id_index: HashMap<String, NodeIndex>,
    /// N-ary relationships (3+ nodes). Stored separately since petgraph
    /// does not natively support hyperedges.
    pub hyperedges: Vec<HyperEdge>,
    pub metadata: GraphMetadata,
}

impl GrapheniumGraph {
    pub fn new() -> Self {
        Self {
            inner: Graph::new_undirected(),
            id_index: HashMap::new(),
            hyperedges: Vec::new(),
            metadata: GraphMetadata::default(),
        }
    }

    // ── Node operations ────────────────────────────────────────────────────

    /// Insert a node, or overwrite an existing one with the same ID.
    /// Last-write-wins: semantic results override AST results.
    pub fn upsert_node(&mut self, node: Node) {
        if let Some(&idx) = self.id_index.get(&node.id) {
            self.inner[idx] = node;
        } else {
            let id = node.id.clone();
            let idx = self.inner.add_node(node);
            self.id_index.insert(id, idx);
        }
    }

    pub fn node_data(&self, id: &str) -> Option<&Node> {
        let &idx = self.id_index.get(id)?;
        Some(&self.inner[idx])
    }

    pub fn node_data_mut(&mut self, id: &str) -> Option<&mut Node> {
        let idx = *self.id_index.get(id)?;
        Some(&mut self.inner[idx])
    }

    pub fn contains_node(&self, id: &str) -> bool {
        self.id_index.contains_key(id)
    }

    pub fn node_count(&self) -> usize {
        self.inner.node_count()
    }

    pub fn node_ids(&self) -> impl Iterator<Item = &str> {
        self.id_index.keys().map(|s| s.as_str())
    }

    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.inner.node_weights()
    }

    // ── Edge operations ────────────────────────────────────────────────────

    /// Add an edge between two existing nodes.
    /// Returns `false` (and skips the edge) if either node ID is not in the graph.
    /// This is the intended behaviour for dangling edges to external libraries.
    pub fn add_edge(&mut self, edge: Edge) -> bool {
        let src_idx = match self.id_index.get(&edge.source) {
            Some(&idx) => idx,
            None => return false,
        };
        let tgt_idx = match self.id_index.get(&edge.target) {
            Some(&idx) => idx,
            None => return false,
        };

        if self.has_logically_equivalent_edge(src_idx, tgt_idx, &edge) {
            return true;
        }

        self.inner.add_edge(src_idx, tgt_idx, edge);
        true
    }

    /// Rebuild the id_index from the current petgraph state.
    /// Must be called after remove_node operations since petgraph may shift indices.
    pub fn rebuild_id_index(&mut self) {
        self.id_index = self
            .inner
            .node_indices()
            .map(|idx| (self.inner[idx].id.clone(), idx))
            .collect();
    }

    /// Validate id_index consistency. Returns true if every entry matches.
    pub fn validate_id_index(&self) -> bool {
        for (id, idx) in &self.id_index {
            if let Some(node) = self.inner.node_weight(*idx) {
                if node.id != *id {
                    return false;
                }
            } else {
                return false;
            }
        }
        true
    }

    pub fn edge_count(&self) -> usize {
        self.inner.edge_count()
    }

    /// Iterate over all edges as `(source_id, target_id, &Edge)`.
    pub fn edges_with_endpoints(&self) -> impl Iterator<Item = (&str, &str, &Edge)> {
        self.inner.edge_references().map(|e| {
            let src = &self.inner[e.source()].id;
            let tgt = &self.inner[e.target()].id;
            (src.as_str(), tgt.as_str(), e.weight())
        })
    }

    /// Get all edges between two nodes, matching by the current node ID.
    pub fn edges_between(&self, a: &str, b: &str) -> Vec<&Edge> {
        self.edges_iter()
            .filter(|e| (e.source == a && e.target == b) || (e.source == b && e.target == a))
            .collect()
    }

    pub fn edges_iter(&self) -> impl Iterator<Item = &Edge> {
        self.inner.edge_weights()
    }

    // ── Neighbour / degree ─────────────────────────────────────────────────

    /// Number of edges incident to node `id`.
    pub fn degree(&self, id: &str) -> usize {
        match self.id_index.get(id) {
            Some(&idx) => self.inner.edges(idx).count(),
            None => 0,
        }
    }

    /// IDs of all nodes directly connected to `id`.
    pub fn neighbor_ids(&self, id: &str) -> Vec<String> {
        let Some(&idx) = self.id_index.get(id) else {
            return Vec::new();
        };
        self.inner
            .edges(idx)
            .map(|e| {
                let other = if e.source() == idx {
                    e.target()
                } else {
                    e.source()
                };
                self.inner[other].id.clone()
            })
            .collect()
    }

    /// Edges incident to node `id`, as `(neighbor_id, &Edge)` pairs.
    pub fn node_edges(&self, id: &str) -> Vec<(&str, &Edge)> {
        let Some(&idx) = self.id_index.get(id) else {
            return Vec::new();
        };
        self.inner
            .edges(idx)
            .map(|e| {
                let other = if e.source() == idx {
                    e.target()
                } else {
                    e.source()
                };
                (self.inner[other].id.as_str(), e.weight())
            })
            .collect()
    }

    // ── Edge removal ──────────────────────────────────────────────────────

    /// Remove all edges incident to node `id` and return the count removed.
    ///
    /// Does not remove the node itself. Returns 0 if the node is not found.
    pub fn remove_edges_for(&mut self, id: &str) -> usize {
        let Some(&idx) = self.id_index.get(id) else {
            return 0;
        };
        let edge_indices: Vec<_> = self.inner.edges(idx).map(|e| e.id()).collect();
        let count = edge_indices.len();
        for edge_idx in edge_indices {
            self.inner.remove_edge(edge_idx);
        }
        count
    }

    // ── File-level replacement (incremental patching) ─────────────────────

    /// Replace all nodes (and their incident edges) that originated from
    /// `source_file` with the contents of `new_result`. Nodes from other
    /// files and edges that don't touch replaced nodes are untouched.
    ///
    /// This is the core operation behind incremental watch-mode patching:
    /// when a single file changes, only that file's contributions are
    /// surgically removed and re-inserted.
    pub fn replace_file_extraction(
        &mut self,
        source_file: &str,
        new_result: &crate::model::ExtractionResult,
    ) -> ReplaceStats {
        let mut stats = ReplaceStats::default();

        // 1. Find and remove all nodes that came from this source file.
        //    Removing a node via petgraph also drops its incident edges.
        let stale_ids: Vec<String> = self
            .nodes()
            .filter(|n| n.source_file == source_file)
            .map(|n| n.id.clone())
            .collect();

        for id in &stale_ids {
            if let Some(idx) = self.id_index.remove(id) {
                self.inner.remove_node(idx);
                stats.nodes_removed += 1;
            }
        }

        // CRITICAL: Rebuild the id_index after batch deletion.
        // petgraph uses swap-remove, which shifts the last node into the
        // vacated slot. Without rebuilding, future lookups via stale indices
        // will silently return the wrong node or panic.
        self.rebuild_id_index();

        // 2. Insert the new nodes and edges from the re-extraction.
        for node in &new_result.nodes {
            self.upsert_node(node.clone());
        }
        stats.nodes_inserted = new_result.nodes.len();

        for edge in &new_result.edges {
            if self.add_edge(edge.clone()) {
                stats.edges_inserted += 1;
            } else {
                stats.edges_dropped_dangling += 1;
            }
        }

        stats.new_hyperedges = new_result.hyperedges.len();

        stats
    }

    // ── Internal petgraph access (for cluster / analyze phases) ───────────

    pub fn inner(&self) -> &PetGraph {
        &self.inner
    }

    pub fn node_index(&self, id: &str) -> Option<NodeIndex> {
        self.id_index.get(id).copied()
    }

    pub fn is_ast_only(&self) -> bool {
        self.metadata.ast_only
    }

    pub fn set_ast_only(&mut self, ast_only: bool) {
        self.metadata.ast_only = ast_only;
    }

    fn has_logically_equivalent_edge(
        &self,
        src_idx: NodeIndex,
        tgt_idx: NodeIndex,
        candidate: &Edge,
    ) -> bool {
        self.inner
            .edges_connecting(src_idx, tgt_idx)
            .any(|existing| logically_same_edge(existing.weight(), candidate))
    }
}

/// Statistics returned by [`GrapheniumGraph::replace_file_extraction`].
#[derive(Debug, Default, Clone)]
pub struct ReplaceStats {
    /// Nodes removed (and their incident edges).
    pub nodes_removed: usize,
    /// New nodes inserted.
    pub nodes_inserted: usize,
    /// New edges successfully added.
    pub edges_inserted: usize,
    /// New edges dropped because one endpoint no longer exists.
    pub edges_dropped_dangling: usize,
    /// New hyperedges from the extraction.
    pub new_hyperedges: usize,
}

fn logically_same_edge(existing: &Edge, candidate: &Edge) -> bool {
    existing.source == candidate.source
        && existing.target == candidate.target
        && existing.relation == candidate.relation
        && existing.confidence == candidate.confidence
}

impl Default for GrapheniumGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::FileType;

    fn make_node(id: &str) -> Node {
        Node::new(id, id, FileType::Code, "test.rs")
    }

    #[test]
    fn upsert_node_and_lookup() {
        let mut g = GrapheniumGraph::new();
        g.upsert_node(make_node("foo"));
        assert!(g.contains_node("foo"));
        assert_eq!(g.node_data("foo").unwrap().id, "foo");
        assert_eq!(g.node_count(), 1);
    }

    #[test]
    fn upsert_node_last_write_wins() {
        let mut g = GrapheniumGraph::new();
        g.upsert_node(make_node("foo"));

        // Overwrite with updated label
        let mut updated = make_node("foo");
        updated.label = "FooUpdated".into();
        g.upsert_node(updated);

        assert_eq!(g.node_count(), 1);
        assert_eq!(g.node_data("foo").unwrap().label, "FooUpdated");
    }

    #[test]
    fn add_edge_success() {
        let mut g = GrapheniumGraph::new();
        g.upsert_node(make_node("a"));
        g.upsert_node(make_node("b"));
        let edge = Edge::extracted("a", "b", "imports", "x.py");
        assert!(g.add_edge(edge));
        assert_eq!(g.edge_count(), 1);
    }

    #[test]
    fn add_edge_deduplicates_logical_duplicates() {
        let mut g = GrapheniumGraph::new();
        g.upsert_node(make_node("a"));
        g.upsert_node(make_node("b"));

        assert!(g.add_edge(Edge::extracted("a", "b", "calls", "x.py")));
        assert!(g.add_edge(Edge::extracted("a", "b", "calls", "y.py")));

        assert_eq!(g.edge_count(), 1);
    }

    #[test]
    fn add_edge_keeps_distinct_direction_or_confidence() {
        let mut g = GrapheniumGraph::new();
        g.upsert_node(make_node("a"));
        g.upsert_node(make_node("b"));

        assert!(g.add_edge(Edge::extracted("a", "b", "calls", "x.py")));
        assert!(g.add_edge(Edge::inferred_call("a", "b", "x.py")));
        assert!(g.add_edge(Edge::extracted("b", "a", "calls", "x.py")));

        assert_eq!(g.edge_count(), 3);
    }

    #[test]
    fn add_edge_dangling_returns_false() {
        let mut g = GrapheniumGraph::new();
        g.upsert_node(make_node("a"));
        // "b" not in graph — dangling edge
        let edge = Edge::extracted("a", "b", "imports", "x.py");
        assert!(!g.add_edge(edge));
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn degree_and_neighbors() {
        let mut g = GrapheniumGraph::new();
        g.upsert_node(make_node("a"));
        g.upsert_node(make_node("b"));
        g.upsert_node(make_node("c"));
        g.add_edge(Edge::extracted("a", "b", "imports", "x.py"));
        g.add_edge(Edge::extracted("a", "c", "imports", "x.py"));

        assert_eq!(g.degree("a"), 2);
        assert_eq!(g.degree("b"), 1);

        let mut neighbors = g.neighbor_ids("a");
        neighbors.sort();
        assert_eq!(neighbors, vec!["b", "c"]);
    }

    #[test]
    fn remove_edges_for_drops_incident_edges() {
        let mut g = GrapheniumGraph::new();
        g.upsert_node(make_node("a"));
        g.upsert_node(make_node("b"));
        g.upsert_node(make_node("c"));
        g.add_edge(Edge::extracted("a", "b", "imports", "x.py"));
        g.add_edge(Edge::extracted("a", "c", "calls", "x.py"));
        g.add_edge(Edge::extracted("b", "c", "calls", "x.py"));

        // Remove edges from "a" — drops a-b and a-c, leaves b-c
        let removed = g.remove_edges_for("a");
        assert_eq!(removed, 2);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.degree("a"), 0);
        assert_eq!(g.degree("b"), 1);
        assert_eq!(g.degree("c"), 1);
        // Node "a" still exists
        assert!(g.contains_node("a"));
    }

    #[test]
    fn remove_edges_for_unknown_node_returns_zero() {
        let mut g = GrapheniumGraph::new();
        g.upsert_node(make_node("a"));
        let removed = g.remove_edges_for("does_not_exist");
        assert_eq!(removed, 0);
    }
}
