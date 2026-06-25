/// JSON node-link export.
///
/// Format:
/// ```json
/// {
///   "nodes":      [ { id, label, file_type, source_file, community? } … ],
///   "links":      [ { source, target, relation, confidence, confidence_score, weight } … ],
///   "hyperedges": [ { id, label, nodes, relation, confidence, confidence_score } … ],
///   "metadata":   { "ast_only": bool }
/// }
/// ```
use std::path::Path;

use serde_json::{json, Value};

use crate::model::graph::{GraphMetadata, GrapheniumGraph};
use crate::model::{Edge, HyperEdge, Node};

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Build a serde_json Value for the full graph (node-link format).
pub fn graph_to_value(graph: &GrapheniumGraph) -> Value {
    let nodes: Vec<Value> = graph
        .nodes()
        .map(|n| {
            json!({
                "id":              n.id,
                "label":           n.label,
                "file_type":       n.file_type.to_string(),
                "source_file":     n.source_file,
                "source_location": n.source_location,
                "community":       n.community,
            })
        })
        .collect();

    let links: Vec<Value> = graph
        .edges_iter()
        .map(|e| {
            json!({
                "source":           e.source,
                "target":           e.target,
                "relation":         e.relation,
                "confidence":       e.confidence.to_string(),
                "confidence_score": e.confidence_score,
                "weight":           e.weight,
                "source_file":      e.source_file,
            })
        })
        .collect();

    let hyperedges: Vec<Value> = graph
        .hyperedges
        .iter()
        .map(|h| {
            json!({
                "id":               h.id,
                "label":            h.label,
                "nodes":            h.nodes,
                "relation":         h.relation,
                "confidence":       h.confidence.to_string(),
                "confidence_score": h.confidence_score,
                "source_file":      h.source_file,
            })
        })
        .collect();

    // Build metadata object: start with the required fields, then add optional ones.
    let mut meta = json!({
        "ast_only": graph.is_ast_only(),
    });
    if let Some(ref v) = graph.metadata.schema_version {
        meta["schema_version"] = json!(v);
    }
    if let Some(ref v) = graph.metadata.graphenium_version {
        meta["graphenium_version"] = json!(v);
    }
    if let Some(ref v) = graph.metadata.created_at {
        meta["created_at"] = json!(v);
    }
    if let Some(ref v) = graph.metadata.project_root {
        meta["project_root"] = json!(v);
    }
    if let Some(ref v) = graph.metadata.extraction_modes {
        meta["extraction_modes"] = json!(v);
    }
    if let Some(ref v) = graph.metadata.languages {
        meta["languages"] = json!(v);
    }

    json!({
        "nodes":      nodes,
        "links":      links,
        "hyperedges": hyperedges,
        "metadata":   meta,
    })
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Pretty-printed JSON (for the `graph.json` file export).
pub fn to_json(graph: &GrapheniumGraph) -> crate::Result<String> {
    Ok(serde_json::to_string_pretty(&graph_to_value(graph))?)
}

/// Compact (no whitespace) JSON — used when embedding in HTML.
pub fn to_json_compact(graph: &GrapheniumGraph) -> crate::Result<String> {
    Ok(serde_json::to_string(&graph_to_value(graph))?)
}

/// Reconstruct a `GrapheniumGraph` from a node-link JSON file produced by [`to_json`].
///
/// Unknown or malformed entries are silently skipped so that a partially-updated
/// file does not prevent the graph from loading.
pub fn load_graph(path: &Path) -> crate::Result<GrapheniumGraph> {
    let content = std::fs::read_to_string(path)?;
    let v: Value = serde_json::from_str(&content)?;

    let mut graph = GrapheniumGraph::new();

    if let Some(ast_only) = v["metadata"]["ast_only"].as_bool() {
        graph.set_ast_only(ast_only);
    }

    if !v["metadata"].is_null() {
        match serde_json::from_value::<GraphMetadata>(v["metadata"].clone()) {
            Ok(metadata) => graph.metadata = metadata,
            Err(e) => eprintln!("[graphenium] warn: skip malformed metadata: {e}"),
        }
    }

    // ── Nodes ──────────────────────────────────────────────────────────────────
    if let Some(nodes) = v["nodes"].as_array() {
        for node_v in nodes {
            match serde_json::from_value::<Node>(node_v.clone()) {
                Ok(node) => graph.upsert_node(node),
                Err(e) => eprintln!("[graphenium] warn: skip malformed node: {e}"),
            }
        }
    }

    // ── Links → Edges ──────────────────────────────────────────────────────────
    if let Some(links) = v["links"].as_array() {
        for link_v in links {
            match serde_json::from_value::<Edge>(link_v.clone()) {
                Ok(edge) => {
                    graph.add_edge(edge);
                }
                Err(e) => eprintln!("[graphenium] warn: skip malformed link: {e}"),
            }
        }
    }

    // ── Hyperedges ─────────────────────────────────────────────────────────────
    if let Some(hes) = v["hyperedges"].as_array() {
        for he_v in hes {
            match serde_json::from_value::<HyperEdge>(he_v.clone()) {
                Ok(he) => graph.hyperedges.push(he),
                Err(e) => eprintln!("[graphenium] warn: skip malformed hyperedge: {e}"),
            }
        }
    }

    Ok(graph)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Edge, FileType, Node};

    fn make_graph() -> GrapheniumGraph {
        let mut g = GrapheniumGraph::new();
        let mut n = Node::new("mod_foo", "Foo", FileType::Code, "src/mod.rs");
        n.community = Some(0);
        g.upsert_node(n);
        g.upsert_node(Node::new(
            "mod_bar",
            "Bar",
            FileType::Document,
            "docs/bar.md",
        ));
        g.add_edge(Edge::extracted(
            "mod_foo",
            "mod_bar",
            "references",
            "src/mod.rs",
        ));
        g
    }

    #[test]
    fn json_has_required_keys() {
        let g = make_graph();
        let s = to_json(&g).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert!(v["nodes"].is_array());
        assert!(v["links"].is_array());
        assert!(v["hyperedges"].is_array());
        assert!(v["metadata"].is_object());
    }

    #[test]
    fn node_fields_present() {
        let mut g = make_graph();
        g.set_ast_only(true);
        let v: serde_json::Value = serde_json::from_str(&to_json(&g).unwrap()).unwrap();
        let node = v["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == "mod_foo")
            .unwrap()
            .clone();
        assert_eq!(node["label"], "Foo");
        assert_eq!(node["file_type"], "code");
        assert_eq!(node["community"], 0);
    }

    #[test]
    fn link_fields_present() {
        let g = make_graph();
        let v: serde_json::Value = serde_json::from_str(&to_json(&g).unwrap()).unwrap();
        let link = &v["links"][0];
        assert_eq!(link["source"], "mod_foo");
        assert_eq!(link["target"], "mod_bar");
        assert_eq!(link["confidence"], "EXTRACTED");
        assert!((link["confidence_score"].as_f64().unwrap() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn compact_is_valid_json() {
        let g = make_graph();
        let s = to_json_compact(&g).unwrap();
        assert!(serde_json::from_str::<serde_json::Value>(&s).is_ok());
        assert!(!s.contains('\n'));
    }

    #[test]
    fn empty_graph_serializes() {
        let g = GrapheniumGraph::new();
        let v: serde_json::Value = serde_json::from_str(&to_json(&g).unwrap()).unwrap();
        assert_eq!(v["nodes"].as_array().unwrap().len(), 0);
        assert_eq!(v["links"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn load_graph_roundtrip() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let json_path = tmp.path().join("graph.json");

        // Build a graph, export it, reload it
        let mut g = make_graph();
        g.set_ast_only(true);
        let raw: serde_json::Value = serde_json::from_str(&to_json(&g).unwrap()).unwrap();
        assert_eq!(raw["metadata"]["ast_only"], true);
        std::fs::write(&json_path, to_json(&g).unwrap()).unwrap();

        let loaded = load_graph(&json_path).unwrap();
        assert_eq!(loaded.node_count(), g.node_count());
        assert_eq!(loaded.edge_count(), g.edge_count());
        assert!(loaded.node_data("mod_foo").is_some());
        assert_eq!(loaded.node_data("mod_foo").unwrap().community, Some(0));
        assert!(loaded.is_ast_only());
    }

    #[test]
    fn load_graph_missing_file_errors() {
        let r = load_graph(std::path::Path::new("/nonexistent/graph.json"));
        assert!(r.is_err());
    }
}
