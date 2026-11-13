//! Lean 4 trust-boundary auditing (`gm trust`).
//!
//! Given a graph that already contains dependency edges (and optionally a
//! `#print axioms` report), this module resolves the transitive dependency cone
//! of a theorem and reports every inherited axiom, separating the trusted Lean
//! kernel from proof gaps (`sorryAx`), native-decision shortcuts, and external
//! user axioms.

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

use serde::Serialize;

use crate::extract::lean_axioms::{
    classify_axiom, find_declaration_node, AxiomClass, LeanAxiomReport,
};
use crate::model::{make_id, GrapheniumGraph};

/// Relations that propagate theorem dependencies.
const DEPENDENCY_RELATIONS: &[&str] = &["calls", "depends_on", "imports", "inherits", "implements"];

/// Options controlling a trust audit.
#[derive(Debug, Clone, Default)]
pub struct TrustOptions {
    /// Restrict the audit to one theorem (name or node ID).
    pub theorem: Option<String>,
    /// Fail when `Lean.ofReduceBool` / `Lean.trustCompiler` is inherited.
    pub forbid_native_decide: bool,
    /// Additional external axioms that are explicitly allowed.
    pub allowed_axioms: Vec<String>,
}

/// One inherited axiom.
#[derive(Debug, Clone, Serialize)]
pub struct InheritedAxiom {
    pub name: String,
    pub class: String,
    pub allowed: bool,
}

/// Trust result for one theorem.
#[derive(Debug, Clone, Serialize)]
pub struct TheoremTrust {
    pub theorem: String,
    pub node_id: String,
    /// Node IDs in the transitive dependency cone (including the theorem).
    pub dependencies: Vec<String>,
    /// Directly and transitively inherited axioms.
    pub axioms: Vec<InheritedAxiom>,
    pub has_sorry: bool,
    pub has_native_decide: bool,
    /// External axioms that are not allowlisted.
    pub disallowed_axioms: Vec<String>,
    pub passed: bool,
}

/// Whole-invocation trust report.
#[derive(Debug, Clone, Serialize)]
pub struct TrustReport {
    pub schema_version: &'static str,
    pub axioms_source_hash: Option<String>,
    pub theorems: Vec<TheoremTrust>,
    pub passed: bool,
}

impl TrustReport {
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }

    pub fn to_text(&self) -> String {
        let mut out = String::from("# Lean 4 Trust Report\n\n");
        if let Some(hash) = &self.axioms_source_hash {
            out.push_str(&format!("**Axiom report hash:** `{hash}`\n\n"));
        }
        for theorem in &self.theorems {
            out.push_str(&format!("## `{}`\n\n", theorem.theorem));
            out.push_str(&format!("- Node: `{}`\n", theorem.node_id));
            out.push_str(&format!(
                "- Dependencies: {} node(s)\n",
                theorem.dependencies.len()
            ));
            out.push_str(&format!("- `sorryAx`: {}\n", theorem.has_sorry));
            out.push_str(&format!(
                "- Native decision (`Lean.ofReduceBool` / `trustCompiler`): {}\n",
                theorem.has_native_decide
            ));
            if theorem.axioms.is_empty() {
                out.push_str("- Axioms: none reported\n");
            } else {
                out.push_str("- Axioms:\n");
                for axiom in &theorem.axioms {
                    out.push_str(&format!(
                        "  - `{}` ({}){}\n",
                        axiom.name,
                        axiom.class,
                        if axiom.allowed { "" } else { " **DISALLOWED**" }
                    ));
                }
            }
            if !theorem.disallowed_axioms.is_empty() {
                out.push_str(&format!(
                    "- Disallowed: {}\n",
                    theorem.disallowed_axioms.join(", ")
                ));
            }
            out.push_str(&format!(
                "- Result: {}\n\n",
                if theorem.passed { "PASS" } else { "FAIL" }
            ));
        }
        out.push_str(if self.passed {
            "**Overall: PASS**\n"
        } else {
            "**Overall: FAIL**\n"
        });
        out
    }
}

/// Evaluate the trust cone for one or all reported theorems.
pub fn evaluate_trust(
    graph: &GrapheniumGraph,
    axiom_report: Option<&LeanAxiomReport>,
    options: &TrustOptions,
) -> Result<TrustReport, String> {
    // Axioms keyed by declaration node ID. Graph edges are still consulted so
    // reports baked in by `gm run --lean-axioms` are auditable without the
    // original report file.
    let mut declared: HashMap<String, BTreeSet<String>> = HashMap::new();
    if let Some(report) = axiom_report {
        for declaration in &report.declarations {
            let node_id = find_declaration_node(graph, &declaration.name)
                .unwrap_or_else(|| make_id(&[declaration.name.as_str()]));
            declared
                .entry(node_id)
                .or_default()
                .extend(declaration.axioms.iter().cloned());
        }
    }

    let targets = match &options.theorem {
        Some(theorem) => {
            let node_id = find_declaration_node(graph, theorem)
                .or_else(|| graph.contains_node(theorem).then(|| theorem.clone()))
                .ok_or_else(|| format!("theorem '{theorem}' not found in the graph"))?;
            vec![node_id]
        }
        None => {
            let mut ids: Vec<String> = declared.keys().cloned().collect();
            for edge in graph.edges_iter() {
                if matches!(
                    edge.relation.as_str(),
                    "depends_on_axiom" | "unverified_sorry" | "unverified_native_decide"
                ) {
                    ids.push(edge.source.clone());
                }
            }
            ids.sort();
            ids.dedup();
            ids
        }
    };

    if targets.is_empty() {
        return Err(
            "no Lean axiom or proof-gap information found in this graph; pass --axioms <path> \
             or run `gm run --lean-axioms <path>`"
                .to_string(),
        );
    }

    let allowed: HashSet<String> = options
        .allowed_axioms
        .iter()
        .map(|name| normalize_axiom_name(name))
        .collect();

    let mut theorems = Vec::new();
    for node_id in targets {
        let theorem_name = graph
            .node_data(&node_id)
            .map(|node| {
                node.qualified_label
                    .clone()
                    .unwrap_or_else(|| node.label.clone())
            })
            .unwrap_or_else(|| node_id.clone());

        let dependencies = dependency_cone(graph, &node_id);
        let mut axiom_names: BTreeSet<String> = BTreeSet::new();

        for dependency in &dependencies {
            if let Some(axioms) = declared.get(dependency) {
                axiom_names.extend(axioms.iter().cloned());
            }
            for edge in graph.edges_iter() {
                if edge.source != *dependency {
                    continue;
                }
                match edge.relation.as_str() {
                    "depends_on_axiom" => {
                        axiom_names.insert(edge.tgt_original.clone());
                        axiom_names.insert(edge.target.clone());
                    }
                    "unverified_sorry" => {
                        axiom_names.insert("sorryAx".to_string());
                    }
                    "unverified_native_decide" => {
                        axiom_names.insert("Lean.ofReduceBool".to_string());
                    }
                    _ => {}
                }
            }
        }

        let mut axioms = Vec::new();
        let mut disallowed = Vec::new();
        let mut has_sorry = false;
        let mut has_native_decide = false;

        for name in axiom_names {
            // A node ID and its original spelling can both land here; keep the
            // more human-readable spelling when one classifies as standard.
            let class = classify_axiom(&name);
            let is_allowed = match class {
                AxiomClass::StandardKernel => true,
                AxiomClass::IncompleteProofGap => {
                    has_sorry = true;
                    false
                }
                AxiomClass::NativeEvalShortcut => {
                    has_native_decide = true;
                    !options.forbid_native_decide
                }
                AxiomClass::ExternalAxiom => allowed.contains(&normalize_axiom_name(&name)),
            };
            if !is_allowed {
                disallowed.push(name.clone());
            }
            axioms.push(InheritedAxiom {
                name,
                class: class.as_str().to_string(),
                allowed: is_allowed,
            });
        }

        // De-duplicate human-readable/normalized duplicates.
        axioms.sort_by(|a, b| a.name.cmp(&b.name));
        axioms.dedup_by(|a, b| normalize_axiom_name(&a.name) == normalize_axiom_name(&b.name));
        disallowed.sort();
        disallowed.dedup_by(|a, b| normalize_axiom_name(a) == normalize_axiom_name(b));

        let passed = !has_sorry
            && !(options.forbid_native_decide && has_native_decide)
            && disallowed.iter().all(|name| {
                classify_axiom(name) == AxiomClass::NativeEvalShortcut
                    && !options.forbid_native_decide
            });

        theorems.push(TheoremTrust {
            theorem: theorem_name,
            node_id,
            dependencies,
            axioms,
            has_sorry,
            has_native_decide,
            disallowed_axioms: disallowed,
            passed,
        });
    }

    let passed = theorems.iter().all(|theorem| theorem.passed);
    Ok(TrustReport {
        schema_version: "lean-trust.v1",
        axioms_source_hash: axiom_report.and_then(|report| report.source_hash.clone()),
        theorems,
        passed,
    })
}

fn normalize_axiom_name(name: &str) -> String {
    name.trim()
        .trim_matches(|c| c == '\'' || c == '"' || c == '`')
        .replace('.', "_")
        .to_ascii_lowercase()
}

/// Transitive dependency cone rooted at `start`, following only edges whose
/// declared `source` is the current node.
fn dependency_cone(graph: &GrapheniumGraph, start: &str) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<String> = VecDeque::new();
    let mut order: Vec<String> = Vec::new();

    seen.insert(start.to_string());
    queue.push_back(start.to_string());

    while let Some(current) = queue.pop_front() {
        order.push(current.clone());
        for edge in graph.edges_iter() {
            if edge.source != current || edge.target == current {
                continue;
            }
            if !DEPENDENCY_RELATIONS.contains(&edge.relation.as_str()) {
                continue;
            }
            if seen.insert(edge.target.clone()) {
                queue.push_back(edge.target.clone());
            }
        }
    }

    order
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::lean_axioms::parse_print_axioms_text;
    use crate::model::{Confidence, Edge, FileType, GrapheniumGraph, Node};

    fn graph_with_theorem() -> (GrapheniumGraph, String) {
        let mut graph = GrapheniumGraph::new();
        let theorem_id = make_id(&["MUB6", "no_four_mubs_dim6"]);
        graph.upsert_node(
            Node::new(
                &theorem_id,
                "no_four_mubs_dim6",
                FileType::Code,
                "Proof.lean",
            )
            .with_qualified_label("MUB6.no_four_mubs_dim6"),
        );
        let helper_id = make_id(&["helper"]);
        graph.upsert_node(Node::new(
            &helper_id,
            "helper",
            FileType::Code,
            "Proof.lean",
        ));
        graph.add_edge(Edge::extracted(
            &theorem_id,
            &helper_id,
            "depends_on",
            "Proof.lean",
        ));
        (graph, theorem_id)
    }

    #[test]
    fn dependency_cone_follows_direction() {
        let (graph, theorem_id) = graph_with_theorem();
        let cone = dependency_cone(&graph, &theorem_id);
        assert_eq!(cone.len(), 2);
    }

    #[test]
    fn external_axiom_fails_without_allowlist() {
        let (graph, _) = graph_with_theorem();
        let report = parse_print_axioms_text(
            "'MUB6.no_four_mubs_dim6' depends on axioms: [propext, EdgeAtlasCertificate]",
        );
        let result = evaluate_trust(
            &graph,
            Some(&report),
            &TrustOptions {
                theorem: Some("MUB6.no_four_mubs_dim6".to_string()),
                ..TrustOptions::default()
            },
        )
        .unwrap();
        assert!(!result.passed);
        assert!(result.theorems[0]
            .disallowed_axioms
            .iter()
            .any(|a| a == "EdgeAtlasCertificate"));
    }

    #[test]
    fn native_decide_can_be_forbidden_or_allowed() {
        let (graph, _) = graph_with_theorem();
        let report = parse_print_axioms_text(
            "'MUB6.no_four_mubs_dim6' depends on axioms: [Lean.ofReduceBool]",
        );
        let permissive = evaluate_trust(
            &graph,
            Some(&report),
            &TrustOptions {
                theorem: Some("MUB6.no_four_mubs_dim6".to_string()),
                ..TrustOptions::default()
            },
        )
        .unwrap();
        assert!(permissive.passed);

        let strict = evaluate_trust(
            &graph,
            Some(&report),
            &TrustOptions {
                theorem: Some("MUB6.no_four_mubs_dim6".to_string()),
                forbid_native_decide: true,
                ..TrustOptions::default()
            },
        )
        .unwrap();
        assert!(!strict.passed);
    }

    #[test]
    fn sorry_always_fails() {
        let (graph, _) = graph_with_theorem();
        let report =
            parse_print_axioms_text("'MUB6.no_four_mubs_dim6' depends on axioms: [sorryAx]");
        let result = evaluate_trust(
            &graph,
            Some(&report),
            &TrustOptions {
                theorem: Some("MUB6.no_four_mubs_dim6".to_string()),
                allowed_axioms: vec!["sorryAx".to_string()],
                ..TrustOptions::default()
            },
        )
        .unwrap();
        assert!(!result.passed);
        assert!(result.theorems[0].has_sorry);
    }

    #[test]
    fn kernel_only_theorem_passes() {
        let (graph, _) = graph_with_theorem();
        let report = parse_print_axioms_text(
            "'MUB6.no_four_mubs_dim6' depends on axioms: [propext, Quot.sound]",
        );
        let result = evaluate_trust(
            &graph,
            Some(&report),
            &TrustOptions {
                theorem: Some("MUB6.no_four_mubs_dim6".to_string()),
                forbid_native_decide: true,
                ..TrustOptions::default()
            },
        )
        .unwrap();
        assert!(result.passed);
    }

    #[test]
    fn baked_graph_edges_are_auditable() {
        let (mut graph, theorem_id) = graph_with_theorem();
        let axiom_id = make_id(&["EdgeAtlasCertificate"]);
        graph.upsert_node(Node::new(
            &axiom_id,
            "EdgeAtlasCertificate",
            FileType::Code,
            "Proof.lean",
        ));
        let mut edge = Edge::new(
            &theorem_id,
            &axiom_id,
            "depends_on_axiom",
            Confidence::Ambiguous,
            "Proof.lean",
        );
        edge.extractor = Some("lean-axioms".to_string());
        graph.add_edge(edge);

        let result = evaluate_trust(
            &graph,
            None,
            &TrustOptions {
                theorem: Some("MUB6.no_four_mubs_dim6".to_string()),
                ..TrustOptions::default()
            },
        )
        .unwrap();
        assert!(!result.passed);
    }
}
