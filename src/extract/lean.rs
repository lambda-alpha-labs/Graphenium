//! Pure-Rust Lean 4 (`.lean`) structural extractor.
//!
//! Extracts theorems, lemmas, definitions, structures, classes, inductive types,
//! axioms, imports, namespace scopes, theorem dependency edges, and flags
//! incomplete proofs (`sorry` / `admit`) as `Confidence::Ambiguous` edges so
//! `gm check` can gate formal verification repositories in CI.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::model::{make_id, Confidence, Edge, ExtractionResult, FileType, Node};

/// Deterministic 32-bit FNV-1a hash, used only to disambiguate names that
/// `make_id` would otherwise collapse (apostrophes, Unicode subscripts, …).
fn stable_hash(value: &str) -> u32 {
    let mut hash: u32 = 0x811c9dc5;
    for byte in value.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x01000193);
    }
    hash
}

/// Build an ID from Lean name parts, adding a stable suffix when a part
/// contains characters that `make_id` would normalize away. Ordinary names
/// keep their familiar IDs; `actionTensorCAlg'` no longer collides with
/// `actionTensorCAlg`.
fn lean_id(parts: &[&str]) -> String {
    let base = make_id(parts);
    let lossy = parts.iter().any(|part| {
        part.chars()
            .any(|c| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '.')))
    });
    if lossy {
        format!("{base}_{:08x}", stable_hash(&parts.join("\u{1f}")))
    } else {
        base
    }
}

struct LeanDecl {
    node_id: String,
    label: String,
    kind: String,
    start_line: usize,
    end_line: usize,
    body_tokens: Vec<String>,
    /// Raw declaration text from the header/signature region. Used to recover
    /// binder types `(h : T)` / `[Fact P]` before the `:=` body.
    header_text: String,
    has_sorry: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ScopeKind {
    Namespace,
    Section,
}

struct ScopeEntry {
    kind: ScopeKind,
    /// Namespace name; sections never contribute to the qualified name.
    name: Option<String>,
}

/// Extract Lean 4 declarations, imports, and proof dependencies from `source`.
pub fn extract(source: &[u8], file_path: &str) -> ExtractionResult {
    extract_with_root(source, file_path, None)
}

/// Like [`extract`], but derives module-qualified node IDs relative to
/// `project_root`. This keeps same-named files such as the twenty different
/// `Basic.lean` files in a large Lean project from collapsing into one node.
pub fn extract_with_root(
    source: &[u8],
    file_path: &str,
    project_root: Option<&Path>,
) -> ExtractionResult {
    let text = String::from_utf8_lossy(source);
    let lines: Vec<&str> = text.lines().collect();

    let file_stem = Path::new(file_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(file_path);
    let module_path = lean_module_path(file_path, project_root);
    let file_node_id = lean_id(&[module_path.as_str()]);

    let mut result = ExtractionResult::new();
    let total_lines = lines.len().max(1);
    result.nodes.push(
        Node::new(file_node_id.clone(), file_stem, FileType::Code, file_path)
            .with_source_location(format!("L1:C1-L{total_lines}:C1")),
    );

    let mut scope_stack: Vec<ScopeEntry> = Vec::new();
    let mut decls: Vec<LeanDecl> = Vec::new();
    let mut label_to_nid: HashMap<String, String> = HashMap::new();
    label_to_nid.insert(file_stem.to_string(), file_node_id.clone());
    label_to_nid.insert(module_path.clone(), file_node_id.clone());

    let mut in_block_comment = 0usize;
    let mut anonymous_instance_counts: HashMap<String, usize> = HashMap::new();

    for (idx, &raw_line) in lines.iter().enumerate() {
        let line_num = idx + 1;
        let cleaned = strip_lean_comments(raw_line, &mut in_block_comment);
        let trimmed = cleaned.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Strip attributes/visibility first so `public import`, `@[expose]
        // public section`, and `private theorem` are all understood.
        let prefixed = strip_lean_prefixes(trimmed);

        // 1. Handle `import Foo.Bar.Baz`, `public import Foo`, etc.
        if let Some(rest) = prefixed.strip_prefix("import ") {
            for mod_path in rest.split_whitespace() {
                let mod_clean = mod_path.trim();
                if mod_clean.is_empty() {
                    continue;
                }
                let short_mod = mod_clean.rsplit('.').next().unwrap_or(mod_clean);
                // Use the full module path, not the final component: `import
                // Foo.Basic` and `import Bar.Basic` must not both target `basic`.
                let target_id = lean_id(&[mod_clean]);
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
        if prefixed.is_empty() {
            continue;
        }

        // 2. Handle `namespace Foo` / `section Foo` / `end Foo`
        if let Some(ns) = prefixed.strip_prefix("namespace ") {
            let ns_name = ns.split_whitespace().next().unwrap_or("").trim();
            if !ns_name.is_empty() {
                scope_stack.push(ScopeEntry {
                    kind: ScopeKind::Namespace,
                    name: Some(ns_name.to_string()),
                });
            }
            continue;
        }
        if prefixed == "section" || prefixed.starts_with("section ") {
            let section_name = prefixed.strip_prefix("section").unwrap_or("").trim();
            scope_stack.push(ScopeEntry {
                kind: ScopeKind::Section,
                name: (!section_name.is_empty()).then(|| section_name.to_string()),
            });
            continue;
        }
        if prefixed == "end" || prefixed.starts_with("end ") {
            scope_stack.pop();
            continue;
        }

        // 3. Detect top-level declaration keywords (after optional modifiers)
        if let Some((kw, mut declared_name, mut short_name, after_name, anonymous_sig)) =
            parse_lean_decl_header(trimmed)
        {
            if let Some(sig) = anonymous_sig {
                // Anonymous instances have no source name. Derive a stable ID
                // from their normalized header instead of their line number, so
                // inserting lines above them does not rename the node.
                let ordinal = anonymous_instance_counts.entry(sig.clone()).or_insert(0);
                *ordinal += 1;
                let hash = stable_hash(&sig);
                let synthetic = if *ordinal == 1 {
                    format!("instance_{hash:08x}")
                } else {
                    format!("instance_{hash:08x}_{ordinal}")
                };
                declared_name = synthetic.clone();
                short_name = synthetic;
            }
            // Close previous declaration's end_line
            if let Some(prev) = decls.last_mut() {
                prev.end_line = line_num.saturating_sub(1).max(prev.start_line);
            }

            let scope_prefix: Vec<&str> = scope_stack
                .iter()
                .filter(|s| s.kind == ScopeKind::Namespace)
                .filter_map(|s| s.name.as_deref())
                .collect();
            let qualified_name = if scope_prefix.is_empty() {
                declared_name.clone()
            } else {
                format!("{}.{}", scope_prefix.join("."), declared_name)
            };
            let node_id = lean_id(&[module_path.as_str(), qualified_name.as_str()]);
            let qualified = if scope_prefix.is_empty() && !declared_name.contains('.') {
                None
            } else {
                Some(qualified_name.clone())
            };

            let mut gnode = Node::new(
                node_id.clone(),
                short_name.as_str(),
                FileType::Code,
                file_path,
            );
            if let Some(q) = qualified {
                gnode = gnode.with_qualified_label(q);
            }
            result.nodes.push(gnode);
            label_to_nid.insert(short_name.clone(), node_id.clone());
            label_to_nid.insert(declared_name, node_id.clone());
            label_to_nid.insert(qualified_name, node_id.clone());

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
                label: short_name,
                kind: kw.to_string(),
                start_line: line_num,
                end_line: total_lines,
                body_tokens,
                header_text: after_name.to_string(),
                has_sorry,
            });
        } else if let Some(current) = decls.last_mut() {
            current.header_text.push(' ');
            current.header_text.push_str(trimmed);
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

    // Pass 3: Emit `depends_on_hypothesis` edges for theorem/lemma/def binder
    // types that refer to project-defined structures, classes, inductives,
    // defs, or opaque declarations. A theorem with such a binder is
    // conditional on that type even when `#print axioms` reports only kernel
    // axioms.
    let type_like: HashMap<&str, (&str, &str)> = decls
        .iter()
        .filter(|d| is_type_like_decl_kind(&d.kind))
        .map(|d| (d.label.as_str(), (d.node_id.as_str(), d.kind.as_str())))
        .collect();

    let mut seen_hypotheses: HashSet<(String, String)> = HashSet::new();
    for d in &decls {
        if !is_parameterized_decl_kind(&d.kind) {
            continue;
        }
        for token in extract_binder_type_tokens(&d.header_text) {
            let short = token.rsplit('.').next().unwrap_or(&token);
            let (target_id, resolved_kind) = match type_like.get(short) {
                Some((id, kind)) => ((*id).to_string(), Some(*kind)),
                None => {
                    if !looks_like_project_type(short) {
                        continue;
                    }
                    (make_id(&[short]), None)
                }
            };
            let key = (d.node_id.clone(), target_id.clone());
            if target_id == d.node_id || !seen_hypotheses.insert(key) {
                continue;
            }

            // Structures/classes/inductives/opaque declarations are the most
            // dangerous hidden assumptions: an unconstructed certificate can be
            // passed in without any axiom showing up. `def`/`abbrev` binder
            // types are still conditional, but are reported as inferred.
            let hidden_structure = resolved_kind.map(is_hidden_structure_kind).unwrap_or(true);
            let confidence = if resolved_kind.is_some() && !hidden_structure {
                Confidence::Inferred
            } else {
                Confidence::Ambiguous
            };
            let mut edge = Edge::new(
                d.node_id.as_str(),
                target_id.as_str(),
                "depends_on_hypothesis",
                confidence,
                file_path,
            );
            edge.extractor = Some("lean-extractor".to_string());
            edge.resolution_status = Some(if resolved_kind.is_some() {
                "resolved".to_string()
            } else {
                "unresolved".to_string()
            });
            result.edges.push(edge);
        }
    }

    result
}

fn is_type_like_decl_kind(kind: &str) -> bool {
    matches!(
        kind,
        "structure" | "class" | "inductive" | "def" | "abbrev" | "opaque"
    )
}

fn is_parameterized_decl_kind(kind: &str) -> bool {
    matches!(
        kind,
        "theorem" | "lemma" | "def" | "abbrev" | "opaque" | "instance"
    )
}

fn is_hidden_structure_kind(kind: &str) -> bool {
    matches!(kind, "structure" | "class" | "inductive" | "opaque")
}

/// Heuristic: a capitalized identifier that is not a common Lean/stdlib type.
/// These are candidates for project-defined declarations in another file.
fn looks_like_project_type(name: &str) -> bool {
    name.chars().next().is_some_and(char::is_uppercase)
        && !is_builtin_lean_type(name)
        && !is_lean_keyword(name)
}

fn is_builtin_lean_type(name: &str) -> bool {
    matches!(
        name,
        "Prop"
            | "Type"
            | "Sort"
            | "Nat"
            | "Int"
            | "Int8"
            | "Int16"
            | "Int32"
            | "Int64"
            | "UInt8"
            | "UInt16"
            | "UInt32"
            | "UInt64"
            | "Float"
            | "Real"
            | "Bool"
            | "String"
            | "Char"
            | "List"
            | "Array"
            | "Option"
            | "Fin"
            | "Finset"
            | "Set"
            | "Multiset"
            | "Fintype"
            | "Decidable"
            | "DecidableEq"
            | "DecidableRel"
            | "Inhabited"
            | "Nonempty"
            | "Subtype"
            | "Sigma"
            | "PSigma"
            | "Prod"
            | "Sum"
            | "Unit"
            | "PUnit"
            | "Empty"
            | "ULift"
            | "PLift"
            | "Quot"
            | "Quotient"
            | "Classical"
            | "Fact"
            | "AutoParam"
            | "optParam"
            | "outParam"
            | "Synthetic"
            | "Thunk"
            | "IO"
            | "Except"
            | "Result"
            | "NeZero"
            | "Positive"
            | "BEq"
            | "Hashable"
            | "Repr"
            | "ToString"
            | "Coe"
            | "CoeT"
            | "HAdd"
            | "HMul"
            | "HSub"
            | "HDiv"
            | "HPow"
            | "LT"
            | "LE"
            | "Eq"
            | "HEq"
    )
}

/// Return the byte offset where a declaration signature ends and the body
/// begins (`:=`) or a `where` clause starts.
fn lean_signature_end(header: &str) -> usize {
    let mut depth = 0i32;
    for (offset, ch) in header.char_indices() {
        match ch {
            '(' | '[' | '{' | '⦃' | '⟨' => depth += 1,
            ')' | ']' | '}' | '⦄' | '⟩' => depth = (depth - 1).max(0),
            _ => {}
        }
        if depth == 0 && header[offset..].starts_with(":=") {
            return offset;
        }
    }
    find_where_token(header).unwrap_or(header.len())
}

/// Collect identifier tokens from the type portion of every binder group in a
/// Lean declaration signature. Binder names before a top-level `:` are ignored
/// (e.g. `(h : EdgeAtlasCertificate)` yields `EdgeAtlasCertificate`).
fn extract_binder_type_tokens(header: &str) -> Vec<String> {
    let signature = &header[..lean_signature_end(header)];
    let chars: Vec<(usize, char)> = signature.char_indices().collect();
    let mut tokens = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        let (start, open) = chars[i];
        let close = match open {
            '(' => Some(')'),
            '[' => Some(']'),
            '{' => Some('}'),
            '⦃' => Some('⦄'),
            _ => None,
        };
        let Some(close) = close else {
            i += 1;
            continue;
        };

        let mut depth = 0i32;
        let mut j = i;
        while j < chars.len() {
            let (_, ch) = chars[j];
            if matches!(ch, '(' | '[' | '{' | '⦃') {
                depth += 1;
            } else if matches!(ch, ')' | ']' | '}' | '⦄') {
                depth -= 1;
                if depth == 0 && ch == close {
                    break;
                }
            }
            j += 1;
        }

        let end_byte = chars
            .get(j)
            .map(|(offset, _)| *offset)
            .unwrap_or(signature.len());
        let inner_start = start + open.len_utf8();
        if inner_start <= end_byte {
            collect_binder_group_tokens(&signature[inner_start..end_byte], &mut tokens);
        }
        i = j + 1;
    }
    tokens
}

fn collect_binder_group_tokens(inner: &str, out: &mut Vec<String>) {
    // If the group contains a top-level `:`, everything after it is the type.
    let mut depth = 0i32;
    let mut type_start = 0usize;
    for (offset, ch) in inner.char_indices() {
        match ch {
            '(' | '[' | '{' | '⦃' => depth += 1,
            ')' | ']' | '}' | '⦄' => depth = (depth - 1).max(0),
            ':' if depth == 0 && !inner[offset..].starts_with(":=") => {
                type_start = offset + 1;
                break;
            }
            _ => {}
        }
    }
    let type_expr = &inner[type_start..];

    for raw in type_expr.split(|c: char| {
        !c.is_alphanumeric() && c != '_' && c != '.' && c != '\'' && c != '!' && c != '?'
    }) {
        let token = raw.trim_matches(|c: char| c == '\'' || c == '!' || c == '?');
        if token.is_empty() {
            continue;
        }
        let token = token.trim_matches('.');
        if token.is_empty() {
            continue;
        }
        if !out.iter().any(|existing| existing == token) {
            out.push(token.to_string());
        }
    }
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

/// Derive a stable, unique module path from a file path.
///
/// With a project root this is the root-relative path without `.lean`, e.g.
/// `FLT/AutomorphicForm/Basic.lean` -> `FLT.AutomorphicForm.Basic`. Without a
/// root it falls back to the path as supplied (still unique across directories).
fn lean_module_path(file_path: &str, project_root: Option<&Path>) -> String {
    let path = Path::new(file_path);
    let relative = project_root
        .and_then(|root| path.strip_prefix(root).ok())
        .unwrap_or(path);
    let without_ext = relative
        .to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches(".lean")
        .trim_start_matches("./")
        .trim_matches('/')
        .to_string();

    if without_ext.is_empty() {
        Path::new(file_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(file_path)
            .to_string()
    } else {
        without_ext.replace('/', ".")
    }
}

/// Strip leading attributes (`@[simp]`) and declaration/visibility modifiers
/// (`private`, `public`, `noncomputable`, …), returning the declaration or
/// command that follows.
fn strip_lean_prefixes(mut rest: &str) -> &str {
    loop {
        let mut stripped_mod = false;
        for modifier in &[
            "private ",
            "public ",
            "protected ",
            "noncomputable ",
            "partial ",
            "unsafe ",
            "local ",
            "scoped ",
            "nonrec ",
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
    rest
}

/// Find the first standalone `where` token in a Lean declaration header.
/// Used to cut an anonymous instance signature off before its body.
fn find_where_token(s: &str) -> Option<usize> {
    for (i, _) in s.char_indices() {
        if !s[i..].starts_with("where") {
            continue;
        }
        let before_ok = s[..i]
            .chars()
            .next_back()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
        let after = i + "where".len();
        let after_ok = s[after..]
            .chars()
            .next()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
        if before_ok && after_ok {
            return Some(i);
        }
    }
    None
}

/// Normalize an anonymous instance header up to `:=` or `where`.
fn anonymous_instance_signature(after_ws: &str) -> String {
    let mut end = after_ws.find(":=").unwrap_or(after_ws.len());
    if let Some(where_pos) = find_where_token(after_ws) {
        end = end.min(where_pos);
    }
    let header = after_ws[..end]
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    format!("instance {header}")
}

fn parse_lean_decl_header(trimmed: &str) -> Option<(&str, String, String, &str, Option<String>)> {
    let rest = strip_lean_prefixes(trimmed);

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
        "instance",
    ] {
        if let Some(after_kw) = rest.strip_prefix(kw) {
            if after_kw.chars().next().is_some_and(char::is_whitespace) {
                let after_ws = after_kw.trim_start();

                if kw == "instance" {
                    // `instance foo : ...` has a name; `instance : ...`,
                    // `instance (x : T) : ...` and `instance {u} : ...` do not.
                    let name_end = after_ws
                        .find(|c: char| {
                            c.is_whitespace() || matches!(c, '(' | '{' | '[' | ':' | '⦃')
                        })
                        .unwrap_or(after_ws.len());
                    let candidate = after_ws[..name_end].trim();
                    if !candidate.is_empty()
                        && candidate
                            .chars()
                            .next()
                            .is_some_and(|c| c.is_alphabetic() || c == '_')
                    {
                        let short_name = candidate
                            .rsplit('.')
                            .next()
                            .unwrap_or(candidate)
                            .to_string();
                        return Some((
                            kw,
                            candidate.to_string(),
                            short_name,
                            &after_ws[name_end..],
                            None,
                        ));
                    }
                    let sig = anonymous_instance_signature(after_ws);
                    return Some((kw, String::new(), String::new(), after_ws, Some(sig)));
                }

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
                    return Some((
                        kw,
                        name.to_string(),
                        short_name,
                        &after_ws[name_end..],
                        None,
                    ));
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
            | "True"
            | "False"
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

    #[test]
    fn extracts_binder_hypothesis_edges() {
        let src = br#"
structure EdgeAtlasCertificate where
  witness : Nat

theorem no_four_mubs_seed (h : EdgeAtlasCertificate) : 1 = 1 := by
  exact rfl

theorem uses_fact [Fact True] : 1 = 1 := by
  exact rfl
"#;
        let r = extract(src, "Hypotheses.lean");
        let seed = r
            .nodes
            .iter()
            .find(|n| n.label == "no_four_mubs_seed")
            .unwrap();
        let cert = r
            .nodes
            .iter()
            .find(|n| n.label == "EdgeAtlasCertificate")
            .unwrap();
        assert!(
            r.edges.iter().any(|e| e.relation == "depends_on_hypothesis"
                && e.source == seed.id
                && e.target == cert.id),
            "theorem binder must emit a depends_on_hypothesis edge: {:?}",
            r.edges
        );
        assert!(
            !r.edges
                .iter()
                .any(|e| e.relation == "depends_on_hypothesis" && e.target == seed.id),
            "the theorem must not depend on itself"
        );
    }

    #[test]
    fn ignores_builtin_binder_types() {
        let src = br#"
theorem numeric (n : Nat) (xs : List Nat) : Nat := n
"#;
        let r = extract(src, "Builtins.lean");
        assert!(
            !r.edges
                .iter()
                .any(|e| e.relation == "depends_on_hypothesis"),
            "Nat/List binders must not create hypothesis edges: {:?}",
            r.edges
        );
    }

    #[test]
    fn extracts_anonymous_instance_and_attributes_its_sorry() {
        let src = br#"
def before_instance : Nat := 1

instance : CommRing Nat where
  add := Nat.add
  add_assoc := by sorry

theorem after_instance : True := by trivial
"#;
        let r = extract(src, "Instances.lean");
        let inst = r
            .nodes
            .iter()
            .find(|n| n.label.starts_with("instance_"))
            .expect("anonymous instance should become a node");
        assert!(
            r.edges
                .iter()
                .any(|e| e.relation == "unverified_sorry" && e.source == inst.id),
            "the instance itself must carry the sorry edge"
        );
        let before = r
            .nodes
            .iter()
            .find(|n| n.label == "before_instance")
            .unwrap();
        assert!(
            !r.edges
                .iter()
                .any(|e| e.relation == "unverified_sorry" && e.source == before.id),
            "the preceding def must not inherit the instance's sorry"
        );
    }

    #[test]
    fn anonymous_instance_id_is_stable_under_line_insertions() {
        let original = br#"
instance : CommRing Nat where
  add := Nat.add
"#;
        let shifted = br#"
-- a comment
def padding : Nat := 0

instance : CommRing Nat where
  add := Nat.add
"#;
        let a = extract(original, "Instances.lean");
        let b = extract(shifted, "Instances.lean");
        let a_inst = a
            .nodes
            .iter()
            .find(|n| n.label.starts_with("instance_"))
            .expect("anonymous instance should become a node");
        let b_inst = b
            .nodes
            .iter()
            .find(|n| n.label.starts_with("instance_"))
            .expect("anonymous instance should become a node");
        assert_eq!(a_inst.id, b_inst.id);
        assert_eq!(a_inst.label, b_inst.label);
    }

    #[test]
    fn duplicate_anonymous_instance_signatures_get_distinct_ids() {
        let src = br#"
instance : CommRing Nat where
  add := Nat.add

instance : CommRing Nat where
  add := Nat.add
"#;
        let r = extract(src, "Instances.lean");
        let instances: Vec<_> = r
            .nodes
            .iter()
            .filter(|n| n.label.starts_with("instance_"))
            .collect();
        assert_eq!(instances.len(), 2);
        assert_ne!(instances[0].id, instances[1].id);
        assert_ne!(instances[0].label, instances[1].label);
    }

    #[test]
    fn same_file_stem_in_different_directories_gets_unique_ids() {
        let root = Path::new("/project");
        let src = b"theorem foo : True := by trivial";
        let a = extract_with_root(src, "/project/Alpha/Basic.lean", Some(root));
        let b = extract_with_root(src, "/project/Beta/Basic.lean", Some(root));

        let a_file = a.nodes.iter().find(|n| n.label == "Basic").unwrap();
        let b_file = b.nodes.iter().find(|n| n.label == "Basic").unwrap();
        assert_eq!(a_file.id, "alpha_basic");
        assert_eq!(b_file.id, "beta_basic");

        let a_decl = a.nodes.iter().find(|n| n.label == "foo").unwrap();
        let b_decl = b.nodes.iter().find(|n| n.label == "foo").unwrap();
        assert_eq!(a_decl.id, "alpha_basic_foo");
        assert_eq!(b_decl.id, "beta_basic_foo");
        assert_ne!(a_decl.id, b_decl.id);
    }

    #[test]
    fn imports_use_full_module_paths() {
        let root = Path::new("/project");
        let src = b"import Alpha.Basic\npublic import Beta.Basic\n";
        let r = extract_with_root(src, "/project/Gamma/Use.lean", Some(root));
        let targets: HashSet<&str> = r
            .edges
            .iter()
            .filter(|e| e.relation == "imports")
            .map(|e| e.target.as_str())
            .collect();
        assert!(targets.contains("alpha_basic"));
        assert!(
            targets.contains("beta_basic"),
            "public import must produce an import edge"
        );
    }

    #[test]
    fn apostrophe_names_get_distinct_ids() {
        let src = br#"
def actionTensorCAlg : Nat := 1
def actionTensorCAlg' : Nat := 2
"#;
        let r = extract(src, "Names.lean");
        let plain = r
            .nodes
            .iter()
            .find(|n| n.label == "actionTensorCAlg")
            .unwrap();
        let primed = r
            .nodes
            .iter()
            .find(|n| n.label == "actionTensorCAlg'")
            .unwrap();
        assert_ne!(plain.id, primed.id);
    }

    #[test]
    fn public_section_does_not_pop_enclosing_namespace() {
        let src = br#"
namespace A
@[expose] public section
theorem first : True := by trivial
end
namespace B
theorem second : True := by trivial
end B
end A
"#;
        let r = extract(src, "PublicSection.lean");
        let first = r.nodes.iter().find(|n| n.label == "first").unwrap();
        let second = r.nodes.iter().find(|n| n.label == "second").unwrap();
        assert_eq!(first.qualified_label.as_deref(), Some("A.first"));
        assert_eq!(second.qualified_label.as_deref(), Some("A.B.second"));
    }

    #[test]
    fn section_end_does_not_pop_enclosing_namespace() {
        let src = br#"
namespace A
section S
theorem first : True := by trivial
end S
namespace B
theorem second : True := by trivial
end B
end A
"#;
        let r = extract(src, "Scopes.lean");
        let first = r.nodes.iter().find(|n| n.label == "first").unwrap();
        let second = r.nodes.iter().find(|n| n.label == "second").unwrap();
        assert_eq!(first.qualified_label.as_deref(), Some("A.first"));
        assert_eq!(second.qualified_label.as_deref(), Some("A.B.second"));
    }
}
