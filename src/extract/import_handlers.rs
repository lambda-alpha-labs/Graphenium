use crate::model::{make_id, normalize_label};
/// Per-language import node handlers.
///
/// Each function matches the `ImportHandlerFn` signature and is called once
/// per import node encountered during the structure pass.  Handlers push
/// `imports` edges onto `result`; dangling edges (where the target node does
/// not yet exist) are silently dropped by `GrapheniumGraph::add_edge` in the
/// build phase.
///
/// Nodes for imported names are **not** created here — only edges from the
/// current file's module node to the imported identifier.  Actual cross-file
/// resolution for Python happens in `cross_file.rs`.
use crate::model::{Edge, ExtractionResult, FileType, Node};

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Extract the UTF-8 text of a tree-sitter node.  Returns `None` on any error.
fn text<'t>(node: tree_sitter::Node<'t>, source: &'t [u8]) -> Option<&'t str> {
    node.utf8_text(source).ok()
}

/// Find the first child whose kind matches `kind`.
fn first_child_of_kind<'t>(
    node: tree_sitter::Node<'t>,
    kind: &str,
) -> Option<tree_sitter::Node<'t>> {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == kind {
                return Some(child);
            }
        }
    }
    None
}

/// Emit an `imports` EXTRACTED edge from the file node to a target node whose
/// ID is `make_id(&[target])`.  A placeholder node is inserted so the edge
/// is not dangling when the graph is built from this single file's result;
/// cross-file resolution may replace it later.
fn emit_import(
    source_id: &str,
    target_label: &str,
    file_path: &str,
    result: &mut ExtractionResult,
) {
    let target_label = normalize_label(target_label);
    if target_label.is_empty() {
        return;
    }
    let target_id = make_id(&[&target_label]);

    // Insert a placeholder so the edge is not dangling within this result.
    // The build phase's last-write-wins will let a richer node (from the
    // target file's extraction) override this placeholder later.
    if !result.nodes.iter().any(|n| n.id == target_id) {
        result.nodes.push(Node::new(
            &target_id,
            &target_label,
            FileType::Code,
            file_path,
        ));
    }

    let edge = Edge::extracted(source_id, &target_id, "imports", file_path);
    result.edges.push(edge);
}

// ── Python ────────────────────────────────────────────────────────────────────

/// Python: `import X [as Y]` and `from X import Y [as Z], ...`
///
/// Grammar:
/// - `import_statement`:      `import` ( dotted_name | aliased_import )+
/// - `import_from_statement`: `from` dotted_name `import` ( `*` | import_list | dotted_name )
pub fn python_import(
    node: tree_sitter::Node<'_>,
    source: &[u8],
    file_path: &str,
    file_node_id: &str,
    result: &mut ExtractionResult,
) {
    match node.kind() {
        "import_statement" => {
            // Children: keyword + one or more dotted_name / aliased_import
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    match child.kind() {
                        "dotted_name" => {
                            if let Some(t) = text(child, source) {
                                emit_import(file_node_id, t, file_path, result);
                            }
                        }
                        "aliased_import" => {
                            // aliased_import: dotted_name `as` identifier
                            if let Some(name) = child.child_by_field_name("name") {
                                if let Some(t) = text(name, source) {
                                    emit_import(file_node_id, t, file_path, result);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        "import_from_statement" => {
            // from <module> import <names>
            if let Some(module) = node.child_by_field_name("module_name") {
                if let Some(mod_text) = text(module, source) {
                    // Emit edge to the module itself
                    emit_import(file_node_id, mod_text, file_path, result);

                    // Also emit per-name edges so cross_file can resolve them
                    // e.g. `from mymod import Foo` -> label `mymod.Foo`, ID `mymod_foo`
                    for i in 0..node.child_count() {
                        if let Some(child) = node.child(i) {
                            if child.kind() == "import_from_as_clause"
                                || child.kind() == "dotted_name"
                                || child.kind() == "identifier"
                            {
                                if let Some(name) = child.child_by_field_name("name") {
                                    if let Some(nm) = text(name, source) {
                                        let combined = format!("{mod_text}.{nm}");
                                        emit_import(file_node_id, &combined, file_path, result);
                                    }
                                } else if child.kind() == "identifier" {
                                    if let Some(nm) = text(child, source) {
                                        let combined = format!("{mod_text}.{nm}");
                                        emit_import(file_node_id, &combined, file_path, result);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

// ── JavaScript / TypeScript ───────────────────────────────────────────────────

/// JS/TS: ES module imports and exports (all forms with a `source` field).
///
/// Handles:
/// - `import X from "module"`           (default import)
/// - `import { X } from "module"`       (named import)
/// - `import * as X from "module"`      (namespace import)
/// - `import "module"`                  (side-effect import)
/// - `export { X } from "module"`       (named re-export)
/// - `export * from "module"`           (star re-export)
///
/// Only processes `import_statement` and `export_statement` nodes that have
/// a `source` child field (the module specifier string).
pub fn es_import_handler(
    node: tree_sitter::Node<'_>,
    source: &[u8],
    file_path: &str,
    file_node_id: &str,
    result: &mut ExtractionResult,
) {
    let src = match node.kind() {
        "import_statement" | "export_statement" => node.child_by_field_name("source"),
        _ => None,
    };
    let Some(src) = src else {
        return;
    };
    let Some(raw) = text(src, source) else {
        return;
    };
    // Strip surrounding quotes
    let module = raw.trim_matches(|c| c == '"' || c == '\'' || c == '`');
    // Normalise path: take the last component, strip extension
    let stem = module
        .rsplit('/')
        .next()
        .unwrap_or(module)
        .split('.')
        .next()
        .unwrap_or(module);
    emit_import(file_node_id, stem, file_path, result);
}

/// JS/TS: CommonJS `require("module")` calls.
///
/// Matches `call_expression` nodes whose function name is `"require"`.
/// Extracts the first string argument as the imported module path.
pub fn cjs_require_handler(
    node: tree_sitter::Node<'_>,
    source: &[u8],
    file_path: &str,
    file_node_id: &str,
    result: &mut ExtractionResult,
) {
    if node.kind() != "call_expression" {
        return;
    }
    // Check the `function` field is an identifier named "require"
    let func_node = match node.child_by_field_name("function") {
        Some(f) => f,
        None => return,
    };
    if text(func_node, source) != Some("require") {
        return;
    }
    // Get the arguments node
    let args_node = match node.child_by_field_name("arguments") {
        Some(a) => a,
        None => return,
    };
    // Find the first string/template argument
    for i in 0..args_node.named_child_count() {
        if let Some(arg) = args_node.named_child(i) {
            if matches!(arg.kind(), "string" | "template_string") {
                if let Some(raw) = text(arg, source) {
                    let module = raw.trim_matches(|c| c == '"' || c == '\'' || c == '`');
                    let stem = module
                        .rsplit('/')
                        .next()
                        .unwrap_or(module)
                        .split('.')
                        .next()
                        .unwrap_or(module);
                    emit_import(file_node_id, stem, file_path, result);
                }
                break;
            }
        }
    }
}

/// Combined JS/TS import handler that dispatches to the appropriate sub-handler
/// based on node kind.
///
/// This is the function registered in `LanguageConfig::import_handler`.
/// It routes to `es_import_handler` for `import_statement`/`export_statement`
/// and to `cjs_require_handler` for `call_expression` (`require(...)`).
pub fn js_import_handler(
    node: tree_sitter::Node<'_>,
    source: &[u8],
    file_path: &str,
    file_node_id: &str,
    result: &mut ExtractionResult,
) {
    match node.kind() {
        "import_statement" | "export_statement" => {
            es_import_handler(node, source, file_path, file_node_id, result);
        }
        "call_expression" => {
            cjs_require_handler(node, source, file_path, file_node_id, result);
        }
        _ => {}
    }
}

// ── Java ──────────────────────────────────────────────────────────────────────

/// Java: `import com.example.Foo;`
///
/// The fully-qualified name is a `scoped_identifier`; we preserve the full
/// import path as the placeholder label and normalize the ID from it.
pub fn java_import(
    node: tree_sitter::Node<'_>,
    source: &[u8],
    file_path: &str,
    file_node_id: &str,
    result: &mut ExtractionResult,
) {
    if node.kind() != "import_declaration" {
        return;
    }
    // Prefer the full qualified import for a more semantic placeholder label.
    let mut imported_name: Option<String> = None;
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            match child.kind() {
                "identifier" | "scoped_identifier" => {
                    if let Some(t) = text(child, source) {
                        imported_name = Some(t.trim_end_matches(".*").to_string());
                    }
                }
                _ => {}
            }
        }
    }
    if let Some(name) = imported_name {
        emit_import(file_node_id, &name, file_path, result);
    }
}

// ── C / C++ ───────────────────────────────────────────────────────────────────

/// C/C++: `#include <stdio.h>` or `#include "myheader.h"`
///
/// Extracts the header filename stem as the import target.
pub fn c_include(
    node: tree_sitter::Node<'_>,
    source: &[u8],
    file_path: &str,
    file_node_id: &str,
    result: &mut ExtractionResult,
) {
    if node.kind() != "preproc_include" {
        return;
    }
    // Children are: #include  <string_literal | system_lib_string>
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if matches!(child.kind(), "string_literal" | "system_lib_string") {
                if let Some(raw) = text(child, source) {
                    // Strip < >, " ", take the filename stem
                    let inner = raw.trim_matches(|c| c == '"' || c == '<' || c == '>' || c == '\'');
                    let stem = inner
                        .rsplit('/')
                        .next()
                        .unwrap_or(inner)
                        .split('.')
                        .next()
                        .unwrap_or(inner);
                    emit_import(file_node_id, stem, file_path, result);
                }
            }
        }
    }
}

// ── Go ────────────────────────────────────────────────────────────────────────

/// Go: `import "fmt"` or `import ( "fmt"; "os" )`
///
/// The generic walker handles `import_spec` nodes; Go's custom extractor
/// (`go.rs`) calls this helper directly.
pub fn go_import(
    node: tree_sitter::Node<'_>,
    source: &[u8],
    file_path: &str,
    file_node_id: &str,
    result: &mut ExtractionResult,
) {
    match node.kind() {
        "import_declaration" => {
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    go_import(child, source, file_path, file_node_id, result);
                }
            }
        }
        "import_spec" => {
            // import_spec: [alias] interpreted_string_literal
            if let Some(path_node) = first_child_of_kind(node, "interpreted_string_literal") {
                if let Some(raw) = text(path_node, source) {
                    let inner = raw.trim_matches('"');
                    let stem = inner.rsplit('/').next().unwrap_or(inner);
                    emit_import(file_node_id, stem, file_path, result);
                }
            }
        }
        _ => {}
    }
}

// ── C# ────────────────────────────────────────────────────────────────────────

/// C#: `using System.Collections.Generic;`
///
/// Preserves the full namespace as the placeholder label.
pub fn csharp_using(
    node: tree_sitter::Node<'_>,
    source: &[u8],
    file_path: &str,
    file_node_id: &str,
    result: &mut ExtractionResult,
) {
    if node.kind() != "using_directive" {
        return;
    }
    // Prefer the full qualified namespace for a more semantic placeholder label.
    let mut candidate: Option<&str> = None;
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == "qualified_name" {
                if let Some(t) = text(child, source) {
                    emit_import(file_node_id, t, file_path, result);
                    return;
                }
            }
            if child.kind() == "identifier" {
                if let Some(t) = text(child, source) {
                    candidate = Some(t);
                }
            }
        }
    }
    if let Some(t) = candidate {
        emit_import(file_node_id, t, file_path, result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_import_preserves_qualified_label() {
        let mut result = ExtractionResult::new();
        emit_import(
            "file_node",
            "  `System.Collections.Generic`  ",
            "src/File.cs",
            &mut result,
        );

        assert_eq!(result.nodes.len(), 1);
        assert_eq!(result.nodes[0].label, "System.Collections.Generic");
        assert_eq!(result.nodes[0].id, "system_collections_generic");
        assert_eq!(result.edges[0].target, "system_collections_generic");
    }

    #[test]
    fn emit_import_normalizes_wrapped_header_label() {
        let mut result = ExtractionResult::new();
        emit_import("file_node", "<stdio.h>", "src/main.c", &mut result);

        assert_eq!(result.nodes[0].label, "stdio.h");
        assert_eq!(result.nodes[0].id, "stdio_h");
    }
}
