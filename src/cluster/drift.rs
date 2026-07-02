//! Architecture drift detection: track community boundary changes over time.
//!
//! Drift detection helps identify architectural erosion: community boundaries
//! becoming blurred, unexpected cross-boundary dependencies forming, or hub
//! nodes changing community.
//!
//! Use with snapshot comparison:
//!   gm diff --before old-graph.json --after new-graph.json --drift

use crate::model::GrapheniumGraph;

/// A detected drift event between two graph snapshots.
#[derive(Debug, Clone)]
pub struct DriftEvent {
    pub kind: DriftKind,
    pub node_id: String,
    pub label: String,
    pub old_community: Option<usize>,
    pub new_community: Option<usize>,
    pub severity: DriftSeverity,
    pub detail: String,
}

/// Types of architecture drift.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriftKind {
    CommunityChanged,
    NewCrossBoundaryEdge,
    HubMoved,
    CommunitySplit,
    CommunityMerge,
    BoundaryViolation,
}

/// How severe the drift is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriftSeverity {
    Info,
    Warning,
    Critical,
}

/// Result of drift analysis between two snapshots.
#[derive(Debug, Clone)]
pub struct DriftReport {
    pub events: Vec<DriftEvent>,
    pub old_community_count: usize,
    pub new_community_count: usize,
}

/// Detect drift between two graph snapshots.
/// Compares communities and cross-boundary edges between the old and new graph.
pub fn detect_drift(old: &GrapheniumGraph, new: &GrapheniumGraph) -> DriftReport {
    let mut events = Vec::new();

    // Build community maps
    let old_comms: std::collections::HashMap<String, Option<usize>> = old
        .nodes()
        .filter_map(|n| {
            let id = n.id.clone();
            Some((id, n.community))
        })
        .collect();

    let new_comms: std::collections::HashMap<String, Option<usize>> = new
        .nodes()
        .filter_map(|n| {
            let id = n.id.clone();
            Some((id, n.community))
        })
        .collect();

    // 1. Detect community changes for nodes that exist in both graphs
    for (id, old_comm) in &old_comms {
        if let Some(new_comm) = new_comms.get(id) {
            if old_comm != new_comm {
                let label = old
                    .node_data(id)
                    .map(|n| n.label.clone())
                    .unwrap_or_else(|| id.clone());
                let severity = match (old_comm, new_comm) {
                    (Some(_), None) => DriftSeverity::Warning,
                    (None, Some(_)) => DriftSeverity::Info,
                    _ => DriftSeverity::Info,
                };
                events.push(DriftEvent {
                    kind: DriftKind::CommunityChanged,
                    node_id: id.clone(),
                    label,
                    old_community: *old_comm,
                    new_community: *new_comm,
                    severity,
                    detail: format!("community {:?} → {:?}", old_comm, new_comm),
                });
            }
        }
    }

    // 2. Detect new cross-boundary edges (edges that cross community boundaries)
    let old_boundary_crossings = count_boundary_crossings(old, &old_comms);
    let new_boundary_crossings = count_boundary_crossings(new, &new_comms);

    if new_boundary_crossings > old_boundary_crossings {
        events.push(DriftEvent {
            kind: DriftKind::NewCrossBoundaryEdge,
            node_id: "cross-boundary".to_string(),
            label: "Cross-boundary edges".to_string(),
            old_community: Some(old_boundary_crossings),
            new_community: Some(new_boundary_crossings),
            severity: DriftSeverity::Warning,
            detail: format!(
                "cross-boundary edges increased: {} → {}",
                old_boundary_crossings, new_boundary_crossings
            ),
        });
    }

    // 3. Detect community count changes (split/merge)
    let old_community_count = count_unique_communities(old);
    let new_community_count = count_unique_communities(new);

    if new_community_count > old_community_count {
        events.push(DriftEvent {
            kind: DriftKind::CommunitySplit,
            node_id: "community-structure".to_string(),
            label: "Community count".to_string(),
            old_community: Some(old_community_count),
            new_community: Some(new_community_count),
            severity: DriftSeverity::Info,
            detail: format!(
                "communities increased: {} → {}",
                old_community_count, new_community_count
            ),
        });
    } else if new_community_count < old_community_count {
        events.push(DriftEvent {
            kind: DriftKind::CommunityMerge,
            node_id: "community-structure".to_string(),
            label: "Community count".to_string(),
            old_community: Some(old_community_count),
            new_community: Some(new_community_count),
            severity: DriftSeverity::Info,
            detail: format!(
                "communities decreased: {} → {}",
                old_community_count, new_community_count
            ),
        });
    }

    // 4. Hub migration: nodes with degree > 5 that changed community
    let hub_ids: std::collections::HashSet<String> = events
        .iter()
        .filter(|e| {
            e.kind == DriftKind::CommunityChanged
                && new
                    .edges_iter()
                    .filter(|edge| edge.source == e.node_id || edge.target == e.node_id)
                    .count()
                    > 5
        })
        .map(|e| e.node_id.clone())
        .collect();
    for id in &hub_ids {
        if let Some(label) = events
            .iter()
            .find(|e| e.node_id == *id)
            .map(|e| e.label.clone())
        {
            let oc = events
                .iter()
                .find(|e| e.node_id == *id)
                .and_then(|e| e.old_community);
            let nc = events
                .iter()
                .find(|e| e.node_id == *id)
                .and_then(|e| e.new_community);
            events.push(DriftEvent {
                kind: DriftKind::HubMoved,
                node_id: id.clone(),
                label,
                old_community: oc,
                new_community: nc,
                severity: DriftSeverity::Critical,
                detail: format!("hub node moved community {:?} → {:?} (degree > 5)", oc, nc),
            });
        }
    }

    DriftReport {
        events,
        old_community_count,
        new_community_count,
    }
}

/// Count edges that cross community boundaries.
fn count_boundary_crossings(
    graph: &GrapheniumGraph,
    communities: &std::collections::HashMap<String, Option<usize>>,
) -> usize {
    let mut count = 0;
    for edge in graph.edges_iter() {
        let src_comm = communities.get(&edge.source).and_then(|c| *c);
        let tgt_comm = communities.get(&edge.target).and_then(|c| *c);
        if let (Some(sc), Some(tc)) = (src_comm, tgt_comm) {
            if sc != tc {
                count += 1;
            }
        }
    }
    count
}

/// Count unique communities in the graph.
fn count_unique_communities(graph: &GrapheniumGraph) -> usize {
    graph
        .nodes()
        .filter_map(|n| n.community)
        .collect::<std::collections::BTreeSet<_>>()
        .len()
}

/// Format a drift report as a human-readable string.
pub fn format_drift(report: &DriftReport) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        "# Architecture Drift Report\n\n\
         Communities: {} → {}\n\n",
        report.old_community_count, report.new_community_count
    ));

    if report.events.is_empty() {
        output.push_str("No drift detected.\n");
        return output;
    }

    // Group by severity
    let critical: Vec<_> = report
        .events
        .iter()
        .filter(|e| e.severity == DriftSeverity::Critical)
        .collect();
    let warnings: Vec<_> = report
        .events
        .iter()
        .filter(|e| e.severity == DriftSeverity::Warning)
        .collect();
    let info: Vec<_> = report
        .events
        .iter()
        .filter(|e| e.severity == DriftSeverity::Info)
        .collect();

    if !critical.is_empty() {
        output.push_str(&format!("## Critical ({} events)\n", critical.len()));
        for e in &critical {
            output.push_str(&format!("  - {}: {} ({})\n", e.label, e.detail, e.node_id));
        }
        output.push('\n');
    }

    if !warnings.is_empty() {
        output.push_str(&format!("## Warnings ({} events)\n", warnings.len()));
        for e in &warnings {
            output.push_str(&format!("  - {}: {} ({})\n", e.label, e.detail, e.node_id));
        }
        output.push('\n');
    }

    if !info.is_empty() {
        output.push_str(&format!("## Info ({} events)\n", info.len()));
        for e in &info {
            output.push_str(&format!("  - {}: {} ({})\n", e.label, e.detail, e.node_id));
        }
        output.push('\n');
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{FileType, Node};

    #[test]
    fn no_drift_for_identical_graphs() {
        let graph = GrapheniumGraph::new();
        let report = detect_drift(&graph, &graph);
        assert!(report.events.is_empty());
    }

    #[test]
    fn community_changes_detected() {
        let mut old = GrapheniumGraph::new();
        let mut new = GrapheniumGraph::new();
        let mut n = Node::new("node_a", "NodeA", FileType::Code, "src/a.rs");
        n.community = Some(1);
        old.upsert_node(n.clone());
        n.community = Some(2);
        new.upsert_node(n);

        let report = detect_drift(&old, &new);
        assert!(report
            .events
            .iter()
            .any(|e| e.kind == DriftKind::CommunityChanged));
    }

    #[test]
    fn drift_report_contains_community_count() {
        let old = GrapheniumGraph::new();
        let mut new = GrapheniumGraph::new();
        let n = Node::new("node_a", "NodeA", FileType::Code, "src/a.rs");
        new.upsert_node(n);

        let report = detect_drift(&old, &new);
        // Both have 0 communities (no cluster run), so counts match
        assert_eq!(report.old_community_count, report.new_community_count);
    }
}
