/// Native Louvain community detection (Blondel et al., 2008).
///
/// Runs the two-phase Louvain method until modularity gain falls below
/// `threshold` or `max_level` aggregation levels are reached.
///
/// Returns a `Vec<usize>` of length `n` where `result[i]` is the community
/// ID of node `i`. Community IDs are renumbered 0..K, largest community first.
use std::collections::HashMap;

// ── Public types ──────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct LouvainConfig {
    /// RNG seed for deterministic node-order shuffling.
    pub seed: u64,
    /// Maximum number of aggregation levels.
    pub max_level: usize,
    /// Modularity gain below which Phase 1 stops.
    pub threshold: f64,
}

impl Default for LouvainConfig {
    fn default() -> Self {
        Self {
            seed: 42,
            max_level: 10,
            threshold: 1e-4,
        }
    }
}

// ── Compact graph representation ───────────────────────────────────────────────

/// Adjacency-list graph used internally by the algorithm.
/// Undirected: each edge (u,v,w) appears as (v,w) in adj[u] AND (u,w) in adj[v].
#[derive(Clone)]
pub struct LGraph {
    pub n: usize,
    pub adj: Vec<Vec<(usize, f64)>>,
    pub degrees: Vec<f64>, // weighted degree of each node
    pub two_m: f64,        // Σ degrees = 2 * total edge weight
}

impl LGraph {
    pub fn new(n: usize) -> Self {
        Self {
            n,
            adj: vec![Vec::new(); n],
            degrees: vec![0.0; n],
            two_m: 0.0,
        }
    }

    pub fn add_edge(&mut self, u: usize, v: usize, w: f64) {
        self.adj[u].push((v, w));
        self.degrees[u] += w;
        if u != v {
            self.adj[v].push((u, w));
            self.degrees[v] += w;
        }
        self.two_m += 2.0 * w;
    }

    /// Modularity Q = Σ_c [ L_c/m  -  (Σ_tot_c / 2m)² ]
    /// where m = total edge weight, Σ_tot_c = sum of degrees in community c.
    pub fn modularity(&self, community: &[usize]) -> f64 {
        if self.two_m == 0.0 {
            return 0.0;
        }
        let num_c = community.iter().copied().max().map(|m| m + 1).unwrap_or(0);
        let mut l_c = vec![0.0f64; num_c]; // internal edge weight per community
        let mut tot = vec![0.0f64; num_c]; // sum of degrees per community

        for i in 0..self.n {
            let ci = community[i];
            tot[ci] += self.degrees[i];
            for &(j, w) in &self.adj[i] {
                if community[j] == ci && j >= i {
                    l_c[ci] += w;
                }
            }
        }

        let m = self.two_m / 2.0;
        l_c.iter()
            .zip(tot.iter())
            .map(|(&l, &t)| l / m - (t / self.two_m).powi(2))
            .sum()
    }
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Run Louvain community detection on a graph with `n` nodes and the given
/// weighted edge list.  Returns community assignments (0-indexed, largest first).
pub fn run(n: usize, edges: &[(usize, usize, f64)], config: &LouvainConfig) -> Vec<usize> {
    if n == 0 {
        return Vec::new();
    }

    let mut g = LGraph::new(n);
    for &(u, v, w) in edges {
        g.add_edge(u, v, w);
    }

    if g.two_m == 0.0 {
        // No edges — trivial partition: each node its own community
        return (0..n).collect();
    }

    let mut rng = Xorshift64::new(config.seed.max(1));

    // `orig_to_current[i]` maps original node i to its current (super)node
    // in the aggregated graph at each level.
    let mut orig_to_current: Vec<usize> = (0..n).collect();
    let mut current = g;
    let mut prev_q = f64::NEG_INFINITY;

    for _level in 0..config.max_level {
        let local_comm = phase1(&current, &mut rng);
        let gain = current.modularity(&local_comm) - prev_q;

        // Always apply this level's partition to the mapping.
        for x in orig_to_current.iter_mut() {
            *x = local_comm[*x];
        }

        // Compact community IDs to 0..K.
        let (renumber, num_c) = compact_ids(&local_comm, current.n);
        for x in orig_to_current.iter_mut() {
            *x = renumber[*x];
        }

        // Stop if gain too small, already collapsed, or can't improve further.
        if gain < config.threshold || num_c <= 1 || num_c >= current.n {
            break;
        }

        current = aggregate(&current, &local_comm, &renumber, num_c);
        prev_q = current.modularity(&(0..current.n).collect::<Vec<_>>());
    }

    renumber_by_size(n, &orig_to_current)
}

// ── Phase 1: local modularity optimisation ────────────────────────────────────

fn phase1(g: &LGraph, rng: &mut Xorshift64) -> Vec<usize> {
    let mut comm: Vec<usize> = (0..g.n).collect();
    // Sum of degrees of all nodes in community c.
    let mut comm_deg: Vec<f64> = g.degrees.clone();

    let mut order: Vec<usize> = (0..g.n).collect();
    let mut improved = true;

    while improved {
        improved = false;
        rng.shuffle(&mut order);

        for &i in &order {
            let ci = comm[i];
            let ki = g.degrees[i];

            // Weight from i to nodes in its current community (excluding i).
            let k_i_ci: f64 = g.adj[i]
                .iter()
                .filter(|&&(j, _)| j != i && comm[j] == ci)
                .map(|&(_, w)| w)
                .sum();

            // Gain of keeping i in ci (after virtual removal): f(ci) = k_{i,ci} - (Σ_tot_ci - k_i)*k_i/2m
            let f_ci = k_i_ci - (comm_deg[ci] - ki) * ki / g.two_m;

            // Weights from i to each neighbouring community.
            let mut neigh: HashMap<usize, f64> = HashMap::new();
            for &(j, w) in &g.adj[i] {
                if j != i {
                    *neigh.entry(comm[j]).or_insert(0.0) += w;
                }
            }

            let mut best_f = f_ci;
            let mut best_c = ci;

            for (c, k_i_c) in &neigh {
                if *c == ci {
                    continue;
                }
                // Gain of moving i into community c: f(c) = k_{i,c} - Σ_tot_c * k_i / 2m
                let f_c = k_i_c - comm_deg[*c] * ki / g.two_m;
                if f_c > best_f {
                    best_f = f_c;
                    best_c = *c;
                }
            }

            if best_c != ci {
                comm_deg[ci] -= ki;
                comm[i] = best_c;
                comm_deg[best_c] += ki;
                improved = true;
            }
        }
    }

    comm
}

// ── Phase 2: community aggregation ────────────────────────────────────────────

fn aggregate(g: &LGraph, comm: &[usize], renumber: &[usize], num_c: usize) -> LGraph {
    let new_comm: Vec<usize> = comm.iter().map(|&c| renumber[c]).collect();

    // Accumulate edge weights between super-nodes.
    // Iterate adj[i] with j >= i to visit each original edge once.
    let mut ew: HashMap<(usize, usize), f64> = HashMap::new();
    for i in 0..g.n {
        for &(j, w) in &g.adj[i] {
            if j >= i {
                let a = new_comm[i];
                let b = new_comm[j];
                let key = if a <= b { (a, b) } else { (b, a) };
                *ew.entry(key).or_insert(0.0) += w;
            }
        }
    }

    let mut ng = LGraph::new(num_c);
    for ((a, b), w) in ew {
        ng.add_edge(a, b, w);
    }
    ng
}

// ── Helpers ────────────────────────────────────────────────────────────────────

/// Compact community IDs to 0..K (preserving encounter order).
fn compact_ids(comm: &[usize], n: usize) -> (Vec<usize>, usize) {
    let max_id = comm.iter().copied().max().unwrap_or(0);
    let mut map = vec![usize::MAX; max_id + 1];
    let mut next = 0usize;
    for &c in comm.iter().take(n) {
        if map[c] == usize::MAX {
            map[c] = next;
            next += 1;
        }
    }
    (map, next)
}

/// Renumber communities so community 0 is the largest, 1 is second-largest, etc.
fn renumber_by_size(_n: usize, assignments: &[usize]) -> Vec<usize> {
    let max_id = assignments.iter().copied().max().unwrap_or(0);
    let mut sizes = vec![0usize; max_id + 1];
    for &c in assignments {
        sizes[c] += 1;
    }

    let mut rank: Vec<usize> = (0..=max_id).collect();
    rank.sort_unstable_by(|&a, &b| sizes[b].cmp(&sizes[a]));

    let mut new_id = vec![0usize; max_id + 1];
    for (new, old) in rank.iter().enumerate() {
        new_id[*old] = new;
    }

    assignments.iter().map(|&c| new_id[c]).collect()
}

// ── Minimal seeded PRNG ────────────────────────────────────────────────────────

struct Xorshift64(u64);

impl Xorshift64 {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn shuffle<T>(&mut self, slice: &mut [T]) {
        let n = slice.len();
        for i in (1..n).rev() {
            let j = (self.next() as usize) % (i + 1);
            slice.swap(i, j);
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Two triangles joined by a single weak edge.
    /// Expected: nodes 0-2 in one community, nodes 3-5 in another.
    #[test]
    fn two_triangles_detect_communities() {
        let cfg = LouvainConfig::default();
        #[rustfmt::skip]
        let edges = vec![
            (0, 1, 1.0), (1, 2, 1.0), (0, 2, 1.0),   // triangle A
            (3, 4, 1.0), (4, 5, 1.0), (3, 5, 1.0),   // triangle B
            (2, 3, 0.05),                               // weak bridge
        ];
        let c = run(6, &edges, &cfg);

        assert_eq!(c[0], c[1], "0 and 1 should be in same community");
        assert_eq!(c[1], c[2], "1 and 2 should be in same community");
        assert_eq!(c[3], c[4], "3 and 4 should be in same community");
        assert_eq!(c[4], c[5], "4 and 5 should be in same community");
        assert_ne!(
            c[0], c[3],
            "triangle A and B should be different communities"
        );
    }

    #[test]
    fn single_node_returns_one_community() {
        let c = run(1, &[], &LouvainConfig::default());
        assert_eq!(c, vec![0]);
    }

    #[test]
    fn no_edges_each_node_own_community() {
        let c = run(4, &[], &LouvainConfig::default());
        assert_eq!(c.len(), 4);
        // Each node must be in a distinct community
        let unique: std::collections::HashSet<_> = c.iter().copied().collect();
        assert_eq!(unique.len(), 4);
    }

    #[test]
    fn fully_connected_graph_one_community() {
        // Complete graph K4: all nodes should end up in one community
        let edges = vec![
            (0, 1, 1.0),
            (0, 2, 1.0),
            (0, 3, 1.0),
            (1, 2, 1.0),
            (1, 3, 1.0),
            (2, 3, 1.0),
        ];
        let c = run(4, &edges, &LouvainConfig::default());
        assert!(
            c.iter().all(|&x| x == c[0]),
            "all nodes should be in community 0, got {c:?}"
        );
    }

    #[test]
    fn largest_community_gets_id_zero() {
        // Triangle A (3 nodes) + isolated node 3
        let edges = vec![(0, 1, 1.0), (1, 2, 1.0), (0, 2, 1.0)];
        let c = run(4, &edges, &LouvainConfig::default());
        // Community 0 must be the largest (nodes 0,1,2)
        let size_of_comm_0 = c.iter().filter(|&&x| x == 0).count();
        assert!(size_of_comm_0 >= 2, "community 0 should be the largest");
    }

    #[test]
    fn modularity_increases() {
        let cfg = LouvainConfig::default();
        let edges = vec![
            (0, 1, 1.0),
            (1, 2, 1.0),
            (0, 2, 1.0),
            (3, 4, 1.0),
            (4, 5, 1.0),
            (3, 5, 1.0),
            (2, 3, 0.05),
        ];
        let mut g = LGraph::new(6);
        for &(u, v, w) in &edges {
            g.add_edge(u, v, w);
        }

        let singleton: Vec<usize> = (0..6).collect();
        let q_singleton = g.modularity(&singleton);

        let comms = run(6, &edges, &cfg);
        let q_final = g.modularity(&comms);

        assert!(
            q_final > q_singleton,
            "Louvain should improve modularity: {q_singleton:.4} -> {q_final:.4}"
        );
    }
}
