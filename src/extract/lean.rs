//! Pure-Rust Lean 4 (`.lean`) structural extractor.
//!
//! Extracts theorems, lemmas, definitions, structures, classes, inductive types,
//! axioms, imports, namespace scopes, theorem dependency edges, and flags
//! incomplete proofs (`sorry` / `admit`) as `Confidence::Ambiguous` edges so
//! `gm check` can gate formal verification repositories in CI.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::model::{make_id, Confidence, Edge, ExtractionResult, FileType, Node};

struct LeanDecl {
    node_id: String,
    label: String,
    start_line: usize,
    end_line: usize,
    body_tokens: Vec<String>,
    has_sorry: bool,
}

/// Extract Lean 4 declarations, imports, and proof dependencies from `source`.
pub fn extract(source: &[u8], file_path: &str) -> ExtractionResult {
    let text = String::from_utf8_lossy(source);
    let lines: Vec<&str> = text.lines().collect();

    let file_stem = Path::new(file_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(file_path);
    let file_node_id = make_id(&[file_stem]);

    let mut result = ExtractionResult::new();
    let total_lines = lines.len().max(1);
    result.nodes.push(
        Node::new(file_node_id.clone(), file_stem, FileType::Code, file_path)
            .with_source_location(format!("L1:C1-L{total_lines}:C1")),
    );

    let mut scope_stack: Vec<String> = Vec::new();
    let mut decls: Vec<LeanDecl> = Vec::new();
    let mut label_to_nid: HashMap<String, String> = HashMap::new();
    label_to_nid.insert(file_stem.to_string(), file_node_id.clone());

    let mut in_block_comment = 0usize;

    for (idx, &raw_line) in lines.iter().enumerate() {
        let line_num = idx + 1;
        let cleaned = strip_lean_comments(raw_line, &mut in_block_comment);
        let trimmed = cleaned.trim();
        if trimmed.is_empty() {
            continue;
        }

        // 1. Handle `import Foo.Bar.Baz`
        if let Some(rest) = trimmed.strip_prefix("import ") {
            for mod_path in rest.split_whitespace() {
                let mod_clean = mod_path.trim();
                if mod_clean.is_empty() {
                    continue;
                }
                let short_mod = mod_clean.rsplit('.').next().unwrap_or(mod_clean);
                let target_id = make_id(&[short_mod]);
                if !result.nodes.iter().any(|n| n.id == target_id) {
                    result.nodes.push(
                        Node::new(target_id.clone(), short_mod, FileType::Code, file_path)
                            .with_qualified_label(mod_clean),
                    );
                }
                result.edges.push(Edge::extracted(
                    file_node_id.as_str(),
                    target_id.as_str(),
                    "imports",
                    file_path,
                ));
            }
            continue;
        }

        // 2. Handle `namespace Foo` / `section Foo` / `end Foo`
        if let Some(ns) = trimmed.strip_prefix("namespace ") {
            let ns_name = ns.split_whitespace().next().unwrap_or("").trim();
            if !ns_name.is_empty() {
                scope_stack.push(ns_name.to_string());
            }
            continue;
        }
        if trimmed == "end" || trimmed.starts_with("end ") {
            scope_stack.pop();
            continue;
        }

        // 3. Detect top-level declaration keywords (after optional modifiers)
        if let Some((kw, name, after_name)) = parse_lean_decl_header(trimmed) {
            // Close previous declaration's end_line
            if let Some(prev) = decls.last_mut() {
                prev.end_line = line_num.saturating_sub(1).max(prev.start_line);
            }

            let node_id = make_id(&[file_stem, &name]);
            let qualified = if scope_stack.is_empty() {
                None
            } else {
                Some(format!("{}.{name}", scope_stack.join(".")))
            };

            let mut gnode = Node::new(node_id.clone(), name.as_str(), FileType::Code, file_path);
            if let Some(q) = qualified {
                gnode = gnode.with_qualified_label(q);
            }
            result.nodes.push(gnode);
            label_to_nid.insert(name.clone(), node_id.clone());

            let rel = if kw == "axiom" {
                "depends_on"
            } else {
                "contains"
            };
            result.edges.push(Edge::extracted(
                file_node_id.as_str(),
                node_id.as_str(),
                rel,
                file_path,
            ));

            let mut body_tokens = Vec::new();
            let mut has_sorry = false;
            collect_lean_tokens(after_name, &mut body_tokens, &mut has_sorry);

            decls.push(LeanDecl {
                node_id,
                label: name,
                start_line: line_num,
                end_line: total_lines,
                body_tokens,
                has_sorry,
            });
        } else if let Some(current) = decls.last_mut() {
            collect_lean_tokens(trimmed, &mut current.body_tokens, &mut current.has_sorry);
        }
    }

    // Update source_location spans on all extracted declaration nodes
    for d in &decls {
        if let Some(node) = result.nodes.iter_mut().find(|n| n.id == d.node_id) {
            node.source_location = format!("L{}:C1-L{}:C1", d.start_line, d.end_line);
        }
    }

    // Pass 2: Emit theorem/definition dependency edges and `unverified_sorry` flags
    let mut seen_calls: HashSet<(String, String)> = HashSet::new();
    for d in &decls {
        if d.has_sorry {
            let mut sorry_edge = Edge::new(
                d.node_id.as_str(),
                d.node_id.as_str(),
                "unverified_sorry",
                Confidence::Ambiguous,
                file_path,
            );
            sorry_edge.extractor = Some("lean-extractor".to_string());
            sorry_edge.resolution_status = Some("ambiguous".to_string());
            result.edges.push(sorry_edge);
        }

        for tok in &d.body_tokens {
            if tok == &d.label || is_lean_keyword(tok) {
                continue;
            }
            if let Some(target_id) = label_to_nid.get(tok) {
                if target_id != &d.node_id
                    && seen_calls.insert((d.node_id.clone(), target_id.clone()))
                {
                    result.edges.push(Edge::inferred_call(
                        d.node_id.as_str(),
                        target_id.as_str(),
                        file_path,
                    ));
                }
            } else if seen_calls.insert((d.node_id.clone(), tok.clone())) {
                let mut edge = Edge::new(
                    d.node_id.clone(),
                    tok.clone(),
                    "calls",
                    Confidence::Ambiguous,
                    file_path,
                );
                edge.extractor = Some("lean-extractor".to_string());
                edge.resolution_status = Some("unresolved".to_string());
                result.edges.push(edge);
            }
        }
    }

    result
}

fn strip_lean_comments(line: &str, block_depth: &mut usize) -> String {
    let mut out = String::with_capacity(line.len());
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if *block_depth > 0 {
            if i + 1 < chars.len() && chars[i] == '/' && chars[i + 1] == '-' {
                *block_depth += 1;
                i += 2;
            } else if i + 1 < chars.len() && chars[i] == '-' && chars[i + 1] == '/' {
                *block_depth -= 1;
                i += 2;
            } else {
                i += 1;
            }
        } else if i + 1 < chars.len() && chars[i] == '/' && chars[i + 1] == '-' {
            *block_depth += 1;
            i += 2;
        } else if i + 1 < chars.len() && chars[i] == '-' && chars[i + 1] == '-' {
            break; // line comment
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

fn parse_lean_decl_header(trimmed: &str) -> Option<(&str, String, &str)> {
    let mut rest = trimmed;
    loop {
        let mut stripped_mod = false;
        for modifier in &[
            "private ",
            "protected ",
            "noncomputable ",
            "partial ",
            "unsafe ",
            "@",
        ] {
            if let Some(after) = rest.strip_prefix(modifier) {
                rest = after.trim_start();
                stripped_mod = true;
            }
        }
        // Skip @[...] attributes at start of line
        if rest.starts_with('[') {
            if let Some(close) = rest.find(']') {
                rest = rest[close + 1..].trim_start();
                stripped_mod = true;
            }
        }
        if !stripped_mod {
            break;
        }
    }

    for &kw in &[
        "theorem",
        "lemma",
        "def",
        "abbrev",
        "structure",
        "class",
        "inductive",
        "axiom",
        "opaque",
    ] {
        if let Some(after_kw) = rest.strip_prefix(kw) {
            if after_kw.chars().next().is_some_and(char::is_whitespace) {
                let after_ws = after_kw.trim_start();
                let name_end = after_ws
                    .find(|c: char| c.is_whitespace() || matches!(c, '(' | '{' | '[' | ':'))
                    .unwrap_or(after_ws.len());
                let name = after_ws[..name_end].trim();
                if !name.is_empty()
                    && name
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_alphabetic() || c == '_')
                {
                    let short_name = name.rsplit('.').next().unwrap_or(name).to_string();
                    return Some((kw, short_name, &after_ws[name_end..]));
                }
            }
        }
    }
    None
}

fn collect_lean_tokens(segment: &str, out: &mut Vec<String>, has_sorry: &mut bool) {
    for raw_tok in segment.split(|c: char| !c.is_alphanumeric() && c != '_' && c != '.') {
        if raw_tok.is_empty() {
            continue;
        }
        let short = raw_tok.rsplit('.').next().unwrap_or(raw_tok);
        if short == "sorry" || short == "admit" {
            *has_sorry = true;
            continue;
        }
        if short.len() >= 3
            && short
                .chars()
                .next()
                .is_some_and(|c| c.is_alphabetic() || c == '_')
            && !is_lean_keyword(short)
        {
            out.push(short.to_string());
        }
    }
}

fn is_lean_keyword(word: &str) -> bool {
    matches!(
        word,
        "theorem"
            | "lemma"
            | "def"
            | "abbrev"
            | "structure"
            | "class"
            | "inductive"
            | "instance"
            | "axiom"
            | "opaque"
            | "where"
            | "by"
            | "rw"
            | "simp"
            | "exact"
            | "apply"
            | "intro"
            | "intros"
            | "have"
            | "let"
            | "show"
            | "from"
            | "fun"
            | "match"
            | "with"
            | "if"
            | "then"
            | "else"
            | "do"
            | "return"
            | "for"
            | "in"
            | "Prop"
            | "Type"
            | "Sort"
            | "Nat"
            | "Int"
            | "Real"
            | "Bool"
            | "String"
            | "List"
            | "true"
            | "false"
            | "norm_num"
            | "ring"
            | "linarith"
            | "decide"
            | "field_simp"
            | "dsimp"
            | "constructor"
            | "refine"
            | "obtain"
            | "calc"
            | "open"
            | "variable"
            | "namespace"
            | "section"
            | "end"
            | "import"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_lean_theorems_calls_and_sorry_flags() {
        let src = br#"
import Mathlib.Tactic.Ring

namespace Verification

def helper_val : Nat := 42

theorem main_safety : helper_val = 42 := by
  rfl

theorem unfinished_claim : 1 = 2 := by
  sorry

end Verification
"#;
        let r = extract(src, "src/Spec.lean");
        assert!(r.nodes.iter().any(|n| n.label == "helper_val"));
        let thm = r.nodes.iter().find(|n| n.label == "main_safety").unwrap();
        assert_eq!(
            thm.qualified_label.as_deref(),
            Some("Verification.main_safety")
        );
        assert!(r.edges.iter().any(|e| e.relation == "imports"));
        assert!(
            r.edges
                .iter()
                .any(|e| e.relation == "calls" && e.source.contains("main_safety")),
            "main_safety should reference helper_val"
        );
        assert!(
            r.edges
                .iter()
                .any(|e| e.relation == "unverified_sorry" && e.confidence == Confidence::Ambiguous),
            "unfinished_claim with sorry must emit an Ambiguous unverified_sorry edge"
        );
    }
}
