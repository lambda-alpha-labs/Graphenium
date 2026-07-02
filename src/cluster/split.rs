/// Oversized community splitting.
/// After the initial Louvain pass, any community larger than
/// `split_fraction * n` nodes (and at least `min_size` nodes) is treated as
/// a candidate for further subdivision.  We run Louvain again on the induced
/// subgraph and replace the original community ID with new sub-community IDs.
/// Community IDs are renumbered by size (largest = 0) after all splits.
use std::collections::HashMap;

use super::louvain::{run, LouvainConfig};

/// Split oversized communities in `assignments` in-place.
/// - `n` — total number of nodes in the graph
/// - `edges` — full edge list (same indexing as `assignments`)
/// - `split_fraction` — fractional threshold (e.g. 0.25 = 25 %)
/// - `min_size` — minimum community size before splitting is attempted
pub fn split_large(
    assignments: &mut Vec<usize>,
    n: usize,
    edges: &[(usize, usize, f64)],
    split_fraction: f64,
    min_size: usize,
    config: &LouvainConfig,
) {
    let threshold = ((split_fraction * n as f64) as usize).max(min_size);

    // Count community sizes.
    let max_id = assignments.iter().copied().max().unwrap_or(0);
    let mut sizes = vec![0usize; max_id + 1];
    for &c in assignments.iter() {
        sizes[c] += 1;
    }

    // Identify communities to split.
    let to_split: Vec<usize> = sizes
        .iter()
        .enumerate()
        .filter(|&(_, &sz)| sz > threshold)
        .map(|(c, _)| c)
        .collect();

    if to_split.is_empty() {
        return;
    }

    let mut next_id = max_id + 1; // Next available global community ID

    for target_comm in to_split {
        // Nodes (original indices) that belong to this community.
        let members: Vec<usize> = assignments
            .iter()
            .enumerate()
            .filter(|(_, &c)| c == target_comm)
            .map(|(i, _)| i)
            .collect();

        if members.len() <= 1 {
            continue;
        }

        // Build a sequential index for the subgraph.
        let mut global_to_local: HashMap<usize, usize> = HashMap::new();
        for (local, &global) in members.iter().enumerate() {
            global_to_local.insert(global, local);
        }

        // Extract edges entirely within this community.
        let sub_edges: Vec<(usize, usize, f64)> = edges
            .iter()
            .filter_map(|&(u, v, w)| {
                let lu = global_to_local.get(&u)?;
                let lv = global_to_local.get(&v)?;
                Some((*lu, *lv, w))
            })
            .collect();

        // Run Louvain on the subgraph.
        let sub_comms = run(members.len(), &sub_edges, config);

        // Map sub-community IDs to fresh global IDs.
        let sub_max = sub_comms.iter().copied().max().unwrap_or(0);
        // sub_comms are already 0-indexed; allocate sub_max+1 new IDs.
        let id_offset = next_id;
        next_id += sub_max + 1;

        // Write back: replace target_comm with new IDs.
        for (local_i, &global_i) in members.iter().enumerate() {
            assignments[global_i] = id_offset + sub_comms[local_i];
        }
    }

    // Renumber all communities by size (largest = 0).
    renumber_by_size(n, assignments);
}

fn renumber_by_size(_n: usize, assignments: &mut Vec<usize>) {
    let max_id = assignments.iter().copied().max().unwrap_or(0);
    let mut sizes = vec![0usize; max_id + 1];
    for &c in assignments.iter() {
        sizes[c] += 1;
    }

    let mut rank: Vec<usize> = (0..=max_id).collect();
    rank.sort_unstable_by(|&a, &b| sizes[b].cmp(&sizes[a]));

    let mut new_id = vec![0usize; max_id + 1];
    for (new, old) in rank.iter().enumerate() {
        new_id[*old] = new;
    }

    for c in assignments.iter_mut() {
        *c = new_id[*c];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_split_when_all_small() {
        let mut assignments = vec![0, 0, 1, 1];
        let edges = vec![(0, 1, 1.0), (2, 3, 1.0)];
        let cfg = LouvainConfig::default();
        split_large(&mut assignments, 4, &edges, 0.25, 10, &cfg);
        // Communities are [0,0,1,1] — neither is ≥ 25% AND ≥ 10 nodes.
        // They should remain unsplit (just possibly renumbered).
        let c0 = assignments[0];
        assert_eq!(assignments[1], c0);
        let c1 = assignments[2];
        assert_eq!(assignments[3], c1);
        assert_ne!(c0, c1);
    }

    #[test]
    fn splits_oversized_community() {
        // Build a graph of 20 nodes: two 10-node cliques, all in community 0.
        let n = 20;
        // Force all into community 0 initially.
        let mut assignments = vec![0usize; n];

        // Edges: two dense clusters (0-9 and 10-19), weak bridge between them.
        let mut edges: Vec<(usize, usize, f64)> = Vec::new();
        for i in 0..10 {
            for j in (i + 1)..10 {
                edges.push((i, j, 1.0));
            }
        }
        for i in 10..20 {
            for j in (i + 1)..20 {
                edges.push((i, j, 1.0));
            }
        }
        edges.push((9, 10, 0.05)); // weak bridge

        let cfg = LouvainConfig::default();
        // threshold = 0.25 * 20 = 5, min_size = 5 → community of 20 > 5 → split
        split_large(&mut assignments, n, &edges, 0.25, 5, &cfg);

        // After splitting, we should have at least 2 communities.
        let unique: std::collections::HashSet<usize> = assignments.iter().copied().collect();
        assert!(
            unique.len() >= 2,
            "expected at least 2 communities after split, got: {assignments:?}"
        );
    }
}
