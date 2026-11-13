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
const DEPENDENCY_RELATIONS: &[&str] = &[
    "calls",
    "depends_on",
    "imports",
    "inherits",
    "implements",
    "depends_on_hypothesis",
    "parameter_type",
];

/// Relations that introduce a hypothesis/parameter assumption.
const HYPOTHESIS_RELATIONS: &[&str] = &["depends_on_hypothesis", "parameter_type"];

/// Options controlling a trust audit.
#[derive(Debug, Clone, Default)]
pub struct TrustOptions {
    /// Restrict the audit to one theorem (name or node ID).
    pub theorem: Option<String>,
    /// Fail when `Lean.ofReduceBool` / `Lean.trustCompiler` is inherited.
    pub forbid_native_decide: bool,
    /// Additional external axioms that are explicitly allowed.
    pub allowed_axioms: Vec<String>,
    /// Explicitly flagged hypotheses (declaration labels, qualified names, or
    /// node IDs).
    pub hypotheses: Vec<String>,
    /// Hypotheses that are explicitly allowed despite being inherited.
    pub allowed_hypotheses: Vec<String>,
    /// Warn when an inherited parameter targets a structure/class/opaque type.
    pub warn_structure_params: bool,
}

/// One inherited axiom.
#[derive(Debug, Clone, Serialize)]
pub struct InheritedAxiom {
    pub name: String,
    pub class: String,
    pub allowed: bool,
}

/// One inherited hypothesis or binder parameter.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Assumption {
    pub name: String,
    /// `hypothesis` for `depends_on_hypothesis`, `parameter_type` for the
    /// alternative relation emitted by some extractors.
    pub kind: String,
    /// `direct` when the audited theorem itself declares the binder;
    /// `transitive` when a downstream dependency introduces it.
    #[serde(rename = "direct_or_transitive")]
    pub scope: String,
    /// Human-readable declaration that introduced the binder.
    pub introduced_by: String,
    pub source_file: String,
    /// Internal hint used by `--warn-structure-params`; never serialized.
    #[serde(skip)]
    pub hidden: bool,
    /// Alternative spellings (node ID, label, qualified label) used to match
    /// `--allow-hypothesis`; never serialized.
    #[serde(skip)]
    pub aliases: Vec<String>,
}

/// Explicit three-way trust classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum TrustStatus {
    /// Kernel-clean and free of inherited hypotheses.
    Unconditional,
    /// Kernel-clean but conditional on one or more hypotheses.
    Conditional,
    /// Inherits a proof gap, forbidden native decision, or disallowed axiom.
    Tainted,
}

impl TrustStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            TrustStatus::Unconditional => "UNCONDITIONAL",
            TrustStatus::Conditional => "CONDITIONAL",
            TrustStatus::Tainted => "TAINTED",
        }
    }
}

/// Trust result for one theorem.
#[derive(Debug, Clone, Serialize)]
pub struct TheoremTrust {
    pub theorem: String,
    pub node_id: String,
    pub status: TrustStatus,
    /// Node IDs in the transitive dependency cone (including the theorem).
    pub dependencies: Vec<String>,
    /// Directly and transitively inherited axioms.
    pub axioms: Vec<InheritedAxiom>,
    #[serde(default)]
    pub kernel_axioms: Vec<String>,
    #[serde(default)]
    pub external_axioms: Vec<String>,
    #[serde(default)]
    pub assumptions: Vec<Assumption>,
    #[serde(default)]
    pub proof_gaps: Vec<String>,
    #[serde(default)]
    pub native_decide: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
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
            out.push_str(&format!("- Status: `{}`\n", theorem.status.as_str()));
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
            if theorem.assumptions.is_empty() {
                out.push_str("- Assumptions: none\n");
            } else {
                out.push_str("- Assumptions:\n");
                for assumption in &theorem.assumptions {
                    out.push_str(&format!(
                        "  - `{}` ({}, {}) introduced by `{}`\n",
                        assumption.name,
                        assumption.kind,
                        assumption.scope,
                        assumption.introduced_by
                    ));
                }
            }
            if !theorem.warnings.is_empty() {
                out.push_str("- Warnings:\n");
                for warning in &theorem.warnings {
                    out.push_str(&format!("  - {warning}\n"));
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

/// Lightweight aggregate used by `gm check`, `verify_changes`, and
/// `get_graph_info` to surface conditional theorems without changing the MCP
/// tool count.
#[derive(Debug, Clone, Default, Serialize)]
pub struct TrustSummary {
    pub theorems: usize,
    pub unconditional: usize,
    pub conditional: usize,
    pub tainted: usize,
    pub assumptions: Vec<String>,
    pub conditional_theorems: Vec<String>,
}

/// Summarize Lean trust data already present in the graph. Returns `None` when
/// no axiom or hypothesis data has been loaded.
pub fn summarize(graph: &GrapheniumGraph) -> Option<TrustSummary> {
    let report = evaluate_trust(graph, None, &TrustOptions::default()).ok()?;
    if report.theorems.is_empty() {
        return None;
    }
    let mut summary = TrustSummary {
        theorems: report.theorems.len(),
        ..TrustSummary::default()
    };
    for theorem in &report.theorems {
        match theorem.status {
            TrustStatus::Unconditional => summary.unconditional += 1,
            TrustStatus::Conditional => {
                summary.conditional += 1;
                summary.conditional_theorems.push(theorem.theorem.clone());
            }
            TrustStatus::Tainted => summary.tainted += 1,
        }
        for assumption in &theorem.assumptions {
            if !summary.assumptions.contains(&assumption.name) {
                summary.assumptions.push(assumption.name.clone());
            }
        }
    }
    summary.assumptions.sort();
    summary.conditional_theorems.sort();
    Some(summary)
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
                ) || HYPOTHESIS_RELATIONS.contains(&edge.relation.as_str())
                {
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
    let allowed_hypotheses: HashSet<String> = options
        .allowed_hypotheses
        .iter()
        .map(|name| normalize_axiom_name(name))
        .collect();
    let flagged_hypotheses: HashSet<String> = options
        .hypotheses
        .iter()
        .map(|name| normalize_axiom_name(name))
        .collect();

    let mut theorems = Vec::new();
    for node_id in targets {
        let theorem_name = node_display_name(graph, &node_id);

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
        let mut kernel_axioms = Vec::new();
        let mut external_axioms = Vec::new();
        let mut proof_gaps = Vec::new();
        let mut native_decide = Vec::new();
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
            match class {
                AxiomClass::StandardKernel => kernel_axioms.push(name.clone()),
                AxiomClass::IncompleteProofGap => proof_gaps.push(name.clone()),
                AxiomClass::NativeEvalShortcut => native_decide.push(name.clone()),
                AxiomClass::ExternalAxiom => external_axioms.push(name.clone()),
            }
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
        for list in [
            &mut kernel_axioms,
            &mut external_axioms,
            &mut proof_gaps,
            &mut native_decide,
        ] {
            list.sort();
            list.dedup_by(|a, b| normalize_axiom_name(a) == normalize_axiom_name(b));
        }

        let mut assumptions = collect_assumptions(graph, &node_id, &dependencies);
        add_flagged_hypotheses(
            graph,
            &node_id,
            &theorem_name,
            &dependencies,
            &flagged_hypotheses,
            &mut assumptions,
        );
        // Explicit `--hypothesis` overrides also work when the extractor missed
        // a binder edge. Only apply this fallback to a single audited theorem;
        // when auditing every theorem, auto-detected edges drive the result.
        if options.theorem.is_some() {
            add_global_flagged_hypothesis(
                graph,
                &node_id,
                &theorem_name,
                &options.hypotheses,
                &mut assumptions,
            );
        }
        assumptions.sort_by(|a, b| a.name.cmp(&b.name));

        let all_hypotheses_allowed = assumptions
            .iter()
            .all(|assumption| hypothesis_allowed(assumption, &allowed_hypotheses));
        let has_disallowed_external = external_axioms
            .iter()
            .any(|name| !allowed.contains(&normalize_axiom_name(name)));
        let tainted = has_sorry
            || (options.forbid_native_decide && has_native_decide)
            || has_disallowed_external;
        let conditional = !assumptions.is_empty();
        let status = if tainted {
            TrustStatus::Tainted
        } else if conditional {
            TrustStatus::Conditional
        } else {
            TrustStatus::Unconditional
        };
        let passed = !tainted && (assumptions.is_empty() || all_hypotheses_allowed);

        let warnings = if options.warn_structure_params {
            assumptions
                .iter()
                .filter(|assumption| {
                    assumption.hidden && !hypothesis_allowed(assumption, &allowed_hypotheses)
                })
                .map(|assumption| {
                    format!(
                        "`{}` is an unconstructed structure/opaque parameter introduced by `{}`; \
                         pass --allow-hypothesis {} to accept it explicitly",
                        assumption.name, assumption.introduced_by, assumption.name
                    )
                })
                .collect()
        } else {
            Vec::new()
        };

        theorems.push(TheoremTrust {
            theorem: theorem_name,
            node_id,
            status,
            dependencies,
            axioms,
            kernel_axioms,
            external_axioms,
            assumptions,
            proof_gaps,
            native_decide,
            warnings,
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

/// Human-readable name for a node, preferring its qualified label.
fn node_display_name(graph: &GrapheniumGraph, node_id: &str) -> String {
    graph
        .node_data(node_id)
        .map(|node| node.display_label().to_string())
        .unwrap_or_else(|| node_id.to_string())
}

fn hypothesis_allowed(assumption: &Assumption, allowed: &HashSet<String>) -> bool {
    let mut candidates = assumption.aliases.iter();
    candidates.any(|alias| allowed.contains(&normalize_axiom_name(alias)))
        || allowed.contains(&normalize_axiom_name(&assumption.name))
}

/// Collect hypothesis edges in the dependency cone of `theorem_id`.
fn collect_assumptions(
    graph: &GrapheniumGraph,
    theorem_id: &str,
    dependencies: &[String],
) -> Vec<Assumption> {
    let mut assumptions = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for dependency in dependencies {
        for edge in graph.edges_iter() {
            if edge.source != *dependency || !HYPOTHESIS_RELATIONS.contains(&edge.relation.as_str())
            {
                continue;
            }
            if edge.target == *dependency {
                continue;
            }
            let target = graph.node_data(&edge.target);
            let name = target.map(|node| node.label.clone()).unwrap_or_else(|| {
                if edge.tgt_original.is_empty() {
                    edge.target.clone()
                } else {
                    edge.tgt_original.clone()
                }
            });
            if !seen.insert(edge.target.clone()) {
                continue;
            }
            let scope = if dependency == theorem_id {
                "direct"
            } else {
                "transitive"
            };
            let introduced_by = node_display_name(graph, dependency);
            let source_file = target
                .map(|node| node.source_file.clone())
                .unwrap_or_else(|| edge.source_file.clone());
            let mut aliases = vec![name.clone(), edge.target.clone()];
            if let Some(node) = target {
                aliases.push(node.id.clone());
                aliases.push(node.label.clone());
                if let Some(qualified) = &node.qualified_label {
                    aliases.push(qualified.clone());
                }
            }
            assumptions.push(Assumption {
                name,
                kind: if edge.relation == "parameter_type" {
                    "parameter_type".to_string()
                } else {
                    "hypothesis".to_string()
                },
                scope: scope.to_string(),
                introduced_by,
                source_file,
                hidden: edge.confidence == crate::model::Confidence::Ambiguous,
                aliases,
            });
        }
    }

    assumptions
}

/// Add dependency-cone nodes explicitly named with `--hypothesis`.
fn add_flagged_hypotheses(
    graph: &GrapheniumGraph,
    theorem_id: &str,
    theorem_name: &str,
    dependencies: &[String],
    flagged: &HashSet<String>,
    assumptions: &mut Vec<Assumption>,
) {
    if flagged.is_empty() {
        return;
    }
    for (index, dependency) in dependencies.iter().enumerate() {
        let Some(node) = graph.node_data(dependency) else {
            continue;
        };
        let mut names = vec![
            normalize_axiom_name(&node.id),
            normalize_axiom_name(&node.label),
        ];
        if let Some(qualified) = &node.qualified_label {
            names.push(normalize_axiom_name(qualified));
        }
        if !names.iter().any(|name| flagged.contains(name)) {
            continue;
        }
        let name = node.label.clone();
        if assumptions
            .iter()
            .any(|assumption| normalize_axiom_name(&assumption.name) == normalize_axiom_name(&name))
        {
            continue;
        }
        let direct = index == 0 && dependency == theorem_id;
        let mut aliases = vec![name.clone(), node.id.clone(), node.label.clone()];
        if let Some(qualified) = &node.qualified_label {
            aliases.push(qualified.clone());
        }
        assumptions.push(Assumption {
            name,
            kind: "hypothesis".to_string(),
            scope: if direct { "direct" } else { "transitive" }.to_string(),
            introduced_by: theorem_name.to_string(),
            source_file: node.source_file.clone(),
            hidden: false,
            aliases,
        });
    }
}

/// Fallback for explicit `--hypothesis` names that could not be resolved to a
/// dependency edge.
fn add_global_flagged_hypothesis(
    graph: &GrapheniumGraph,
    theorem_id: &str,
    theorem_name: &str,
    flagged: &[String],
    assumptions: &mut Vec<Assumption>,
) {
    for raw in flagged {
        let node_id = find_declaration_node(graph, raw)
            .or_else(|| graph.contains_node(raw).then(|| raw.clone()));
        let Some(node_id) = node_id else {
            continue;
        };
        if node_id == theorem_id {
            continue;
        }
        let Some(node) = graph.node_data(&node_id) else {
            continue;
        };
        let name = node.label.clone();
        if assumptions
            .iter()
            .any(|assumption| normalize_axiom_name(&assumption.name) == normalize_axiom_name(&name))
        {
            continue;
        }
        let mut aliases = vec![
            name.clone(),
            node.id.clone(),
            node.label.clone(),
            raw.clone(),
        ];
        if let Some(qualified) = &node.qualified_label {
            aliases.push(qualified.clone());
        }
        assumptions.push(Assumption {
            name,
            kind: "hypothesis".to_string(),
            scope: "transitive".to_string(),
            introduced_by: theorem_name.to_string(),
            source_file: node.source_file.clone(),
            hidden: false,
            aliases,
        });
    }
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
