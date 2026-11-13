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

        for axiom in &declaration.axioms {
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
                    let axiom_id = make_id(&[axiom.as_str()]);
                    if emitted_nodes.insert(axiom_id.clone()) {
                        let mut node =
                            Node::new(axiom_id.clone(), axiom.as_str(), FileType::Code, decl_file);
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
                        axiom.as_str(),
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

        for axiom in &declaration.axioms {
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
                    let axiom_id = make_id(&[axiom.as_str()]);
                    if !graph.contains_node(&axiom_id) && emitted.insert(axiom_id.clone()) {
                        let mut node =
                            Node::new(axiom_id.clone(), axiom.as_str(), FileType::Code, &decl_file);
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
                        axiom.as_str(),
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
}
