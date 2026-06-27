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
            let mut obj = json!({
                "id":              n.id,
                "label":           n.label,
                "file_type":       n.file_type.to_string(),
                "source_file":     n.source_file,
                "source_location": n.source_location,
                "community":       n.community,
            });
            if let Some(ref v) = n.extractor {
                obj["extractor"] = json!(v);
            }
            if let Some(ref v) = n.resolution_status {
                obj["resolution_status"] = json!(v);
            }
            obj
        })
        .collect();

    let links: Vec<Value> = graph
        .edges_iter()
        .map(|e| {
            let mut obj = json!({
                "source":           e.source,
                "target":           e.target,
                "relation":         e.relation,
                "confidence":       e.confidence.to_string(),
                "confidence_score": e.confidence_score,
                "weight":           e.weight,
                "source_file":      e.source_file,
            });
            if let Some(ref v) = e.extractor {
                obj["extractor"] = json!(v);
            }
            if let Some(ref v) = e.resolution_status {
                obj["resolution_status"] = json!(v);
            }
            obj
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

// ── Quality report ─────────────────────────────────────────────────────────

/// Generate a structured quality report for the graph.
pub fn generate_quality_report(graph: &GrapheniumGraph) -> serde_json::Value {
    let mut by_file: Vec<serde_json::Value> = Vec::new();
    let mut total_resolved = 0usize;
    let mut total_unresolved = 0usize;
    let mut total_extracted = 0usize;
    let mut total_inferred = 0usize;
    let mut total_ambiguous = 0usize;
    let mut total_imports = 0usize;

    // Per-file stats
    let mut file_stats: std::collections::BTreeMap<String, (usize, usize, usize)> =
        std::collections::BTreeMap::new();
    for edge in graph.edges_iter() {
        let stats = file_stats.entry(edge.source_file.clone()).or_default();
        stats.0 += 1; // total edges
        match edge.confidence {
            crate::model::Confidence::Extracted => total_extracted += 1,
            crate::model::Confidence::Inferred => total_inferred += 1,
            crate::model::Confidence::Ambiguous => total_ambiguous += 1,
        }
        if edge.relation == "imports" {
            total_imports += 1;
            match edge.resolution_status.as_deref() {
                Some("resolved") => total_resolved += 1,
                Some("unresolved") => {
                    total_unresolved += 1;
                    stats.1 += 1;
                }
                _ => {}
            }
        }
    }

    for (file, (n, unresolved, _)) in &file_stats {
        if *unresolved > 0 {
            by_file.push(serde_json::json!({
                "file": file,
                "total_edges": n,
                "unresolved_refs": unresolved,
            }));
        }
    }
    by_file.sort_by(|a, b| {
        b["unresolved_refs"]
            .as_u64()
            .cmp(&a["unresolved_refs"].as_u64())
    });
    by_file.truncate(20);

    let resolution_ratio = if total_imports > 0 {
        total_resolved as f64 / total_imports as f64
    } else {
        1.0
    };

    let mut recommended_commands = Vec::new();
    if total_unresolved > 0 {
        recommended_commands.push("gm doctor --resolution".to_string());
    }
    if total_ambiguous > 0 {
        recommended_commands.push("gm check".to_string());
    }

    serde_json::json!({
        "schema_version": "quality.v1",
        "generated_at": chrono_or_fallback(),
        "graph_schema_version": graph.metadata.schema_version,
        "summary": {
            "nodes": graph.node_count(),
            "edges": graph.edge_count(),
            "resolved_references": total_resolved,
            "unresolved_references": total_unresolved,
            "resolution_ratio": (resolution_ratio * 1000.0).round() / 1000.0,
            "extracted_edges": total_extracted,
            "inferred_edges": total_inferred,
            "ambiguous_edges": total_ambiguous,
        },
        "by_file": by_file,
        "by_relation": [],
        "top_risks": [],
        "recommended_commands": recommended_commands,
    })
}

/// Get an ISO timestamp without the chrono crate dependency.
fn chrono_or_fallback() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let days = secs / 86400;
    let time = secs % 86400;
    let h = time / 3600;
    let m = (time % 3600) / 60;
    let s = time % 60;
    let mut y = 1970i64;
    let mut rem = days as i64;
    loop {
        let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
        let dim = if leap { 366 } else { 365 };
        if rem < dim {
            break;
        }
        rem -= dim;
        y += 1;
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let md = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut mo = 0usize;
    for &d in &md {
        if rem < d {
            break;
        }
        rem -= d;
        mo += 1;
    }
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        mo + 1,
        rem + 1,
        h,
        m,
        s
    )
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
