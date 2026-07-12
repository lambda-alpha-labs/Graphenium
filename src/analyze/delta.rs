//! Topological delta gating: evaluate planned changes against modularity invariants.
//!
//! Compares a physical-only baseline subgraph against a virtual graph that includes
//! a planning workspace, then gates on modularity decay and surprise edge scores.

use std::collections::HashMap;

use petgraph::visit::EdgeRef;

use crate::analyze::surprise::{surprising_connections, SurprisingEdge};
use crate::cluster::drift::{detect_drift, DriftEvent};
use crate::cluster::louvain::LGraph;
use crate::cluster::{cluster, ClusterOptions};
use crate::error::GrapheniumError;
use crate::model::GrapheniumGraph;

#[derive(Debug, Clone, serde::Serialize)]
pub struct DeltaGateReport {
    pub plan_id: String,
    pub passes: bool,
    pub modularity_baseline: f64,
    pub modularity_virtual: f64,
    pub modularity_delta: f64,
    pub plan_surprise_edges: Vec<SurprisingEdge>,
    pub drift_events: Vec<DriftEvent>,
}

/// Clones the graph and filters it to keep only physical nodes and edges (plan_id is None).
pub fn extract_baseline_subgraph(graph: &GrapheniumGraph) -> GrapheniumGraph {
    let mut baseline = GrapheniumGraph::new();
    baseline.set_ast_only(graph.is_ast_only());
    baseline.metadata = graph.metadata.clone();

    for node in graph.nodes() {
        if node.plan_id.is_none() {
            baseline.upsert_node(node.clone());
        }
    }

    for edge in graph.edges_iter() {
        if edge.plan_id.is_none() {
            baseline.add_edge(edge.clone());
        }
    }

    baseline.rebuild_id_index();
    baseline
}

/// Build an internal Louvain graph and aligned community assignments from a clustered graph.
fn modularity_for_graph(graph: &GrapheniumGraph) -> f64 {
    let n = graph.node_count();
    if n == 0 {
        return 0.0;
    }

    let node_indices: Vec<_> = graph.inner().node_indices().collect();
    let idx_to_seq: HashMap<_, usize> = node_indices
        .iter()
        .enumerate()
        .map(|(i, &ni)| (ni, i))
        .collect();

    let node_ids: Vec<String> = node_indices
        .iter()
        .map(|&ni| graph.inner()[ni].id.clone())
        .collect();

    let mut lg = LGraph::new(n);
    for e in graph.inner().edge_references() {
        let u = idx_to_seq[&e.source()];
        let v = idx_to_seq[&e.target()];
        lg.add_edge(u, v, e.weight().weight);
    }

    let community_seq: Vec<usize> = node_ids
        .iter()
        .map(|id| graph.node_data(id).and_then(|n| n.community).unwrap_or(0))
        .collect();

    lg.modularity(&community_seq)
}

/// Evaluates a planning workspace against Graphenium's topological invariants.
pub fn evaluate_delta_gate(
    graph: &GrapheniumGraph,
    plan_id: &str,
    modularity_tolerance: f64,
    surprise_threshold: f64,
) -> Result<DeltaGateReport, GrapheniumError> {
    // 1. Isolate the baseline (physical-only) graph
    let mut baseline = extract_baseline_subgraph(graph);
    let _baseline_comms = cluster(&mut baseline, &ClusterOptions::default());
    let baseline_q = modularity_for_graph(&baseline);

    // 2. Isolate the virtual (physical + plan_id) graph
    let mut virtual_graph = GrapheniumGraph::new();
    virtual_graph.set_ast_only(graph.is_ast_only());
    virtual_graph.metadata = graph.metadata.clone();

    for node in graph.nodes() {
        if node.plan_id.is_none() || node.plan_id.as_deref() == Some(plan_id) {
            virtual_graph.upsert_node(node.clone());
        }
    }
    for edge in graph.edges_iter() {
        if edge.plan_id.is_none() || edge.plan_id.as_deref() == Some(plan_id) {
            virtual_graph.add_edge(edge.clone());
        }
    }
    virtual_graph.rebuild_id_index();

    let _virtual_comms = cluster(&mut virtual_graph, &ClusterOptions::default());
    let virtual_q = modularity_for_graph(&virtual_graph);

    let q_delta = virtual_q - baseline_q;

    // 3. Extract surprising connections introduced strictly by the plan
    let all_surprise = surprising_connections(&virtual_graph, 50);
    let plan_surprise_edges: Vec<SurprisingEdge> = all_surprise
        .into_iter()
        .filter(|e| {
            virtual_graph
                .edges_between(&e.source, &e.target)
                .iter()
                .any(|edge| edge.plan_id.as_deref() == Some(plan_id))
                && e.score >= surprise_threshold
        })
        .collect();

    // 4. Trace community structural drift
    let drift = detect_drift(&baseline, &virtual_graph);
    let filtered_drift: Vec<DriftEvent> = drift
        .events
        .into_iter()
        .filter(|e| {
            e.node_id == "cross-boundary"
                || e.node_id == "community-structure"
                || graph
                    .node_data(&e.node_id)
                    .and_then(|n| n.plan_id.as_deref())
                    == Some(plan_id)
        })
        .collect();

    // The gate passes if modularity does not decay beyond tolerance,
    // and no planned edge violates our structural surprise threshold.
    let passes = q_delta >= modularity_tolerance && plan_surprise_edges.is_empty();

    Ok(DeltaGateReport {
        plan_id: plan_id.to_string(),
        passes,
        modularity_baseline: baseline_q,
        modularity_virtual: virtual_q,
        modularity_delta: q_delta,
        plan_surprise_edges,
        drift_events: filtered_drift,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Edge, FileType, Node};

    fn build_graph(node_ids: &[&str], edges: &[(&str, &str)]) -> GrapheniumGraph {
        let mut g = GrapheniumGraph::new();
        for &id in node_ids {
            g.upsert_node(Node::new(id, id, FileType::Code, "f.rs"));
        }
        for &(s, t) in edges {
            g.add_edge(Edge::extracted(s, t, "calls", "f.rs"));
        }
        g
    }

    #[test]
    fn extract_baseline_excludes_planned_nodes() {
        let mut g = build_graph(&["a", "b"], &[("a", "b")]);
        let mut planned = Node::new("plan_x", "PlanX", FileType::Code, "plan.rs");
        planned.plan_id = Some("plan-1".to_string());
        g.upsert_node(planned);
        g.add_edge({
            let mut e = Edge::extracted("plan_x", "a", "calls", "plan.rs");
            e.plan_id = Some("plan-1".to_string());
            e
        });

        let baseline = extract_baseline_subgraph(&g);
        assert_eq!(baseline.node_count(), 2);
        assert_eq!(baseline.edge_count(), 1);
    }

    #[test]
    fn evaluate_delta_gate_passes_for_physical_only_graph() {
        let g = build_graph(&["a", "b", "c"], &[("a", "b"), ("b", "c")]);
        let report = evaluate_delta_gate(&g, "missing-plan", -0.02, 5.0).unwrap();
        assert!(report.passes);
        assert!(report.plan_surprise_edges.is_empty());
    }
}
