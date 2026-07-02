/// Go custom extractor.
/// Go's `method_declaration` nodes include a receiver parameter list, e.g.:
/// ```go
/// func (r *Router) Handle(path string) { ... }
/// ```
/// The generic walker can extract functions and classes but does not know how
/// to associate a method with its receiver type.  This extractor reads the
/// receiver type and builds `method` edges from the type node to the method.
/// Top-level functions and structs are delegated to the generic walker after
/// the custom pass collects method information.
use std::collections::{HashMap, HashSet};

use tree_sitter::{Node, Parser};

use crate::model::{make_id, Edge, ExtractionResult, FileType, Node as GNode};

use super::import_handlers::go_import;
use super::walker::{located_node, source_span};

// ── Public entry point ────────────────────────────────────────────────────────

pub fn extract(source: &[u8], file_path: &str) -> ExtractionResult {
    #[cfg(feature = "lang-go")]
    {
        extract_inner(source, file_path)
    }
    #[cfg(not(feature = "lang-go"))]
    {
        let _ = (source, file_path);
        ExtractionResult::new()
    }
}

// ── Implementation ─────────────────────────────────────────────────────────────

#[cfg(feature = "lang-go")]
fn extract_inner(source: &[u8], file_path: &str) -> ExtractionResult {
    let mut parser = Parser::new();
    if let Err(e) = parser.set_language(&tree_sitter_go::LANGUAGE.into()) {
        eprintln!(
            "[graphenium] warn: failed to set native Go grammar for file '{}': {:?}",
            file_path, e
        );
        return ExtractionResult::new();
    }
    let Some(tree) = parser.parse(source, None) else {
        eprintln!(
            "[graphenium] warn: tree-sitter failed to parse Go file '{}'",
            file_path
        );
        return ExtractionResult::new();
    };

    let file_stem = std::path::Path::new(file_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(file_path);

    let file_node_id = make_id(&[file_stem]);
    let mut result = ExtractionResult::new();

    // Module node
    result.nodes.push(
        GNode::new(&file_node_id, file_stem, FileType::Code, file_path)
            .with_source_location(source_span(tree.root_node())),
    );

    let mut label_to_nid: HashMap<String, String> = HashMap::new();
    label_to_nid.insert(file_stem.to_string(), file_node_id.clone());

    let mut fn_bodies: Vec<(String, Node<'_>)> = Vec::new();
    let pkg = extract_package_name(tree.root_node(), source);

    walk_node(
        tree.root_node(),
        source,
        file_path,
        file_stem,
        &file_node_id,
        &mut result,
        &mut label_to_nid,
        &mut fn_bodies,
        pkg.as_deref(),
    );

    // Call-graph pass
    let mut seen: HashSet<(String, String)> = HashSet::new();
    let mut call_edges: Vec<Edge> = Vec::new();

    for (owner_id, body_node) in &fn_bodies {
        collect_calls(
            *body_node,
            owner_id,
            source,
            &label_to_nid,
            &mut seen,
            &mut call_edges,
        );
    }

    result.edges.extend(call_edges);
    result
}

#[cfg(feature = "lang-go")]
fn walk_node<'tree>(
    node: Node<'tree>,
    source: &[u8],
    file_path: &str,
    file_stem: &str,
    file_node_id: &str,
    result: &mut ExtractionResult,
    label_to_nid: &mut HashMap<String, String>,
    fn_bodies: &mut Vec<(String, Node<'tree>)>,
    pkg: Option<&str>,
) {
    match node.kind() {
        // ── Top-level function ─────────────────────────────────────────────
        "function_declaration" => {
            let Some(name) = field_text(node, "name", source) else {
                return descend(
                    node,
                    source,
                    file_path,
                    file_stem,
                    file_node_id,
                    result,
                    label_to_nid,
                    fn_bodies,
                    pkg,
                );
            };
            let node_id = make_id(&[file_stem, &name]);
            let gnode = located_node(&node_id, &name, FileType::Code, file_path, node);
            let gnode = match qualify(pkg, None, &name) {
                Some(q) => gnode.with_qualified_label(q),
                None => gnode,
            };
            result.nodes.push(gnode);
            label_to_nid.insert(name, node_id.clone());
            result.edges.push(Edge::extracted(
                file_node_id,
                &node_id,
                "contains",
                file_path,
            ));

            if let Some(body) = node.child_by_field_name("body") {
                fn_bodies.push((node_id, body));
            }
        }

        // ── Method with receiver ───────────────────────────────────────────
        "method_declaration" => {
            let Some(name) = field_text(node, "name", source) else {
                return;
            };
            // Receiver type: parameter_list -> parameter_declaration -> type
            let receiver_type = extract_receiver_type(node, source);
            let parent_label = receiver_type.as_deref().unwrap_or(file_stem);

            // Ensure the receiver type node exists (it may have been created
            // earlier as a struct; if not, create a placeholder).
            let parent_id = label_to_nid.get(parent_label).cloned().unwrap_or_else(|| {
                let pid = make_id(&[file_stem, parent_label]);
                if !result.nodes.iter().any(|n| n.id == pid) {
                    let gnode = located_node(&pid, parent_label, FileType::Code, file_path, node);
                    let gnode = match qualify(pkg, None, parent_label) {
                        Some(q) => gnode.with_qualified_label(q),
                        None => gnode,
                    };
                    result.nodes.push(gnode);
                    result
                        .edges
                        .push(Edge::extracted(file_node_id, &pid, "contains", file_path));
                }
                label_to_nid.insert(parent_label.to_string(), pid.clone());
                pid
            });

            let node_id = make_id(&[&parent_id, &name]);
            let gnode = located_node(&node_id, &name, FileType::Code, file_path, node);
            let gnode = match qualify(pkg, receiver_type.as_deref(), &name) {
                Some(q) => gnode.with_qualified_label(q),
                None => gnode,
            };
            result.nodes.push(gnode);
            label_to_nid.insert(name, node_id.clone());
            result
                .edges
                .push(Edge::extracted(&parent_id, &node_id, "method", file_path));

            if let Some(body) = node.child_by_field_name("body") {
                fn_bodies.push((node_id, body));
            }
        }

        // ── Type declarations (struct, interface) ──────────────────────────
        "type_declaration" => {
            // type_declaration -> type_spec -> type_identifier + type
            for i in 0..node.child_count() {
                if let Some(spec) = node.child(i) {
                    if spec.kind() == "type_spec" {
                        if let Some(name) = field_text(spec, "name", source) {
                            let node_id = make_id(&[file_stem, &name]);
                            let gnode =
                                located_node(&node_id, &name, FileType::Code, file_path, spec);
                            let gnode = match qualify(pkg, None, &name) {
                                Some(q) => gnode.with_qualified_label(q),
                                None => gnode,
                            };
                            result.nodes.push(gnode);
                            label_to_nid.insert(name, node_id.clone());
                            result.edges.push(Edge::extracted(
                                file_node_id,
                                &node_id,
                                "contains",
                                file_path,
                            ));
                        }
                    }
                }
            }
        }

        // ── Import declarations ────────────────────────────────────────────
        "import_declaration" => {
            go_import(node, source, file_path, file_node_id, result);
        }

        // ── Generic descent ────────────────────────────────────────────────
        _ => {
            descend(
                node,
                source,
                file_path,
                file_stem,
                file_node_id,
                result,
                label_to_nid,
                fn_bodies,
                pkg,
            );
        }
    }
}

#[cfg(feature = "lang-go")]
fn descend<'tree>(
    node: Node<'tree>,
    source: &[u8],
    file_path: &str,
    file_stem: &str,
    file_node_id: &str,
    result: &mut ExtractionResult,
    label_to_nid: &mut HashMap<String, String>,
    fn_bodies: &mut Vec<(String, Node<'tree>)>,
    pkg: Option<&str>,
) {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            walk_node(
                child,
                source,
                file_path,
                file_stem,
                file_node_id,
                result,
                label_to_nid,
                fn_bodies,
                pkg,
            );
        }
    }
}

/// Find the package name from a Go file's `package_clause`. Returns `None`
/// if the clause is missing or unreadable.
#[cfg(feature = "lang-go")]
fn extract_package_name(root: Node<'_>, source: &[u8]) -> Option<String> {
    for i in 0..root.child_count() {
        let child = root.child(i)?;
        if child.kind() == "package_clause" {
            for j in 0..child.child_count() {
                if let Some(c) = child.child(j) {
                    if c.kind() == "package_identifier" {
                        return c.utf8_text(source).ok().map(|s| s.to_string());
                    }
                }
            }
        }
    }
    None
}

/// Build a Go-style qualified label. Shape:
///   `pkg.Name`        — free function / type
///   `pkg.Receiver.Name` — method
/// Returns `None` when no package was recovered (leaves `qualified_label` unset).
#[cfg(feature = "lang-go")]
fn qualify(pkg: Option<&str>, receiver: Option<&str>, name: &str) -> Option<String> {
    let pkg = pkg?;
    Some(match receiver {
        Some(r) => format!("{pkg}.{r}.{name}"),
        None => format!("{pkg}.{name}"),
    })
}

#[cfg(feature = "lang-go")]
fn extract_receiver_type(method_node: Node<'_>, source: &[u8]) -> Option<String> {
    // receiver field -> parameter_list -> parameter_declaration -> type (pointer_type or type_identifier)
    let receiver = method_node.child_by_field_name("receiver")?;
    for i in 0..receiver.child_count() {
        if let Some(param) = receiver.child(i) {
            if param.kind() == "parameter_declaration" {
                if let Some(ty) = param.child_by_field_name("type") {
                    return extract_type_name(ty, source);
                }
            }
        }
    }
    None
}

#[cfg(feature = "lang-go")]
fn extract_type_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    match node.kind() {
        "type_identifier" => node.utf8_text(source).ok().map(|s| s.to_string()),
        // *TypeName (pointer receiver)
        "pointer_type" => {
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    if let Some(name) = extract_type_name(child, source) {
                        return Some(name);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// Get the text of a named field from a node.
fn field_text(node: Node<'_>, field: &str, source: &[u8]) -> Option<String> {
    let child = node.child_by_field_name(field)?;
    child.utf8_text(source).ok().map(|s| s.to_string())
}

#[cfg(feature = "lang-go")]
fn collect_calls<'tree>(
    node: Node<'tree>,
    owner_id: &str,
    source: &[u8],
    label_to_nid: &HashMap<String, String>,
    seen: &mut HashSet<(String, String)>,
    out: &mut Vec<Edge>,
) {
    if node.kind() == "call_expression" {
        if let Some(callee) = super::walker::get_call_target(node, source) {
            if let Some(callee_id) = label_to_nid.get(&callee) {
                let pair = (owner_id.to_string(), callee_id.clone());
                if seen.insert(pair) {
                    out.push(Edge::inferred_call(owner_id, callee_id, ""));
                }
            } else {
                // Emit unresolvable edge for cross-file call
                let mut edge = Edge::new(
                    owner_id.to_string(),
                    callee,
                    "calls",
                    crate::model::Confidence::Ambiguous,
                    "",
                );
                edge.extractor = Some("tree-sitter".to_string());
                edge.resolution_status = Some("unresolved".to_string());
                out.push(edge);
            }
        }
    }
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            collect_calls(child, owner_id, source, label_to_nid, seen, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "lang-go")]
    #[test]
    fn function_and_method() {
        let src = b"package main\n\
            type Server struct{}\n\
            func (s *Server) Run() {}\n\
            func NewServer() *Server { return &Server{} }\n";
        let r = extract(src, "server.go");

        assert!(
            r.nodes.iter().any(|n| n.label == "Server"),
            "missing Server type"
        );
        assert!(
            r.nodes.iter().any(|n| n.label == "Run"),
            "missing Run method"
        );
        assert!(
            r.nodes.iter().any(|n| n.label == "NewServer"),
            "missing NewServer function"
        );
        assert!(
            r.edges.iter().any(|e| e.relation == "method"),
            "missing method edge"
        );
    }

    #[cfg(feature = "lang-go")]
    #[test]
    fn method_qualified_with_package_and_receiver() {
        let src = b"package server\n\
            type Router struct{}\n\
            func (r *Router) Handle() {}\n";
        let r = extract(src, "router.go");
        let handle = r
            .nodes
            .iter()
            .find(|n| n.label == "Handle")
            .expect("missing Handle");
        assert_eq!(
            handle.qualified_label.as_deref(),
            Some("server.Router.Handle")
        );
    }

    #[cfg(feature = "lang-go")]
    #[test]
    fn free_function_qualified_with_package() {
        let src = b"package main\nfunc Run() {}\n";
        let r = extract(src, "main.go");
        let run = r.nodes.iter().find(|n| n.label == "Run").unwrap();
        assert_eq!(run.qualified_label.as_deref(), Some("main.Run"));
    }
}
