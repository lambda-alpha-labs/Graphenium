/// Python-only cross-file import resolution.
/// After all files have been individually extracted, this pass upgrades
/// generic `imports` edges into concrete `uses` INFERRED edges between
/// code entities in different files.
/// ## Algorithm (two passes)
/// **Pass 1** — Build a lookup map from `(module_stem, ClassName)` to `node_id`.
/// Iterate over all nodes whose `source_file` ends in `.py`, group by the
/// file stem, and record class-level node IDs.
/// **Pass 2** — For each `from X import Y` import edge (i.e. relation ==
/// "imports" whose target ID looks like `<module>_<name>`), find the concrete
/// node for `Y` in module `X` and emit a `uses` INFERRED edge from the
/// importing file's module node.
use std::collections::HashMap;
use std::path::Path;

use crate::model::{Confidence, Edge, ExtractionResult};

/// Resolve Python cross-file imports in `combined`.
/// This function is a no-op for non-Python files; calling it on a mixed
/// corpus is safe.
pub fn resolve_python_imports(combined: &mut ExtractionResult) {
    // Pass 1: build stem -> (short_label -> node_id) map for Python nodes.
    let mut stem_map: HashMap<String, HashMap<String, String>> = HashMap::new();

    for node in &combined.nodes {
        if !node.source_file.ends_with(".py") && !node.source_file.ends_with(".pyw") {
            continue;
        }
        let stem = file_stem(&node.source_file);
        stem_map
            .entry(stem)
            .or_default()
            .insert(node.label.clone(), node.id.clone());
    }

    // Pass 2: for each imports edge whose source is a Python file node and
    // whose target matches `<stem>_<label>`, emit a concrete `uses` edge.
    let mut new_edges: Vec<Edge> = Vec::new();

    for edge in &combined.edges {
        if edge.relation != "imports" {
            continue;
        }
        if !edge.source_file.ends_with(".py") && !edge.source_file.ends_with(".pyw") {
            continue;
        }

        // The target ID was synthesised as make_id(&[module, name]) during
        // import handling, e.g. "pathlib_path".
        let target_id = &edge.target;

        // Try to find the actual node that matches this target ID.
        if combined.nodes.iter().any(|n| &n.id == target_id) {
            // Already resolved (the placeholder matches a real node).
            // Upgrade to a `uses` INFERRED edge.
            let mut resolved = edge.clone();
            resolved.relation = "uses".to_string();
            resolved.confidence = Confidence::Inferred;
            resolved.confidence_score = Confidence::Inferred.default_score();
            new_edges.push(resolved);
        } else {
            // Try to split "module_name" into (module, name) and look up in stem_map.
            // We try all split points from right to left.
            let id_str = target_id.as_str();
            let mut resolved = false;
            for split in find_split_points(id_str) {
                let (mod_part, name_part) = (&id_str[..split], &id_str[split + 1..]);
                if let Some(labels) = stem_map.get(mod_part) {
                    // Case-insensitive label lookup (make_id lowercases everything)
                    let name_lower = name_part.to_lowercase();
                    if let Some(node_id) = labels.values().find(|id| {
                        combined
                            .nodes
                            .iter()
                            .any(|n| &n.id == *id && n.label.to_lowercase() == name_lower)
                    }) {
                        let mut e = edge.clone();
                        e.target = node_id.clone();
                        e.relation = "uses".to_string();
                        e.confidence = Confidence::Inferred;
                        e.confidence_score = Confidence::Inferred.default_score();
                        new_edges.push(e);
                        resolved = true;
                        break;
                    }
                }
            }
            // If still unresolved, keep the original `imports` edge as-is.
            // It will remain as a dangling edge and be dropped by `add_edge`.
            let _ = resolved;
        }
    }

    combined.edges.extend(new_edges);
}

/// All `_` split positions within `s`, from right to left.
fn find_split_points(s: &str) -> impl Iterator<Item = usize> + '_ {
    let positions: Vec<usize> = s
        .char_indices()
        .filter(|(_, c)| *c == '_')
        .map(|(i, _)| i)
        .collect();
    positions.into_iter().rev()
}

fn file_stem(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(path)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{FileType, Node};

    fn make_node(id: &str, label: &str, file: &str) -> Node {
        Node::new(id, label, FileType::Code, file)
    }

    #[test]
    fn resolves_known_import() {
        let mut result = ExtractionResult::new();

        // "utils.py" exports class Parser
        result
            .nodes
            .push(make_node("utils_parser", "Parser", "utils.py"));
        // "main.py" imports Parser from utils
        result.nodes.push(make_node("main", "main", "main.py"));

        // Synthesised import edge from main -> utils_parser
        let mut edge = Edge::extracted("main", "utils_parser", "imports", "main.py");
        edge.target = "utils_parser".to_string();
        result.edges.push(edge);

        resolve_python_imports(&mut result);

        // A `uses` edge should have been emitted
        assert!(
            result.edges.iter().any(|e| e.relation == "uses"),
            "expected a uses edge after resolution"
        );
    }

    #[test]
    fn non_python_files_ignored() {
        let mut result = ExtractionResult::new();
        result.nodes.push(make_node("foo_bar", "Bar", "foo.ts"));
        let edge = Edge::extracted("foo", "foo_bar", "imports", "foo.ts");
        result.edges.push(edge);

        let before = result.edges.len();
        resolve_python_imports(&mut result);
        // No new edges should be emitted for TypeScript
        assert_eq!(result.edges.len(), before);
    }
}
