//! Semantic Lean 4 trust-boundary ingestion.
//!
//! Graphenium does not embed a Lean compiler. Instead this module ingests the
//! output of Lean's `#print axioms` command (raw text or a `lean-axioms.v1`
//! JSON document) and turns it into graph facts:
//!
//! - standard kernel axioms (`propext`, `Classical.choice`, `Quot.sound`) are
//!   recorded as ordinary `depends_on_axiom` edges,
//! - `sorryAx` becomes an `unverified_sorry` self-edge (proof gap),
//! - `Lean.ofReduceBool` / `Lean.trustCompiler` becomes an
//!   `unverified_native_decide` self-edge,
//! - every other axiom is treated as a user/external axiom and recorded as a
//!   `depends_on_axiom` edge with `Confidence::Ambiguous`.
//!
//! The Datalog standard library (`uses_axiom/2`, `has_untrusted_axiom/1`,
//! `has_native_decide/1`) and the `gm trust` command consume these edges.

use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::model::{make_id, Confidence, Edge, ExtractionResult, FileType, GrapheniumGraph, Node};

/// Schema identifier accepted by the JSON ingestion path.
pub const LEAN_AXIOMS_SCHEMA: &str = "lean-axioms.v1";

/// Classification of a Lean axiom reported by `#print axioms`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AxiomClass {
    /// Trusted Lean kernel axioms.
    StandardKernel,
    /// `sorryAx` — an incomplete proof.
    IncompleteProofGap,
    /// `Lean.ofReduceBool` / `Lean.trustCompiler` — `native_decide` trust.
    NativeEvalShortcut,
    /// A user-declared or imported non-kernel axiom.
    ExternalAxiom,
}

impl AxiomClass {
    pub fn as_str(self) -> &'static str {
        match self {
            AxiomClass::StandardKernel => "standard_kernel",
            AxiomClass::IncompleteProofGap => "incomplete_proof_gap",
            AxiomClass::NativeEvalShortcut => "native_eval_shortcut",
            AxiomClass::ExternalAxiom => "external_axiom",
        }
    }
}

/// One binder recovered from Lean's elaborated `ConstantInfo.type`.
///
/// Lean stores theorem parameters as a chain of `forallE` nodes. Report
/// producers can either flatten those nodes into `binders` directly or include
/// the elaborated expression under the declaration's `type` field; both shapes
/// are understood by [`elaborated_binders`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElaboratedBinder {
    /// Binder name (`C`, `h`, `R`, …). May be empty for anonymous binders.
    #[serde(default)]
    pub name: String,
    /// Binder type, e.g. `SeedAtlas.EdgeX.EdgeAtlasCertificate`.
    #[serde(
        default,
        rename = "type",
        deserialize_with = "deserialize_binder_type_name"
    )]
    pub type_name: String,
    /// Lean binder info: `explicit`, `implicit`, `strictImplicit`, or
    /// `instance`. Missing values are treated as explicit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

/// `#print axioms` information for a single declaration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeclarationAxioms {
    /// Fully qualified Lean declaration name (`MUB6.no_four_mubs_dim6`).
    pub name: String,
    #[serde(default)]
    pub axioms: Vec<String>,
    /// Optional repo-relative source file for the declaration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_file: Option<String>,
    /// Binders flattened from the declaration's elaborated `ConstantInfo.type`
    /// (`forallE` chain). Optional for backwards compatibility.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub binders: Vec<ElaboratedBinder>,
    /// Raw elaborated `ConstantInfo.type` expression, when a producer emits it.
    /// Accepts the common `forallE` object/array encodings (see
    /// [`elaborated_binders`]).
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub constant_type: Option<serde_json::Value>,
}

/// Parsed `#print axioms` report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeanAxiomReport {
    #[serde(default = "default_schema")]
    pub schema_version: String,
    /// Hash of the `.olean` / source artifact the report was generated from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_hash: Option<String>,
    #[serde(default)]
    pub declarations: Vec<DeclarationAxioms>,
}

fn default_schema() -> String {
    LEAN_AXIOMS_SCHEMA.to_string()
}

/// Classify one axiom name.
pub fn classify_axiom(name: &str) -> AxiomClass {
    let normalized = name
        .trim()
        .trim_matches(|c| c == '\'' || c == '"' || c == '`')
        .replace('.', "_")
        .to_ascii_lowercase();
    match normalized.as_str() {
        "propext" | "classical_choice" | "quot_sound" => AxiomClass::StandardKernel,
        "sorryax" | "sorry_ax" => AxiomClass::IncompleteProofGap,
        "lean_ofreducebool" | "lean_trustcompiler" => AxiomClass::NativeEvalShortcut,
        _ => AxiomClass::ExternalAxiom,
    }
}

/// Normalize an axiom name into its canonical lookup key.
///
/// Lean's kernel writes `Classical.choice` / `Quot.sound`, while graph node IDs
/// use `classical_choice` / `quot_sound`. Normalizing to the ID form lets the
/// trust auditor collapse both spellings to one axiom.
pub fn normalize_axiom_name(name: &str) -> String {
    name.trim()
        .trim_matches(|c| c == '\'' || c == '"' || c == '`')
        .replace('.', "_")
        .to_ascii_lowercase()
}

/// Return the canonical Lean spelling for an axiom.
///
/// Kernel axioms are always rendered exactly as Lean reports them, regardless
/// of whether the input arrived as a raw name or as a normalized node ID. This
/// prevents reports from containing both `Quot.sound` and `quot_sound`.
pub fn canonical_axiom_label(name: &str) -> String {
    let trimmed = name
        .trim()
        .trim_matches(|c| c == '\'' || c == '"' || c == '`');
    match normalize_axiom_name(trimmed).as_str() {
        "propext" => "propext".to_string(),
        "classical_choice" => "Classical.choice".to_string(),
        "quot_sound" => "Quot.sound".to_string(),
        "sorryax" | "sorry_ax" => "sorryAx".to_string(),
        "lean_ofreducebool" => "Lean.ofReduceBool".to_string(),
        "lean_trustcompiler" => "Lean.trustCompiler".to_string(),
        _ => trimmed.to_string(),
    }
}

/// Prefer the more informative of two spellings of the same axiom.
fn preferred_axiom_label<'a>(existing: &'a str, candidate: &'a str) -> &'a str {
    fn rank(label: &str) -> u8 {
        let key = normalize_axiom_name(label);
        let mut score = 0;
        if label != key {
            score += 2;
        }
        if label.chars().any(|c| c.is_ascii_uppercase()) {
            score += 1;
        }
        if label.contains('.') {
            score += 1;
        }
        score
    }
    if rank(candidate) > rank(existing) {
        candidate
    } else {
        existing
    }
}

/// Insert an axiom spelling into a canonical-keyed map.
pub fn insert_canonical_axiom(map: &mut std::collections::BTreeMap<String, String>, raw: &str) {
    if raw.trim().is_empty() {
        return;
    }
    let key = normalize_axiom_name(raw);
    let candidate = canonical_axiom_label(raw);
    match map.get(&key) {
        Some(existing) => {
            let chosen = preferred_axiom_label(existing, &candidate).to_string();
            map.insert(key, chosen);
        }
        None => {
            map.insert(key, candidate);
        }
    }
}

fn declaration_axiom_labels(declaration: &DeclarationAxioms) -> Vec<String> {
    let mut axioms = std::collections::BTreeMap::new();
    for axiom in &declaration.axioms {
        insert_canonical_axiom(&mut axioms, axiom);
    }
    axioms.into_values().collect()
}

/// Flatten the binders of a declaration from its optional `binders` list and
/// its elaborated `ConstantInfo.type` (`forallE`) expression.
pub fn elaborated_binders(declaration: &DeclarationAxioms) -> Vec<ElaboratedBinder> {
    let mut binders = declaration.binders.clone();
    if let Some(value) = &declaration.constant_type {
        flatten_forall_expr(value, &mut binders);
    }

    let mut seen = HashSet::new();
    binders.retain(|binder| {
        let key = (
            binder.name.clone(),
            binder.type_name.trim().to_string(),
            binder.kind.clone(),
        );
        !binder.type_name.trim().is_empty() && seen.insert(key)
    });
    binders
}

/// Recursively flatten common JSON encodings of Lean's `Expr.forallE` chain.
fn flatten_forall_expr(value: &serde_json::Value, out: &mut Vec<ElaboratedBinder>) {
    match value {
        serde_json::Value::Array(items) => {
            for item in items {
                flatten_forall_expr(item, out);
            }
        }
        serde_json::Value::String(text) => {
            out.extend(parse_binders_from_text(text));
        }
        serde_json::Value::Object(map) => {
            if let Some(inner) = map.get("forallE") {
                flatten_forall_expr(inner, out);
                return;
            }

            let name = map
                .get("binderName")
                .or_else(|| map.get("name"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let type_value = map
                .get("binderType")
                .or_else(|| map.get("type"))
                .or_else(|| map.get("binder_type"));
            if let Some(type_value) = type_value {
                if let Some(type_name) = expr_type_name(type_value) {
                    let kind = map
                        .get("binderInfo")
                        .or_else(|| map.get("binder_info"))
                        .or_else(|| map.get("kind"))
                        .map(binder_kind_name);
                    out.push(ElaboratedBinder {
                        name: name.to_string(),
                        type_name,
                        kind,
                    });
                }
            }
            if let Some(body) = map.get("body") {
                flatten_forall_expr(body, out);
            }
        }
        _ => {}
    }
}

fn binder_kind_name(value: &serde_json::Value) -> String {
    if let Some(text) = value.as_str() {
        return text.to_ascii_lowercase();
    }
    if let Some(map) = value.as_object() {
        if map.contains_key("instance") || map.contains_key("instImplicit") {
            return "instance".to_string();
        }
        if map.contains_key("strictImplicit") || map.contains_key("strict_implicit") {
            return "strictimplicit".to_string();
        }
        if map.contains_key("implicit") {
            return "implicit".to_string();
        }
    }
    "explicit".to_string()
}

/// Accept either a plain type string or a serialized expression object in the
/// `type` field of a binder.
fn deserialize_binder_type_name<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    expr_type_name(&value)
        .ok_or_else(|| serde::de::Error::custom("binder type must be a Lean type name"))
}

/// Extract a simple type name from a serialized Lean expression.
fn expr_type_name(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) => simple_head_type_name(text),
        serde_json::Value::Object(map) => {
            for key in ["const", "name", "str", "binderType", "type"] {
                if let Some(inner) = map.get(key) {
                    if let Some(name) = expr_type_name(inner) {
                        return Some(name);
                    }
                }
            }
            if let Some(inner) = map.get("app") {
                if let Some(items) = inner.as_array() {
                    return items.first().and_then(expr_type_name);
                }
                if let Some(function) = inner.get("fn").or_else(|| inner.get("function")) {
                    return expr_type_name(function);
                }
            }
            None
        }
        _ => None,
    }
}

/// Parse binder groups from a textual `∀ (x : T), …` / `forall (x : T), …`
/// expression.
fn parse_binders_from_text(text: &str) -> Vec<ElaboratedBinder> {
    let mut binders = Vec::new();
    let mut rest = text.trim();

    while let Some((index, marker_len)) = rest
        .find('∀')
        .map(|idx| (idx, '∀'.len_utf8()))
        .or_else(|| rest.find("forall").map(|idx| (idx, "forall".len())))
    {
        let mut body = &rest[index + marker_len..];
        loop {
            body = body.trim_start();
            let Some((open, close)) = body.chars().next().and_then(|ch| match ch {
                '(' => Some(('(', ')')),
                '{' => Some(('{', '}')),
                '[' => Some(('[', ']')),
                '⦃' => Some(('⦃', '⦄')),
                _ => None,
            }) else {
                break;
            };
            let Some(end) = find_matching_close(body, open, close) else {
                break;
            };
            let inner = &body[open.len_utf8()..end];
            if let Some(type_name) = binder_inner_type_name(inner) {
                binders.push(ElaboratedBinder {
                    name: binder_inner_name(inner),
                    type_name,
                    kind: Some(match open {
                        '[' => "instance".to_string(),
                        '{' | '⦃' => "implicit".to_string(),
                        _ => "explicit".to_string(),
                    }),
                });
            }
            body = &body[end + close.len_utf8()..];
        }

        rest = body.trim_start_matches(|c: char| c == ',' || c.is_whitespace());
        if !rest.contains('∀') && !rest.contains("forall") {
            break;
        }
    }
    binders
}

fn find_matching_close(text: &str, open: char, close: char) -> Option<usize> {
    let mut depth = 0i32;
    for (offset, ch) in text.char_indices() {
        if ch == open {
            depth += 1;
        } else if ch == close {
            depth -= 1;
            if depth == 0 {
                return Some(offset);
            }
        }
    }
    None
}

fn binder_inner_name(inner: &str) -> String {
    match inner.split_once(':') {
        Some((name, _)) => name.trim().to_string(),
        None => inner.trim().to_string(),
    }
}

fn binder_inner_type_name(inner: &str) -> Option<String> {
    let type_expr = inner.split_once(':').map(|(_, ty)| ty).unwrap_or(inner);
    simple_head_type_name(type_expr)
}

/// Return the leading identifier of a simple Lean type expression.
///
/// This deliberately rejects applications (`Matrix n n ℂ`), arrows, and
/// `forall` expressions so the trust audit only treats a *simple* binder type
/// as a potential certificate/hypothesis.
pub(crate) fn simple_head_type_name(type_expr: &str) -> Option<String> {
    let text = type_expr.trim();
    if text.is_empty()
        || text.starts_with('∀')
        || text.starts_with("forall")
        || text.contains('→')
        || text.contains("->")
    {
        return None;
    }
    let token = text
        .split(|c: char| c.is_whitespace() || matches!(c, '(' | ')' | '{' | '}' | '[' | ']' | ','))
        .next()?
        .trim_matches('.');
    if token.is_empty()
        || !token
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '\''))
    {
        return None;
    }
    Some(token.to_string())
}

/// Decide whether an elaborated binder is a hypothesis/certificate parameter.
///
/// Square-bracket typeclass constraints (`[CommRing R]`), primitive data
/// parameters (`(n : Nat)`), and known typeclass/application types are rejected.
/// A simple project/certificate type such as
/// `SeedAtlas.EdgeX.EdgeAtlasCertificate` is accepted.
pub fn binder_hypothesis_type(binder: &ElaboratedBinder) -> Option<String> {
    if let Some(kind) = binder.kind.as_deref() {
        let kind = kind.trim().to_ascii_lowercase();
        if kind.contains("inst") || kind == "typeclass" {
            return None;
        }
    }
    let head = simple_head_type_name(&binder.type_name)?;
    let short = head.rsplit('.').next().unwrap_or(&head);
    crate::extract::lean::looks_like_hypothesis_type(short).then_some(head)
}

/// Parse either a `lean-axioms.v1` JSON document or raw `#print axioms` text.
pub fn parse_report(input: &str) -> Result<LeanAxiomReport, String> {
    let trimmed = input.trim_start();
    if trimmed.starts_with('{') {
        let report: LeanAxiomReport = serde_json::from_str(trimmed)
            .map_err(|e| format!("invalid lean-axioms.v1 JSON: {e}"))?;
        if report.schema_version != LEAN_AXIOMS_SCHEMA {
            return Err(format!(
                "unsupported Lean axiom schema '{}' (expected '{LEAN_AXIOMS_SCHEMA}')",
                report.schema_version
            ));
        }
        return Ok(report);
    }
    Ok(parse_print_axioms_text(trimmed))
}

/// Parse the textual output of `#print axioms`.
///
/// Accepted forms (quotes optional, output may be wrapped across lines):
/// - `'Foo.bar' depends on axioms: [propext, Quot.sound]`
/// - `Foo.bar depends on axioms: [sorryAx]`
/// - `'Foo.bar' does not depend on any axioms`
pub fn parse_print_axioms_text(input: &str) -> LeanAxiomReport {
    let mut declarations = Vec::new();

    let mut pending: Option<(String, String)> = None;
    for raw_line in input.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        let candidate = match pending.take() {
            Some((name, mut buffer)) => {
                buffer.push(' ');
                buffer.push_str(line);
                (name, buffer)
            }
            None => match split_declaration_prefix(line) {
                Some((name, rest)) => (name, rest.to_string()),
                None => continue,
            },
        };

        let (name, rest) = candidate;
        let lower = rest.to_ascii_lowercase();

        if lower.contains("does not depend on any axioms") {
            declarations.push(DeclarationAxioms {
                name,
                axioms: Vec::new(),
                source_file: None,
                binders: Vec::new(),
                constant_type: None,
            });
            continue;
        }

        if let Some(start) = rest.find('[') {
            if let Some(end) = rest[start..].find(']') {
                let inside = &rest[start + 1..start + end];
                let axioms = inside
                    .split(',')
                    .map(|axiom| axiom.trim().trim_matches(|c| c == '\'' || c == '"'))
                    .filter(|axiom| !axiom.is_empty())
                    .map(str::to_string)
                    .collect();
                declarations.push(DeclarationAxioms {
                    name,
                    axioms,
                    source_file: None,
                    binders: Vec::new(),
                    constant_type: None,
                });
                continue;
            }
            // Array not closed on this line — keep accumulating.
            pending = Some((name, rest));
            continue;
        }

        if lower.contains("depends on axioms") {
            // The list starts on the next line.
            pending = Some((name, rest));
        }
    }

    if let Some((name, rest)) = pending {
        // Best-effort flush for truncated output.
        let axioms = rest
            .split([',', '[', ']'])
            .map(str::trim)
            .filter(|token| {
                !token.is_empty() && !token.to_ascii_lowercase().contains("depends on axioms")
            })
            .map(str::to_string)
            .collect();
        declarations.push(DeclarationAxioms {
            name,
            axioms,
            source_file: None,
            binders: Vec::new(),
            constant_type: None,
        });
    }

    LeanAxiomReport {
        schema_version: LEAN_AXIOMS_SCHEMA.to_string(),
        source_hash: None,
        declarations,
    }
}

/// Split `'Name' depends on ...` into `(Name, rest)`.
fn split_declaration_prefix(line: &str) -> Option<(String, &str)> {
    let start = line.find(|c: char| c == '\'' || c == '"' || c.is_alphabetic())?;
    let rest = &line[start..];
    if let Some(stripped) = rest.strip_prefix('\'').or_else(|| rest.strip_prefix('"')) {
        let quote = rest.chars().next()?;
        let end = stripped.find(quote)?;
        let name = stripped[..end].trim();
        let after = &stripped[end + 1..];
        return Some((name.to_string(), after.trim_start()));
    }
    // Unquoted: the declaration name is the first whitespace-delimited token.
    let mut parts = rest.splitn(2, char::is_whitespace);
    let name = parts.next()?.trim();
    let tail = parts.next().unwrap_or("").trim_start();
    Some((name.to_string(), tail))
}

/// Read and parse an axiom report from disk. Raw text or JSON is auto-detected.
pub fn load_report(path: &Path) -> Result<LeanAxiomReport, String> {
    let input = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read Lean axiom report {}: {e}", path.display()))?;
    let mut report = parse_report(&input)?;
    if report.source_hash.is_none() {
        use sha2::{Digest, Sha256};
        report.source_hash = Some(hex::encode(Sha256::digest(input.as_bytes())));
    }
    Ok(report)
}

/// Resolve a Lean declaration name to a graph node ID.
///
/// Prefers qualified labels (emitted by the Lean extractor), then the canonical
/// `make_id` form, then an unambiguous short-label match.
pub fn find_declaration_node(graph: &GrapheniumGraph, name: &str) -> Option<String> {
    let short = name.rsplit('.').next().unwrap_or(name);
    let canonical = make_id(&[name]);

    if let Some(node) = graph
        .nodes()
        .find(|node| node.qualified_label.as_deref() == Some(name) || node.id == canonical)
    {
        return Some(node.id.clone());
    }

    let mut matches = graph.nodes().filter(|node| {
        node.label.eq_ignore_ascii_case(short)
            || node
                .qualified_label
                .as_deref()
                .is_some_and(|q| q.eq_ignore_ascii_case(name))
    });
    let first = matches.next()?.id.clone();
    if matches.next().is_some() {
        // Ambiguous short label without a qualified match: do not guess.
        return None;
    }
    Some(first)
}

/// Build a standalone extraction result from a report.
///
/// Useful when axiom data arrives before a graph exists (tests, harness
/// embedding). `source_file` is applied to declarations that do not carry one.
pub fn report_to_extraction(report: &LeanAxiomReport, source_file: &str) -> ExtractionResult {
    let mut result = ExtractionResult::new();
    let mut emitted_nodes: HashSet<String> = HashSet::new();

    for declaration in &report.declarations {
        let decl_id = make_id(&[declaration.name.as_str()]);
        let decl_file = declaration
            .source_file
            .as_deref()
            .filter(|file| !file.is_empty())
            .unwrap_or(source_file);

        if emitted_nodes.insert(decl_id.clone()) {
            let label = declaration
                .name
                .rsplit('.')
                .next()
                .unwrap_or(&declaration.name);
            let mut node = Node::new(decl_id.clone(), label, FileType::Code, decl_file);
            node = node.with_qualified_label(declaration.name.clone());
            node.extractor = Some("lean-axioms".to_string());
            result.nodes.push(node);
        }

        for axiom in &declaration_axiom_labels(declaration) {
            let class = classify_axiom(axiom);
            match class {
                AxiomClass::IncompleteProofGap => {
                    let mut edge = Edge::new(
                        decl_id.clone(),
                        decl_id.clone(),
                        "unverified_sorry",
                        Confidence::Ambiguous,
                        decl_file,
                    );
                    edge.extractor = Some("lean-axioms".to_string());
                    edge.resolution_status = Some("ambiguous".to_string());
                    result.edges.push(edge);
                }
                AxiomClass::NativeEvalShortcut => {
                    let mut edge = Edge::new(
                        decl_id.clone(),
                        decl_id.clone(),
                        "unverified_native_decide",
                        Confidence::Ambiguous,
                        decl_file,
                    );
                    edge.extractor = Some("lean-axioms".to_string());
                    edge.resolution_status = Some("ambiguous".to_string());
                    result.edges.push(edge);
                }
                AxiomClass::StandardKernel | AxiomClass::ExternalAxiom => {
                    let axiom_label = canonical_axiom_label(axiom);
                    let axiom_id = make_id(&[axiom_label.as_str()]);
                    if emitted_nodes.insert(axiom_id.clone()) {
                        let mut node = Node::new(
                            axiom_id.clone(),
                            axiom_label.as_str(),
                            FileType::Code,
                            decl_file,
                        );
                        node.extractor = Some("lean-axioms".to_string());
                        result.nodes.push(node);
                    }
                    let confidence = if class == AxiomClass::StandardKernel {
                        Confidence::Inferred
                    } else {
                        Confidence::Ambiguous
                    };
                    // Pass the raw spelling so `tgt_original` preserves
                    // `Classical.choice` / `EdgeAtlasCertificate` for audits.
                    let mut edge = Edge::new(
                        decl_id.clone(),
                        axiom_label.as_str(),
                        "depends_on_axiom",
                        confidence,
                        decl_file,
                    );
                    edge.extractor = Some("lean-axioms".to_string());
                    edge.resolution_status = Some(if class == AxiomClass::StandardKernel {
                        "resolved".to_string()
                    } else {
                        "ambiguous".to_string()
                    });
                    result.edges.push(edge);
                }
            }
        }

        // Elaborated `ConstantInfo.type` binders (forallE) capture implicit and
        // section hypotheses that a line-oriented source scan cannot see.
        for binder in elaborated_binders(declaration) {
            let Some(type_name) = binder_hypothesis_type(&binder) else {
                continue;
            };
            let target_id = make_id(&[type_name.as_str()]);
            if target_id == decl_id {
                continue;
            }
            if emitted_nodes.insert(target_id.clone()) {
                let label = type_name.rsplit('.').next().unwrap_or(&type_name);
                let mut node = Node::new(target_id.clone(), label, FileType::Code, decl_file)
                    .with_qualified_label(type_name.clone());
                node.extractor = Some("lean-axioms".to_string());
                result.nodes.push(node);
            }
            let mut edge = Edge::new(
                decl_id.clone(),
                type_name.as_str(),
                "depends_on_hypothesis",
                Confidence::Ambiguous,
                decl_file,
            );
            edge.extractor = Some("lean-axioms".to_string());
            edge.resolution_status = Some("unresolved".to_string());
            result.edges.push(edge);
        }
    }

    result
}

/// Merge an axiom report directly into an existing graph, wiring declarations to
/// their extracted nodes when possible and creating placeholder nodes otherwise.
pub fn merge_report_into_graph(
    graph: &mut GrapheniumGraph,
    report: &LeanAxiomReport,
    fallback_source: &str,
) {
    let mut emitted: HashSet<String> = HashSet::new();

    for declaration in &report.declarations {
        let decl_id = find_declaration_node(graph, &declaration.name)
            .unwrap_or_else(|| make_id(&[declaration.name.as_str()]));
        let decl_file = declaration
            .source_file
            .as_deref()
            .filter(|file| !file.is_empty())
            .unwrap_or(fallback_source)
            .to_string();

        if !graph.contains_node(&decl_id) && emitted.insert(decl_id.clone()) {
            let label = declaration
                .name
                .rsplit('.')
                .next()
                .unwrap_or(&declaration.name);
            let mut node = Node::new(decl_id.clone(), label, FileType::Code, &decl_file)
                .with_qualified_label(declaration.name.clone());
            node.extractor = Some("lean-axioms".to_string());
            graph.upsert_node(node);
        }

        for axiom in &declaration_axiom_labels(declaration) {
            let class = classify_axiom(axiom);
            match class {
                AxiomClass::IncompleteProofGap => {
                    let mut edge = Edge::new(
                        decl_id.clone(),
                        decl_id.clone(),
                        "unverified_sorry",
                        Confidence::Ambiguous,
                        &decl_file,
                    );
                    edge.extractor = Some("lean-axioms".to_string());
                    edge.resolution_status = Some("ambiguous".to_string());
                    graph.add_edge(edge);
                }
                AxiomClass::NativeEvalShortcut => {
                    let mut edge = Edge::new(
                        decl_id.clone(),
                        decl_id.clone(),
                        "unverified_native_decide",
                        Confidence::Ambiguous,
                        &decl_file,
                    );
                    edge.extractor = Some("lean-axioms".to_string());
                    edge.resolution_status = Some("ambiguous".to_string());
                    graph.add_edge(edge);
                }
                AxiomClass::StandardKernel | AxiomClass::ExternalAxiom => {
                    let axiom_label = canonical_axiom_label(axiom);
                    let axiom_id = make_id(&[axiom_label.as_str()]);
                    if !graph.contains_node(&axiom_id) && emitted.insert(axiom_id.clone()) {
                        let mut node = Node::new(
                            axiom_id.clone(),
                            axiom_label.as_str(),
                            FileType::Code,
                            &decl_file,
                        );
                        node.extractor = Some("lean-axioms".to_string());
                        graph.upsert_node(node);
                    }
                    let confidence = if class == AxiomClass::StandardKernel {
                        Confidence::Inferred
                    } else {
                        Confidence::Ambiguous
                    };
                    let mut edge = Edge::new(
                        decl_id.clone(),
                        axiom_label.as_str(),
                        "depends_on_axiom",
                        confidence,
                        &decl_file,
                    );
                    edge.extractor = Some("lean-axioms".to_string());
                    edge.resolution_status = Some(if class == AxiomClass::StandardKernel {
                        "resolved".to_string()
                    } else {
                        "ambiguous".to_string()
                    });
                    graph.add_edge(edge);
                }
            }
        }

        // Wire elaborated binder hypotheses to existing declaration nodes when
        // possible, otherwise create a placeholder so the edge is not dangling.
        for binder in elaborated_binders(declaration) {
            let Some(type_name) = binder_hypothesis_type(&binder) else {
                continue;
            };
            let target_id = find_declaration_node(graph, &type_name)
                .unwrap_or_else(|| make_id(&[type_name.as_str()]));
            if target_id == decl_id {
                continue;
            }
            if !graph.contains_node(&target_id) && emitted.insert(target_id.clone()) {
                let label = type_name.rsplit('.').next().unwrap_or(&type_name);
                let mut node = Node::new(target_id.clone(), label, FileType::Code, &decl_file)
                    .with_qualified_label(type_name.clone());
                node.extractor = Some("lean-axioms".to_string());
                graph.upsert_node(node);
            }
            let mut edge = Edge::new(
                decl_id.clone(),
                type_name.as_str(),
                "depends_on_hypothesis",
                Confidence::Ambiguous,
                &decl_file,
            );
            // Preserve the resolved target when the graph's node ID is
            // module-qualified rather than the plain normalized label.
            edge.target = target_id;
            edge.tgt_original = type_name.clone();
            edge.extractor = Some("lean-axioms".to_string());
            edge.resolution_status = Some("resolved".to_string());
            graph.add_edge(edge);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RAW: &str = r#"
'MUB6.no_four_mubs_dim6' depends on axioms: [propext, Quot.sound, sorryAx,
 Lean.ofReduceBool, EdgeAtlasCertificate]
'clean_theorem' depends on axioms: [propext, Classical.choice, Quot.sound]
'trivial' does not depend on any axioms
"#;

    #[test]
    fn parses_print_axioms_text() {
        let report = parse_print_axioms_text(RAW);
        assert_eq!(report.declarations.len(), 3);
        assert_eq!(report.declarations[0].name, "MUB6.no_four_mubs_dim6");
        assert_eq!(report.declarations[0].axioms.len(), 5);
        assert!(report.declarations[0]
            .axioms
            .contains(&"Lean.ofReduceBool".to_string()));
        assert!(report.declarations[1]
            .axioms
            .contains(&"Classical.choice".to_string()));
        assert!(report.declarations[2].axioms.is_empty());
    }

    #[test]
    fn parses_json_schema() {
        let json = r#"{
          "schema_version": "lean-axioms.v1",
          "source_hash": "abc",
          "declarations": [
            { "name": "MUB6.no_four_mubs_dim6", "axioms": ["propext", "Quot.sound"] }
          ]
        }"#;
        let report = parse_report(json).unwrap();
        assert_eq!(report.declarations.len(), 1);
        assert_eq!(report.source_hash.as_deref(), Some("abc"));
    }

    #[test]
    fn classifies_kernel_gap_native_and_external() {
        assert_eq!(classify_axiom("propext"), AxiomClass::StandardKernel);
        assert_eq!(
            classify_axiom("Classical.choice"),
            AxiomClass::StandardKernel
        );
        assert_eq!(classify_axiom("Quot.sound"), AxiomClass::StandardKernel);
        assert_eq!(classify_axiom("sorryAx"), AxiomClass::IncompleteProofGap);
        assert_eq!(
            classify_axiom("Lean.ofReduceBool"),
            AxiomClass::NativeEvalShortcut
        );
        assert_eq!(
            classify_axiom("Lean.trustCompiler"),
            AxiomClass::NativeEvalShortcut
        );
        assert_eq!(
            classify_axiom("EdgeAtlasCertificate"),
            AxiomClass::ExternalAxiom
        );
    }

    #[test]
    fn extraction_emits_expected_edges() {
        let report = parse_print_axioms_text(RAW);
        let extraction = report_to_extraction(&report, "Proof.lean");
        assert!(extraction
            .nodes
            .iter()
            .any(|n| n.id == make_id(&["EdgeAtlasCertificate"])));
        assert!(extraction
            .edges
            .iter()
            .any(|e| e.relation == "unverified_sorry"));
        assert!(extraction
            .edges
            .iter()
            .any(|e| e.relation == "unverified_native_decide"));
        assert!(extraction
            .edges
            .iter()
            .any(|e| e.relation == "depends_on_axiom"));
    }

    #[test]
    fn canonicalizes_kernel_axiom_spellings() {
        let mut map = std::collections::BTreeMap::new();
        insert_canonical_axiom(&mut map, "Quot.sound");
        insert_canonical_axiom(&mut map, "quot_sound");
        insert_canonical_axiom(&mut map, "Classical.choice");
        insert_canonical_axiom(&mut map, "classical_choice");
        let labels: Vec<String> = map.into_values().collect();
        assert_eq!(labels, vec!["Classical.choice", "Quot.sound"]);
    }

    #[test]
    fn flattens_elaborated_forall_binders() {
        let json = r#"{
          "schema_version": "lean-axioms.v1",
          "declarations": [
            {
              "name": "NoFourMUBs.EdgeXFinal.no_four_mubs_seed",
              "axioms": ["propext"],
              "type": {
                "forallE": {
                  "binderName": "C",
                  "binderType": "SeedAtlas.EdgeX.EdgeAtlasCertificate",
                  "binderInfo": "explicit",
                  "body": {
                    "forallE": {
                      "binderName": "R",
                      "binderType": "CommRing R",
                      "binderInfo": "instance",
                      "body": null
                    }
                  }
                }
              }
            }
          ]
        }"#;
        let report = parse_report(json).unwrap();
        let binders = elaborated_binders(&report.declarations[0]);
        assert_eq!(binders.len(), 2, "{binders:?}");
        assert_eq!(
            binder_hypothesis_type(&binders[0]).as_deref(),
            Some("SeedAtlas.EdgeX.EdgeAtlasCertificate")
        );
        assert!(
            binder_hypothesis_type(&binders[1]).is_none(),
            "typeclass instance binders must be ignored"
        );

        let extraction = report_to_extraction(&report, "Proof.lean");
        assert!(extraction.edges.iter().any(|edge| {
            edge.relation == "depends_on_hypothesis"
                && edge.tgt_original == "SeedAtlas.EdgeX.EdgeAtlasCertificate"
        }));
    }

    #[test]
    fn semantic_binders_ignore_primitive_parameters() {
        let json = r#"{
          "schema_version": "lean-axioms.v1",
          "declarations": [
            {
              "name": "generic",
              "axioms": ["propext"],
              "binders": [
                { "name": "n", "type": "Nat" },
                { "name": "R", "type": "Type" },
                { "name": "inst", "type": "CommRing R", "kind": "instance" }
              ]
            }
          ]
        }"#;
        let report = parse_report(json).unwrap();
        let binders = elaborated_binders(&report.declarations[0]);
        assert_eq!(binders.len(), 3);
        assert!(binders
            .iter()
            .all(|binder| binder_hypothesis_type(binder).is_none()));
    }
}
