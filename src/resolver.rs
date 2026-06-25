//! Cross-file resolution post-processor.
//!
//! After extraction results are ready but before graph assembly, this
//! module resolves import declarations against the known symbol index.
//!
//! Resolved edges get `resolution_status: "resolved"` and carry the
//! `extractor: "resolver"` provenance. Unresolved edges get
//! `resolution_status: "unresolved"`.

use std::collections::HashMap;

use crate::model::ExtractionResult;

/// Post-process extraction results to mark import resolution status.
///
/// Builds an export index from all results, then walks every edge:
/// - Import edges whose target name exists in the export index get
///   `resolved` status.
/// - Import edges whose target is absent get `unresolved` status.
/// - Non-import edges are left untouched.
pub fn resolve_imports(results: &mut [ExtractionResult]) {
    // Build export index: node ID -> ID and label -> ID
    let mut exports: HashMap<String, String> = HashMap::new();
    for result in results.iter() {
        for node in &result.nodes {
            exports
                .entry(node.id.clone())
                .or_insert_with(|| node.id.clone());
            exports
                .entry(node.label.clone())
                .or_insert_with(|| node.id.clone());
            if let Some(ref ql) = node.qualified_label {
                exports
                    .entry(ql.clone())
                    .or_insert_with(|| node.id.clone());
            }
        }
    }

    // Walk edges in all results, marking import resolution
    for result in results.iter_mut() {
        for edge in result.edges.iter_mut() {
            if edge.relation != "imports" {
                continue;
            }
            let status = if exports.contains_key(&edge.target) {
                "resolved"
            } else {
                "unresolved"
            };
            edge.extractor = Some("resolver".to_string());
            edge.resolution_status = Some(status.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Edge, FileType, Node};

    fn make_result(nodes: Vec<(&str, &str, &str)>, edges: Vec<(&str, &str)>) -> ExtractionResult {
        let mut r = ExtractionResult::new();
        for (id, label, file) in nodes {
            r.nodes
                .push(Node::new(id, label, FileType::Code, file));
        }
        for (src, tgt) in edges {
            r.edges
                .push(Edge::extracted(src, tgt, "imports", "file.rs"));
        }
        r
    }

    #[test]
    fn resolves_known_symbols() {
        let mut results = vec![make_result(
            vec![("a", "helper::A", "src/a.rs"), ("b", "B", "src/b.rs")],
            vec![("a", "b")],
        )];
        resolve_imports(&mut results);
        let edge = &results[0].edges[0];
        assert_eq!(edge.resolution_status, Some("resolved".to_string()));
        assert_eq!(edge.extractor, Some("resolver".to_string()));
    }

    #[test]
    fn unresolved_symbol_marked() {
        let mut results = vec![make_result(
            vec![("a", "A", "src/a.rs")],
            vec![("a", "does_not_exist")],
        )];
        resolve_imports(&mut results);
        let edge = &results[0].edges[0];
        assert_eq!(edge.resolution_status, Some("unresolved".to_string()));
    }

    #[test]
    fn non_import_edges_untouched() {
        let mut r = ExtractionResult::new();
        r.nodes
            .push(Node::new("a", "A", FileType::Code, "src/a.rs"));
        let mut edge = Edge::extracted("a", "b", "contains", "src/a.rs");
        edge.extractor = None;
        edge.resolution_status = None;
        r.edges.push(edge);

        let mut results = vec![r];
        resolve_imports(&mut results);
        assert!(results[0].edges[0].extractor.is_none());
    }
}
