/// Rust custom extractor.
/// Rust's module structure is richer than the generic walker handles:
/// - `impl TypeName { fn ... }` — methods grouped by impl block
/// - `impl TraitName for TypeName { fn ... }` — trait implementations
/// - `struct`, `enum`, `trait` items at module scope
/// This extractor handles all of these correctly, building nodes and
/// `contains` / `method` edges.
use std::collections::{HashMap, HashSet};

use tree_sitter::{Node, Parser};

use crate::model::{make_id, Edge, ExtractionResult, FileType, Node as GNode};

use super::walker::{located_node, source_span};

// ── Public entry point ────────────────────────────────────────────────────────

pub fn extract(source: &[u8], file_path: &str) -> ExtractionResult {
    #[cfg(feature = "lang-rust")]
    {
        extract_inner(source, file_path)
    }
    #[cfg(not(feature = "lang-rust"))]
    {
        let _ = (source, file_path);
        ExtractionResult::new()
    }
}

// ── Implementation ─────────────────────────────────────────────────────────────

#[cfg(feature = "lang-rust")]
fn extract_inner(source: &[u8], file_path: &str) -> ExtractionResult {
    let mut parser = Parser::new();
    if let Err(e) = parser.set_language(&tree_sitter_rust::LANGUAGE.into()) {
        eprintln!(
            "[graphenium] warn: failed to set native Rust grammar for file '{}': {:?}",
            file_path, e
        );
        return ExtractionResult::new();
    }
    let Some(tree) = parser.parse(source, None) else {
        eprintln!(
            "[graphenium] warn: tree-sitter failed to parse Rust file '{}'",
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

    result.nodes.push(
        GNode::new(&file_node_id, file_stem, FileType::Code, file_path)
            .with_source_location(source_span(tree.root_node())),
    );

    let mut label_to_nid: HashMap<String, String> = HashMap::new();
    label_to_nid.insert(file_stem.to_string(), file_node_id.clone());

    let mut fn_bodies: Vec<(String, Node<'_>)> = Vec::new();
    let mut scope: Vec<String> = Vec::new();

    walk_node(
        tree.root_node(),
        source,
        file_path,
        file_stem,
        &file_node_id,
        &mut result,
        &mut label_to_nid,
        &mut fn_bodies,
        &mut scope,
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

#[cfg(feature = "lang-rust")]
fn walk_node<'tree>(
    node: Node<'tree>,
    source: &[u8],
    file_path: &str,
    file_stem: &str,
    file_node_id: &str,
    result: &mut ExtractionResult,
    label_to_nid: &mut HashMap<String, String>,
    fn_bodies: &mut Vec<(String, Node<'tree>)>,
    scope: &mut Vec<String>,
) {
    match node.kind() {
        // ── Type declarations ──────────────────────────────────────────────
        "struct_item" | "enum_item" | "trait_item" | "union_item" => {
            if let Some(name) = named_child_text(node, "name", source) {
                let node_id = make_id(&[file_stem, &name]);
                let gnode = located_node(&node_id, &name, FileType::Code, file_path, node);
                let qualified = qualify(scope, &name);
                let gnode = match qualified {
                    Some(q) => gnode.with_qualified_label(q),
                    None => gnode,
                };
                result.nodes.push(gnode);
                label_to_nid.insert(name.clone(), node_id.clone());
                result.edges.push(Edge::extracted(
                    file_node_id,
                    &node_id,
                    "contains",
                    file_path,
                ));
                // Descend into trait bodies for associated function signatures.
                if node.kind() == "trait_item" {
                    if let Some(body) = node.child_by_field_name("body") {
                        scope.push(name);
                        for i in 0..body.child_count() {
                            if let Some(child) = body.child(i) {
                                walk_node(
                                    child,
                                    source,
                                    file_path,
                                    file_stem,
                                    &node_id,
                                    result,
                                    label_to_nid,
                                    fn_bodies,
                                    scope,
                                );
                            }
                        }
                        scope.pop();
                    }
                }
            }
        }

        // ── impl blocks ───────────────────────────────────────────────────
        "impl_item" => {
            handle_impl(
                node,
                source,
                file_path,
                file_stem,
                file_node_id,
                result,
                label_to_nid,
                fn_bodies,
                scope,
            );
        }

        // ── Free functions ────────────────────────────────────────────────
        "function_item" => {
            handle_fn(
                node,
                source,
                file_path,
                file_stem,
                file_node_id,
                result,
                label_to_nid,
                fn_bodies,
                scope,
            );
        }

        // ── use declarations ──────────────────────────────────────────────
        "use_declaration" => {
            handle_use(node, source, file_path, file_node_id, result);
        }

        // ── mod items (descend) ────────────────────────────────────────────
        "mod_item" => {
            let mod_name = named_child_text(node, "name", source);
            if let Some(body) = node.child_by_field_name("body") {
                if let Some(n) = mod_name.as_deref() {
                    scope.push(n.to_string());
                }
                for i in 0..body.child_count() {
                    if let Some(child) = body.child(i) {
                        walk_node(
                            child,
                            source,
                            file_path,
                            file_stem,
                            file_node_id,
                            result,
                            label_to_nid,
                            fn_bodies,
                            scope,
                        );
                    }
                }
                if mod_name.is_some() {
                    scope.pop();
                }
            }
        }

        // ── Generic descent ────────────────────────────────────────────────
        _ => {
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
                        scope,
                    );
                }
            }
        }
    }
}

#[cfg(feature = "lang-rust")]
fn handle_impl<'tree>(
    node: Node<'tree>,
    source: &[u8],
    file_path: &str,
    file_stem: &str,
    file_node_id: &str,
    result: &mut ExtractionResult,
    label_to_nid: &mut HashMap<String, String>,
    fn_bodies: &mut Vec<(String, Node<'tree>)>,
    scope: &mut Vec<String>,
) {
    // `type` field = the type being implemented
    let Some(type_name) = node
        .child_by_field_name("type")
        .and_then(|n| leaf_text(n, source))
    else {
        return;
    };

    // Look up or create the type node
    let type_id = label_to_nid.get(&type_name).cloned().unwrap_or_else(|| {
        let id = make_id(&[file_stem, &type_name]);
        if !result.nodes.iter().any(|n| n.id == id) {
            let gnode = located_node(&id, &type_name, FileType::Code, file_path, node);
            let qualified = qualify(scope, &type_name);
            let gnode = match qualified {
                Some(q) => gnode.with_qualified_label(q),
                None => gnode,
            };
            result.nodes.push(gnode);
            result
                .edges
                .push(Edge::extracted(file_node_id, &id, "contains", file_path));
        }
        label_to_nid.insert(type_name.clone(), id.clone());
        id
    });

    // Walk the body with the type pushed onto the scope stack so that methods
    // get fully-qualified labels like `module::Type::method`.
    if let Some(body) = node.child_by_field_name("body") {
        scope.push(type_name);
        for i in 0..body.child_count() {
            if let Some(child) = body.child(i) {
                if child.kind() == "function_item" {
                    handle_fn(
                        child,
                        source,
                        file_path,
                        file_stem,
                        &type_id,
                        result,
                        label_to_nid,
                        fn_bodies,
                        scope,
                    );
                }
            }
        }
        scope.pop();
    }
}

#[cfg(feature = "lang-rust")]
fn handle_fn<'tree>(
    node: Node<'tree>,
    source: &[u8],
    file_path: &str,
    file_stem: &str,
    parent_id: &str,
    result: &mut ExtractionResult,
    label_to_nid: &mut HashMap<String, String>,
    fn_bodies: &mut Vec<(String, Node<'tree>)>,
    scope: &mut Vec<String>,
) {
    let Some(name) = named_child_text(node, "name", source) else {
        return;
    };
    let relation = if parent_id.contains(file_stem) && parent_id != &make_id(&[file_stem]) {
        "method"
    } else {
        "contains"
    };
    let node_id = make_id(&[parent_id, &name]);
    let gnode = located_node(&node_id, &name, FileType::Code, file_path, node);
    let qualified = qualify(scope, &name);
    let gnode = match qualified {
        Some(q) => gnode.with_qualified_label(q),
        None => gnode,
    };
    result.nodes.push(gnode);
    label_to_nid.insert(name, node_id.clone());
    result
        .edges
        .push(Edge::extracted(parent_id, &node_id, relation, file_path));

    if let Some(body) = node.child_by_field_name("body") {
        fn_bodies.push((node_id, body));
    }
}

/// Build a qualified label from the current Rust scope stack and a symbol
/// name. Returns `None` at top level (empty scope) so callers leave the
/// `qualified_label` field unset for already-unique symbols.
#[cfg(feature = "lang-rust")]
fn qualify(scope: &[String], name: &str) -> Option<String> {
    if scope.is_empty() {
        None
    } else {
        let mut parts: Vec<&str> = scope.iter().map(String::as_str).collect();
        parts.push(name);
        Some(parts.join("::"))
    }
}

#[cfg(feature = "lang-rust")]
fn handle_use(
    node: Node<'_>,
    source: &[u8],
    file_path: &str,
    file_node_id: &str,
    result: &mut ExtractionResult,
) {
    // use_declaration: `use` use_as_clause | scoped_identifier | ...
    // Extract the last segment as the imported name.
    if let Some(arg) = node.child_by_field_name("argument") {
        if let Some(name) = leaf_text(arg, source) {
            if name != "self" && name != "*" {
                let target_id = make_id(&[&name]);
                if !result.nodes.iter().any(|n| n.id == target_id) {
                    result
                        .nodes
                        .push(GNode::new(&target_id, &name, FileType::Code, file_path));
                }
                result.edges.push(Edge::extracted(
                    file_node_id,
                    &target_id,
                    "imports",
                    file_path,
                ));
            }
        }
    }
}

// ── Call-graph helpers ─────────────────────────────────────────────────────────

#[cfg(feature = "lang-rust")]
fn collect_calls<'tree>(
    node: Node<'tree>,
    owner_id: &str,
    source: &[u8],
    label_to_nid: &HashMap<String, String>,
    seen: &mut HashSet<(String, String)>,
    out: &mut Vec<Edge>,
) {
    let kind = node.kind();
    let is_call = kind == "call_expression" || kind == "method_call_expression";

    if is_call {
        let callee = if kind == "method_call_expression" {
            // method_call_expression: receiver . method arguments
            node.child_by_field_name("method")
                .and_then(|m| m.utf8_text(source).ok().map(|s| s.to_string()))
        } else {
            super::walker::get_call_target(node, source)
        };

        if let Some(label) = callee.clone() {
            if let Some(callee_id) = label_to_nid.get(&label) {
                let pair = (owner_id.to_string(), callee_id.clone());
                if seen.insert(pair) {
                    out.push(Edge::inferred_call(owner_id, callee_id, ""));
                }
            } else {
                // Emit unresolvable edge for cross-file call
                let mut edge = Edge::new(
                    owner_id.to_string(),
                    label.clone(),
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

// ── Name helpers ──────────────────────────────────────────────────────────────

fn named_child_text(node: Node<'_>, field: &str, source: &[u8]) -> Option<String> {
    node.child_by_field_name(field)
        .and_then(|n| n.utf8_text(source).ok().map(|s| s.to_string()))
}

/// Find the last (rightmost) leaf identifier in a (possibly complex) type expression.
fn leaf_text(node: Node<'_>, source: &[u8]) -> Option<String> {
    match node.kind() {
        "identifier" | "type_identifier" => node.utf8_text(source).ok().map(|s| s.to_string()),
        _ => {
            // Walk children in reverse to pick up e.g. `Vec<String>` -> "String"
            for i in (0..node.child_count()).rev() {
                if let Some(child) = node.child(i) {
                    if let Some(t) = leaf_text(child, source) {
                        return Some(t);
                    }
                }
            }
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "lang-rust")]
    #[test]
    fn struct_and_impl() {
        let src = b"struct Foo {}\nimpl Foo {\n    fn bar(&self) {}\n}\n";
        let r = extract(src, "foo.rs");

        assert!(
            r.nodes.iter().any(|n| n.label == "Foo"),
            "missing struct Foo"
        );
        assert!(
            r.nodes.iter().any(|n| n.label == "bar"),
            "missing method bar"
        );
        assert!(
            r.edges
                .iter()
                .any(|e| e.relation == "method" || e.relation == "contains"),
            "missing structural edge"
        );
    }

    #[cfg(feature = "lang-rust")]
    #[test]
    fn free_function() {
        let src = b"fn hello() {}\n";
        let r = extract(src, "lib.rs");

        assert!(
            r.nodes.iter().any(|n| n.label == "hello"),
            "missing fn hello"
        );
    }

    #[cfg(feature = "lang-rust")]
    #[test]
    fn methods_in_impl_are_qualified_with_type() {
        let src = b"struct Foo {}\nimpl Foo {\n    fn bar(&self) {}\n}\n";
        let r = extract(src, "foo.rs");
        let bar = r
            .nodes
            .iter()
            .find(|n| n.label == "bar")
            .expect("missing bar");
        assert_eq!(
            bar.qualified_label.as_deref(),
            Some("Foo::bar"),
            "methods should be qualified with their impl type"
        );
    }

    #[cfg(feature = "lang-rust")]
    #[test]
    fn top_level_free_function_has_no_qualified_label() {
        let src = b"fn hello() {}\n";
        let r = extract(src, "lib.rs");
        let hello = r
            .nodes
            .iter()
            .find(|n| n.label == "hello")
            .expect("missing hello");
        assert!(
            hello.qualified_label.is_none(),
            "top-level fn should have no qualified label, got: {:?}",
            hello.qualified_label
        );
    }

    #[cfg(feature = "lang-rust")]
    #[test]
    fn fn_inside_mod_is_qualified_with_mod_name() {
        let src = b"mod inner {\n    fn helper() {}\n}\n";
        let r = extract(src, "lib.rs");
        let helper = r
            .nodes
            .iter()
            .find(|n| n.label == "helper")
            .expect("missing helper");
        assert_eq!(helper.qualified_label.as_deref(), Some("inner::helper"));
    }

    #[cfg(feature = "lang-rust")]
    #[test]
    fn enum_extracted() {
        let src = b"enum Color { Red, Green, Blue }\n";
        let r = extract(src, "color.rs");

        assert!(
            r.nodes.iter().any(|n| n.label == "Color"),
            "missing enum Color"
        );
    }
}
