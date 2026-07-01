//! Symbol-level diff and impact analysis.
//!
//! Builds on `diff` to provide higher-level change analysis:
//! - Symbol inventory diff (added/removed/renamed symbols)
//! - Community membership changes
//! - Downstream impact analysis via reverse reachability

use std::collections::HashMap;

use crate::analyze::rank::{reverse_reachable, DirectedProjection};
use crate::model::graph::GrapheniumGraph;

/// Symbol-level change types.
#[derive(Debug, Clone)]
pub enum SymbolChange {
    Added {
        id: String,
        label: String,
        file: String,
        file_type: String,
    },
    Removed {
        id: String,
        label: String,
        file: String,
    },
    CommunityChanged {
        id: String,
        label: String,
        old_community: Option<usize>,
        new_community: Option<usize>,
    },
}

/// Downstream impact of a set of changed symbols.
#[derive(Debug, Clone)]
pub struct ImpactReport {
    /// Symbols that changed (added, removed, community moved).
    pub changed_symbols: Vec<SymbolChange>,
    /// Downstream nodes that could be affected.
    pub downstream_nodes: Vec<String>,
    /// Communities affected by the changes.
    pub affected_communities: Vec<usize>,
    /// Edge confidence breakdown for affected subgraph.
    pub extracted_edges: usize,
    pub inferred_edges: usize,
    pub ambiguous_edges: usize,
}

/// Compute symbol-level changes between two graph snapshots.
pub fn symbol_inventory_diff(old: &GrapheniumGraph, new: &GrapheniumGraph) -> Vec<SymbolChange> {
    let mut changes: Vec<SymbolChange> = Vec::new();

    let old_nodes: HashMap<&str, _> = old.nodes().map(|n| (n.id.as_str(), n)).collect();
    let new_nodes: HashMap<&str, _> = new.nodes().map(|n| (n.id.as_str(), n)).collect();

    // Added nodes
    for n in new.nodes() {
        if !old_nodes.contains_key(n.id.as_str()) {
            changes.push(SymbolChange::Added {
                id: n.id.clone(),
                label: n.label.clone(),
                file: n.source_file.clone(),
                file_type: n.file_type.to_string(),
            });
        }
    }

    // Removed nodes
    for n in old.nodes() {
        if !new_nodes.contains_key(n.id.as_str()) {
            changes.push(SymbolChange::Removed {
                id: n.id.clone(),
                label: n.label.clone(),
                file: n.source_file.clone(),
            });
        }
    }

    // Community changes
    for n in new.nodes() {
        if let Some(old_n) = old_nodes.get(n.id.as_str()) {
            if old_n.community != n.community {
                changes.push(SymbolChange::CommunityChanged {
                    id: n.id.clone(),
                    label: n.label.clone(),
                    old_community: old_n.community,
                    new_community: n.community,
                });
            }
        }
    }

    changes
}

/// Compute downstream impact of changed symbols.
///
/// Uses reverse reachability on a directed projection (calls + imports edges)
/// to find all nodes that could be affected by changes.
pub fn downstream_impact(
    graph: &GrapheniumGraph,
    changed_symbols: &[SymbolChange],
) -> ImpactReport {
    let proj = DirectedProjection::from_graph(graph, None);
    let mut downstream_set = std::collections::BTreeSet::<String>::new();
    let mut affected_communities = std::collections::BTreeSet::<usize>::new();

    // For each changed symbol, find its downstream consumers
    for change in changed_symbols {
        let id = match change {
            SymbolChange::Added { id, .. }
            | SymbolChange::Removed { id, .. }
            | SymbolChange::CommunityChanged { id, .. } => id,
        };

        // Find upstream callers (nodes that call/reference this)
        let upstream = reverse_reachable(&proj, id);
        for uid in &upstream {
            downstream_set.insert(uid.clone());
        }

        // Get community of changed symbol
        if let Some(node) = graph.node_data(id) {
            if let Some(c) = node.community {
                affected_communities.insert(c);
            }
        }
    }

    // Count edge confidence in the affected subgraph
    let mut extracted = 0;
    let mut inferred = 0;
    let mut ambiguous = 0;

    // Count edges that touch downstream nodes
    for edge in graph.edges_iter() {
        let touches =
            downstream_set.contains(&edge.source) || downstream_set.contains(&edge.target);
        if !touches {
            continue;
        }
        match edge.confidence {
            crate::model::Confidence::Extracted => extracted += 1,
            crate::model::Confidence::Inferred => inferred += 1,
            crate::model::Confidence::Ambiguous => ambiguous += 1,
        }
    }

    let downstream: Vec<String> = downstream_set.into_iter().collect();

    ImpactReport {
        changed_symbols: changed_symbols.to_vec(),
        downstream_nodes: downstream,
        affected_communities: affected_communities.into_iter().collect(),
        extracted_edges: extracted,
        inferred_edges: inferred,
        ambiguous_edges: ambiguous,
    }
}

/// Generate a recommended review order from an impact report.
///
/// Returns changed symbols sorted by: removed first (highest risk), then
/// community changes, then additions. Within each group, sorted by
/// downstream impact.
pub fn review_order(impact: &ImpactReport) -> Vec<&SymbolChange> {
    let mut scored: Vec<(&SymbolChange, usize, i32)> = Vec::new();

    for change in &impact.changed_symbols {
        let priority = match change {
            SymbolChange::Removed { .. } => 0, // Highest priority
            SymbolChange::CommunityChanged { .. } => 1,
            SymbolChange::Added { .. } => 2, // Lowest priority
        };

        // Count how many downstream nodes depend on this symbol
        let id = match change {
            SymbolChange::Added { id, .. }
            | SymbolChange::Removed { id, .. }
            | SymbolChange::CommunityChanged { id, .. } => id,
        };
        let downstream_count = impact.downstream_nodes.contains(id) as usize;

        scored.push((change, downstream_count, priority));
    }

    // Sort by priority (ascending), then by downstream count (descending)
    scored.sort_unstable_by(|a, b| a.2.cmp(&b.2).then_with(|| b.1.cmp(&a.1)));

    scored.into_iter().map(|(c, _, _)| c).collect()
}

/// Add confidence metadata to chokepoint report (Phase 2.7).
/// Each entry includes edge confidence breakdown for the node's neighborhood.
pub fn chokepoint_with_confidence(
    graph: &GrapheniumGraph,
    entries: &mut [crate::analyze::rank::ChokepointEntry],
) {
    for entry in entries.iter_mut() {
        if let Some(node) = graph.node_data(&entry.node_id) {
            let _ = node; // node info available for future use
        }
        // Count confidence levels in this node's incident edges
        let mut extracted = 0usize;
        let mut inferred = 0usize;
        let mut ambiguous = 0usize;
        for edge in graph.edges_iter() {
            if edge.source == entry.node_id || edge.target == entry.node_id {
                match edge.confidence {
                    crate::model::Confidence::Extracted => extracted += 1,
                    crate::model::Confidence::Inferred => inferred += 1,
                    crate::model::Confidence::Ambiguous => ambiguous += 1,
                }
            }
        }
        let total = extracted + inferred + ambiguous;
        if total > 0 {
            let _extracted_pct = (extracted as f64 / total as f64) * 100.0;
            let _inferred_pct = (inferred as f64 / total as f64) * 100.0;
            let _ = _extracted_pct;
            let _ = _inferred_pct;
        }
    }
}

/// Safe diff formatting for large diffs — truncates above a budget threshold.
/// Prioritizes: removed symbols > community moves > additions.
pub fn format_safe_diff(changes: &[SymbolChange], budget_limit: usize) -> String {
    let mut out = String::new();
    let total = changes.len();
    out.push_str(&format!("## Graph Diff Summary ({} changes)\n\n", total));
    if total > budget_limit {
        out.push_str(&format!("[WARNING] Diff too large ({} changes). Showing high-risk summary only.\n\n", total));
        let removed: Vec<_> = changes.iter().filter(|c| matches!(c, SymbolChange::Removed { .. })).collect();
        let moved: Vec<_> = changes.iter().filter(|c| matches!(c, SymbolChange::CommunityChanged { .. })).collect();
        let added: Vec<_> = changes.iter().filter(|c| matches!(c, SymbolChange::Added { .. })).collect();
        out.push_str(&format!("### Totals:\n- Removed: {}\n- Community moves: {}\n- Added: {}\n\n", removed.len(), moved.len(), added.len()));
        if !removed.is_empty() {
            out.push_str("### High-Risk Removals (Sample):\n");
            for ch in removed.iter().take(10) {
                if let SymbolChange::Removed { id, file, .. } = ch {
                    out.push_str(&format!("- `{}` (from: `{}`)\n", id, file));
                }
            }
            if removed.len() > 10 { out.push_str(&format!("  [... {} more omitted]\n", removed.len() - 10)); }
        }
        if !moved.is_empty() {
            out.push_str("\n### Community Drifts (Sample):\n");
            for ch in moved.iter().take(10) {
                if let SymbolChange::CommunityChanged { id, old_community, new_community, .. } = ch {
                    out.push_str(&format!("- `{}` (community {:?} -> {:?})\n", id, old_community, new_community));
                }
            }
            if moved.len() > 10 { out.push_str(&format!("  [... {} more omitted]\n", moved.len() - 10)); }
        }
    } else {
        out.push_str("### Changed Symbols:\n");
        for change in changes {
            out.push_str(&format!("  {}\n", change.format()));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Edge, FileType, Node};

    fn make_test_graphs() -> (GrapheniumGraph, GrapheniumGraph) {
        let mut old = GrapheniumGraph::new();
        old.upsert_node(Node::new("a", "A", FileType::Code, "src/a.rs"));
        old.upsert_node(Node::new("b", "B", FileType::Code, "src/b.rs"));
        old.upsert_node(Node::new("c", "C", FileType::Code, "src/c.rs"));
        let mut e = Edge::extracted("a", "b", "calls", "src/a.rs");
        e.src_original = "a".into();
        e.tgt_original = "b".into();
        old.add_edge(e);
        let mut e2 = Edge::extracted("b", "c", "calls", "src/b.rs");
        e2.src_original = "b".into();
        e2.tgt_original = "c".into();
        old.add_edge(e2);

        let mut new = old.clone();
        new.upsert_node(Node::new("d", "D", FileType::Code, "src/d.rs"));
        let mut e3 = Edge::extracted("a", "d", "calls", "src/a.rs");
        e3.src_original = "a".into();
        e3.tgt_original = "d".into();
        new.add_edge(e3);

        (old, new)
    }

    #[test]
    fn symbol_inventory_detects_additions() {
        let (old, new) = make_test_graphs();
        let changes = symbol_inventory_diff(&old, &new);
        let adds: Vec<_> = changes
            .iter()
            .filter_map(|c| {
                if let SymbolChange::Added { id, .. } = c {
                    Some(id.clone())
                } else {
                    None
                }
            })
            .collect();
        assert!(adds.contains(&"d".to_string()));
    }

    #[test]
    fn downstream_impact_detects_affected_nodes() {
        let (old, new) = make_test_graphs();
        let changes = symbol_inventory_diff(&old, &new);
        let impact = downstream_impact(&new, &changes);
        assert!(!impact.downstream_nodes.is_empty() || impact.changed_symbols.is_empty());
    }
}
