/// Generic two-pass AST walker for all languages covered by `LanguageConfig`.
/// # Pass 1 – Structure
/// Walk the tree top-down.  On class/function nodes: emit a `Node` and a
/// `contains` / `method` `Edge`.  On import nodes: delegate to the language's
/// `import_handler`.  All other nodes are descended transparently.
/// # Pass 2 – Call graph
/// Re-walk each function body collected in Pass 1.  Match call targets against
/// the `label_to_nid` map built in Pass 1 and emit INFERRED `calls` edges
/// (weight = 0.8).  Deduplicated via a `HashSet<(String, String)>`.
use std::collections::{HashMap, HashSet};

use tree_sitter::{Node, Parser, Tree};

use crate::model::{make_id, Edge, ExtractionResult, FileType, Node as GNode};

use super::config::LanguageConfig;

pub fn source_span(node: Node<'_>) -> String {
    let start = node.start_position();
    let end = node.end_position();
    format!(
        "L{}:C{}-L{}:C{}",
        start.row + 1,
        start.column + 1,
        end.row + 1,
        end.column + 1
    )
}

pub fn located_node(
    id: &str,
    label: &str,
    file_type: FileType,
    source_file: &str,
    node: Node<'_>,
) -> GNode {
    GNode::new(id, label, file_type, source_file).with_source_location(source_span(node))
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Parse `source` with `config.language`, run the two-pass walk, and return
/// the populated `ExtractionResult`.
pub fn walk_file(source: &[u8], config: &LanguageConfig, file_path: &str) -> ExtractionResult {
    let mut parser = Parser::new();
    if let Err(e) = parser.set_language(&config.language) {
        eprintln!(
            "[graphenium] warn: failed to set tree-sitter language for file '{}': {:?}",
            file_path, e
        );
        return ExtractionResult::new();
    }

    let Some(tree) = parser.parse(source, None) else {
        eprintln!(
            "[graphenium] warn: tree-sitter failed to parse file '{}'",
            file_path
        );
        return ExtractionResult::new();
    };

    walk_tree(source, &tree, config, file_path)
}

/// Walk an already-parsed `tree`.  Called by `walk_file` and may be reused by
/// custom extractors (Go, Rust) when they want the generic logic.
pub fn walk_tree<'tree>(
    source: &[u8],
    tree: &'tree Tree,
    config: &LanguageConfig,
    file_path: &str,
) -> ExtractionResult {
    let file_stem = std::path::Path::new(file_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(file_path);

    let file_node_id = make_id(&[file_stem]);

    let mut state = WalkState {
        source,
        config,
        file_path,
        file_stem,
        file_node_id: file_node_id.clone(),
        label_to_nid: HashMap::new(),
        fn_bodies: Vec::new(),
        result: ExtractionResult::new(),
    };

    // Create the module node for this file.
    state.result.nodes.push(
        GNode::new(&file_node_id, file_stem, FileType::Code, file_path)
            .with_source_location(source_span(tree.root_node())),
    );
    state
        .label_to_nid
        .insert(file_stem.to_string(), file_node_id);

    // Pass 1: structure
    structure_walk(tree.root_node(), None, &mut state);

    // Pass 2: call graph.
    //
    // Take fn_bodies out so we can immutably borrow `state` for label lookup
    // while accumulating edges into a side-buffer.  After the loop, we extend
    // state.result with the buffered edges.
    let fn_bodies: Vec<(String, Node<'tree>)> = std::mem::take(&mut state.fn_bodies);
    let mut seen_calls: HashSet<(String, String)> = HashSet::new();
    let mut call_edges: Vec<Edge> = Vec::new();

    for (owner_id, body_node) in &fn_bodies {
        // `&state` (immutable) is released at end of each loop body.
        callgraph_walk(
            *body_node,
            owner_id,
            &state,
            &mut seen_calls,
            &mut call_edges,
        );
    }

    state.result.edges.extend(call_edges);
    state.result
}

// ── Internal state ─────────────────────────────────────────────────────────────

struct WalkState<'src, 'tree> {
    source: &'src [u8],
    config: &'src LanguageConfig,
    file_path: &'src str,
    file_stem: &'src str,
    file_node_id: String,
    /// Maps short label (class/function name) to its canonical node ID.
    label_to_nid: HashMap<String, String>,
    /// Function body nodes collected during Pass 1 for the call-graph pass.
    fn_bodies: Vec<(String, Node<'tree>)>,
    result: ExtractionResult,
}

// ── Pass 1: structure walk ─────────────────────────────────────────────────────

fn structure_walk<'src, 'tree>(
    node: Node<'tree>,
    parent_id: Option<&str>,
    state: &mut WalkState<'src, 'tree>,
) {
    let kind = node.kind();

    if state.config.class_kinds.contains(&kind) {
        handle_class(node, parent_id, state);
    } else if state.config.function_kinds.contains(&kind) {
        handle_function(node, parent_id, state);
    } else if state.config.import_kinds.contains(&kind) {
        if let Some(handler) = state.config.import_handler {
            let fid = state.file_node_id.clone();
            handler(node, state.source, state.file_path, &fid, &mut state.result);
        }
        // No recursion needed inside import nodes.
    } else {
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                structure_walk(child, parent_id, state);
            }
        }
    }
}

fn handle_class<'src, 'tree>(
    node: Node<'tree>,
    parent_id: Option<&str>,
    state: &mut WalkState<'src, 'tree>,
) {
    let effective_parent = parent_id.unwrap_or(&state.file_node_id).to_string();

    let node_id = match get_node_name(node, state.source, state.config.name_field) {
        Some(name) => {
            let id = make_id(&[state.file_stem, &name]);
            state.result.nodes.push(located_node(
                &id,
                &name,
                FileType::Code,
                state.file_path,
                node,
            ));
            state.label_to_nid.insert(name, id.clone());
            state.result.edges.push(Edge::extracted(
                &effective_parent,
                &id,
                "contains",
                state.file_path,
            ));
            id
        }
        // Anonymous class expression: inherit parent, still recurse children
        None => effective_parent.clone(),
    };

    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            structure_walk(child, Some(&node_id), state);
        }
    }
}

fn handle_function<'src, 'tree>(
    node: Node<'tree>,
    parent_id: Option<&str>,
    state: &mut WalkState<'src, 'tree>,
) {
    let effective_parent = parent_id.unwrap_or(&state.file_node_id).to_string();
    let relation = if parent_id.is_some() {
        "method"
    } else {
        "contains"
    };

    // Identify the body child first so we can skip it during non-body recursion.
    let body = node.child_by_field_name("body").or_else(|| {
        // Fallback: last named child that looks like a block
        (0..node.named_child_count())
            .rev()
            .find_map(|i| node.named_child(i))
            .filter(|n| {
                matches!(
                    n.kind(),
                    "block"
                        | "statement_block"
                        | "compound_statement"
                        | "suite"
                        | "body"
                        | "declaration_list"
                )
            })
    });

    let node_id = match get_node_name(node, state.source, state.config.name_field) {
        Some(name) => {
            let id = make_id(&[&effective_parent, &name]);
            state.result.nodes.push(located_node(
                &id,
                &name,
                FileType::Code,
                state.file_path,
                node,
            ));
            state.label_to_nid.insert(name, id.clone());
            state.result.edges.push(Edge::extracted(
                &effective_parent,
                &id,
                relation,
                state.file_path,
            ));

            if let Some(b) = body {
                state.fn_bodies.push((id.clone(), b));
            }
            id
        }
        // Anonymous function: collect body for call graph but use parent id
        None => {
            if let Some(b) = body {
                state.fn_bodies.push((effective_parent.clone(), b));
            }
            effective_parent.clone()
        }
    };

    // Recurse into non-body, non-parameter children (e.g. nested classes in Python).
    let body_id = body.map(|b| b.id());
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            // Skip body and parameter nodes.
            if body_id == Some(child.id()) {
                continue;
            }
            if matches!(
                child.kind(),
                "parameters" | "formal_parameters" | "parameter_list"
            ) {
                continue;
            }
            structure_walk(child, Some(&node_id), state);
        }
    }
}

// ── Pass 2: call-graph walk ────────────────────────────────────────────────────

fn callgraph_walk<'tree>(
    node: Node<'tree>,
    owner_id: &str,
    state: &WalkState<'_, 'tree>,
    seen: &mut HashSet<(String, String)>,
    out: &mut Vec<Edge>,
) {
    if state.config.call_kinds.contains(&node.kind()) {
        if let Some(callee_label) = get_call_target(node, state.source) {
            if let Some(callee_id) = state.label_to_nid.get(&callee_label) {
                let pair = (owner_id.to_string(), callee_id.clone());
                if seen.insert(pair) {
                    out.push(Edge::inferred_call(owner_id, callee_id, state.file_path));
                }
            }
        }
        // Descend into call arguments for nested calls.
    }

    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            callgraph_walk(child, owner_id, state, seen, out);
        }
    }
}

// ── Name extraction helpers ────────────────────────────────────────────────────

/// Get the name of a class or function node via the configured `name_field`.
/// Falls back to searching for the first leaf identifier (handles C/C++
/// nested declarator chains).
pub fn get_node_name(node: Node<'_>, source: &[u8], name_field: &str) -> Option<String> {
    if let Some(name_node) = node.child_by_field_name(name_field) {
        return extract_leaf_identifier(name_node, source);
    }
    // Secondary fallback: try "name" when the primary field is "declarator"
    // (class_specifier in C++ actually uses "name").
    if name_field == "declarator" {
        if let Some(name_node) = node.child_by_field_name("name") {
            return extract_leaf_identifier(name_node, source);
        }
    }
    None
}

/// Recursively descend until we find a leaf identifier node.
/// Handles C/C++ nested declarators: `*fn_name(...)` → "fn_name".
fn extract_leaf_identifier(node: Node<'_>, source: &[u8]) -> Option<String> {
    match node.kind() {
        "identifier"
        | "type_identifier"
        | "field_identifier"
        | "property_identifier"
        | "simple_identifier" => {
            return node.utf8_text(source).ok().map(|s| s.to_string());
        }
        _ => {}
    }
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if let Some(name) = extract_leaf_identifier(child, source) {
                return Some(name);
            }
        }
    }
    None
}

/// Get the callee label from a call expression.
/// Handles:
/// - `foo()`            → "foo"
/// - `obj.foo()`        → "foo"
/// - `Foo::bar()`       → "bar"  (Rust / C++)
pub fn get_call_target(call_node: Node<'_>, source: &[u8]) -> Option<String> {
    // Most languages: call has a "function" field (Python, JS, C)
    if let Some(fn_node) = call_node.child_by_field_name("function") {
        return extract_callee_name(fn_node, source);
    }
    // Java: method_invocation has "name" + optional "object" fields
    if let Some(name_node) = call_node.child_by_field_name("name") {
        return name_node.utf8_text(source).ok().map(|s| s.to_string());
    }
    // C#: invocation_expression — walk first named child
    if let Some(expr) = call_node.named_child(0) {
        return extract_callee_name(expr, source);
    }
    None
}

fn extract_callee_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    match node.kind() {
        "identifier" => node.utf8_text(source).ok().map(|s| s.to_string()),
        // obj.method  /  Cls.method  /  self.method
        "member_expression" | "attribute" | "field_expression" => node
            .child_by_field_name("property")
            .or_else(|| node.child_by_field_name("attribute"))
            .or_else(|| node.child_by_field_name("field"))
            .and_then(|p| p.utf8_text(source).ok().map(|s| s.to_string())),
        // Foo::bar  (Rust / C++)
        "scoped_identifier" | "qualified_identifier" => node
            .child_by_field_name("name")
            .and_then(|n| n.utf8_text(source).ok().map(|s| s.to_string()))
            .or_else(|| {
                (0..node.named_child_count())
                    .rev()
                    .find_map(|i| node.named_child(i))
                    .and_then(|c| c.utf8_text(source).ok().map(|s| s.to_string()))
            }),
        _ => None,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "lang-python")]
    mod python {
        use super::*;

        fn cfg() -> LanguageConfig {
            super::super::super::config::config_for_extension("py").unwrap()
        }

        #[test]
        fn class_and_method() {
            let cfg = cfg();
            let src = b"class Foo:\n    def bar(self):\n        pass\n";
            let r = walk_file(src, &cfg, "test.py");

            assert!(
                r.nodes.iter().any(|n| n.label == "Foo"),
                "missing class Foo"
            );
            assert!(
                r.nodes.iter().any(|n| n.label == "bar"),
                "missing method bar"
            );
            assert!(
                r.edges.iter().any(|e| e.relation == "contains"),
                "missing contains edge"
            );
            assert!(
                r.edges.iter().any(|e| e.relation == "method"),
                "missing method edge"
            );
        }

        #[test]
        fn call_graph() {
            let cfg = cfg();
            let src = b"def greet():\n    pass\n\ndef run():\n    greet()\n";
            let r = walk_file(src, &cfg, "test.py");

            assert!(
                r.edges.iter().any(|e| e.relation == "calls"),
                "expected a calls edge from run -> greet"
            );
        }

        #[test]
        fn imports_emitted() {
            let cfg = cfg();
            let src = b"import os\nfrom pathlib import Path\n";
            let r = walk_file(src, &cfg, "test.py");

            assert!(
                r.edges.iter().any(|e| e.relation == "imports"),
                "expected imports edges"
            );
        }

        #[test]
        fn no_duplicate_calls() {
            let cfg = cfg();
            let src = b"def helper():\n    pass\ndef run():\n    helper()\n    helper()\n";
            let r = walk_file(src, &cfg, "test.py");

            let calls: Vec<_> = r.edges.iter().filter(|e| e.relation == "calls").collect();
            assert_eq!(
                calls.len(),
                1,
                "duplicate call edges should be deduplicated"
            );
        }
    }
}
