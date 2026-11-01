# Mapping the proof gaps in Fermat's Last Theorem

### I indexed Kevin Buzzard's `ImperialCollegeLondon/FLT` with Graphenium, fixed the indexer bugs that were lying about the taint, and then proved one of the `sorry`s.

---

## TL;DR

- The first index said **76% of the repo was tainted by `sorry`**. That number was mostly an artifact of duplicate module IDs, missed `public import`s, and unparsed `instance` declarations.
- After fixing the indexer: **4,564 nodes, 1,519 imports 100% resolved, 50 gap declarations, 538 tainted nodes (11.8%)**.
- At declaration level, ignoring import/containment noise: **72 tainted declarations out of 3,732 (1.93%)**.
- The bottlenecks are not spread evenly. **`TateCurve.lean` has 11 gaps; `Torsion.lean` has 6; `Flat.lean` has 3 gaps with 473–474-node downstream cones.**
- I proved all four `sorry`s in `WeierstrassCurve.galoisRepresentation` and verified it with Lean 4.35.0.
- The graph went from **50 → 49 gap declarations** and **538 → 537 tainted nodes**. The small global move is the interesting part: **taint is module-coherent**, so clearing one leaf inside a still-sorryful module barely changes the repo-wide number.

---

## 1. The setup

```bash
git clone https://github.com/ImperialCollegeLondon/FLT /tmp/FLT
cd /tmp/FLT
gm init
gm run . --no-semantic --no-viz
gm doctor --graph graphenium-out/graph.json --resolution
```

The repo pins Lean `v4.35.0` and mathlib `56a005855ea1`.

---

## 2. The first index was wrong

The initial run produced:

| Metric | First index |
|---|---:|
| Nodes | 3,290 |
| Edges | 15,021 |
| `unverified_sorry` edges | 49 |
| Natively tainted nodes | 2,501 / 3,290 (**76.0%**) |
| `tainted_by_proof_gap` query | timed out (>30 s) |

That 76% was not real. The indexer had several Lean-specific edge cases:

1. **Duplicate `Basic.lean` stems.** Twenty different `Basic.lean` files collapsed into one `basic` node.
2. **`instance` declarations were not parsed**, including `instance ... := sorry`.
3. **`public import` was missed.** 807 of 1,159 FLT imports were invisible; `FLT/Proof.lean` had zero imports captured.
4. **Lossy Lean names collided.** `actionTensorCAlg'` and `actionTensorCAlg` produced the same ID.
5. **The taint query materialised the full transitive closure** and blew the solver budget.
6. **Louvain clustering was non-deterministic.** Identical graphs produced 9, 10, 11, or 20 communities because `HashMap` iteration order leaked into tie-breaking.
7. **Import-site stubs overwrote real module nodes**, so a module could be attributed to the file that imported it rather than its own file.

After the fixes, the clean pre-proof graph was:

| Metric | Clean index |
|---|---:|
| Nodes | 4,564 |
| Edges | 15,050 |
| Imports resolved | 1,519 / 1,519 (**100%**) |
| `unverified_sorry` edges | 50 |
| Tainted nodes | 538 (**11.8%**) |
| Communities | 11 (deterministic) |

The headline: **the original 76% taint was mostly fake.** The real module-propagated number is 11.8%; the strict declaration-level number is 1.93%.

> **Why not just `grep "sorry"` and `lake build`?**
>
> `grep` finds textual occurrences, not the declaration graph, so it cannot tell you which theorems depend on a gap. `lake build` proves the whole project compiles (or fails), but it does not expose the reverse dependency cone, module/import propagation, or the difference between the 11.8% module-import taint and the 1.93% strict declaration-level taint. Graphenium's Datalog layer computes the reverse transitive closure and lets you separate those views.

---

## 3. The three exact queries

### Proof-gap declarations

```bash
gm query "proof-gap" \
  --datalog "?- has_proof_gap(X)." \
  --graph graphenium-out/graph.json
```

Result after the proof: **49 rows** (down from 50).

### Taint cone

```bash
gm query "taint" \
  --datalog "?- tainted_by_proof_gap(X)." \
  --graph graphenium-out/graph.json
```

Result after the proof: **537 rows** (down from 538).

### Report head

```bash
head -n 120 graphenium-out/GRAPH_REPORT.md
```

The final summary:

```text
Nodes       4564
Edges       15054
Communities 10
EXTRACTED   5349 (36%)
INFERRED    9656 (64%)
AMBIGUOUS   49 (0%)
```

---

## 4. The topological proof-gap map

`tainted_by_proof_gap` propagates backwards from each `unverified_sorry` edge through calls, imports, inheritance, implementations, and containment.

Two views matter:

- **Full cone**: every node that transitively depends on the gap, including module/import propagation.
- **Declaration-only cone**: only call/inheritance/implementation edges between declarations.

| Gap | Full downstream cone | Declaration-only cone | File |
|---|---:|---:|---|
| `WeierstrassCurve.resultant_Φ_ΨSq` | 474 | 1 | `Flat.lean` |
| `WeierstrassCurve.isCoprime_Φ_ΨSq` | 474 | 1 | `Flat.lean` |
| `WeierstrassCurve.torsion_flat_of_good_reduction` | 473 | 1 | `Flat.lean` |
| `WeierstrassCurve.galoisRep` | 444 | **9** | `Torsion.lean` |
| `WeierstrassCurve.tateEquivSepClosure` | 443 | **6** | `TateCurve.lean` |
| `Group.subsingleton_of_pow_prime_eq_one` | 441 | 4 | `CompactHausdorffRings.lean` |
| `IsDedekindDomain.diamond_fix` | 441 | 3 | `GLnDefs.lean` |
| `FLT.Bosses.B4_proof` | 440 | 4 | `Proof.lean` |
| `Deformation.flatFunctor` | — | 5 | `LiftFunctor.lean` |

The bottlenecks cluster by module:

| Module | Gap declarations |
|---|---:|
| `KnownIn1980s/EllipticCurves/TateCurve.lean` | 11 |
| `EllipticCurve/Torsion.lean` | 6 (5 after the proof) |
| `Data/HurwitzRatHat.lean` | 4 |
| `GlobalLanglandsConjectures/GLnDefs.lean` | 4 |
| `GlobalLanglandsConjectures/GLzero.lean` | 4 |
| `KnownIn1980s/EllipticCurves/Flat.lean` | 3 |

This is the map to attack: **clear whole bottleneck modules, not isolated leaves.**

---

## 5. The proof

Target: the four `sorry`s in `WeierstrassCurve.galoisRepresentation` in `FLT/EllipticCurve/Torsion.lean`.

The original code even says:

```lean
one_smul := sorry -- these should all be easy
mul_smul := sorry
smul_zero := sorry
smul_add := sorry
```

The proof uses the existing `WeierstrassCurve.Points.map_id` and `Points.map_comp` lemmas plus the generic `map_zero` / `map_add`:

```lean
noncomputable instance WeierstrassCurve.galoisRepresentation
    (K : Type u) [Field K] [DecidableEq K] [Algebra k K] :
    DistribMulAction (K ≃ₐ[k] K) (E⁄K).Point where
      one_smul P := by
        change WeierstrassCurve.Affine.Point.map ((1 : K ≃ₐ[k] K) : K →ₐ[k] K) P = P
        rw [show ((1 : K ≃ₐ[k] K) : K →ₐ[k] K) = AlgHom.id k K from rfl]
        exact DFunLike.congr_fun (WeierstrassCurve.Points.map_id E K) P
      mul_smul g h P := by
        change WeierstrassCurve.Affine.Point.map ((g * h : K ≃ₐ[k] K) : K →ₐ[k] K) P =
          WeierstrassCurve.Affine.Point.map (g : K →ₐ[k] K)
            (WeierstrassCurve.Affine.Point.map (h : K →ₐ[k] K) P)
        rw [show ((g * h : K ≃ₐ[k] K) : K →ₐ[k] K) =
          (g : K →ₐ[k] K).comp (h : K →ₐ[k] K) from rfl]
        exact (DFunLike.congr_fun
          (WeierstrassCurve.Points.map_comp E K K K (h : K →ₐ[k] K) (g : K →ₐ[k] K)) P).symm
      smul_zero g := by
        change WeierstrassCurve.Affine.Point.map (g : K →ₐ[k] K) (0 : (E⁄K).Point) = 0
        exact map_zero _
      smul_add g P Q := by
        change WeierstrassCurve.Affine.Point.map (g : K →ₐ[k] K) (P + Q) =
          WeierstrassCurve.Affine.Point.map (g : K →ₐ[k] K) P +
            WeierstrassCurve.Affine.Point.map (g : K →ₐ[k] K) Q
        exact map_add _ _ _
```

Public proof commit on the fork: [`6dbb0de`](https://github.com/lambda-alpha-labs/FLT/commit/6dbb0de09cf1e0af664a79cdd28ad362b8ff06e0)

Lean verification:

```text
$ lake build FLT.EllipticCurve.Torsion
✔ [3564/3564] Built FLT.EllipticCurve.Torsion
Build completed successfully (3564 jobs).
```

The file still has five unrelated `sorry`s, but `galoisRepresentation` is now sorry-free.

---

## 6. The virtual AST plan

Before editing, Graphenium's planning workspace was used to declare the four proof obligations and their dependencies.

```text
Plan `prove-galois-representation`: ✅ PASS (dynamic delta gating)
Modularity Delta (ΔQ): 0.0064 (0.7294 → 0.7358)
```

After the edit, a graph-node-level plan verifies the real edges:

```text
$ gm check --graph graphenium-out/graph.json \
    --min-resolution 0 --max-ambiguous 100 \
    --plan prove-galois-representation-final2

=== Graphenium Plan Compliance Gate ===
Plan ID: prove-galois-representation-final2
Compliance: PASS

Implemented: ["galoisRepresentation"]
Implemented edges:
  galoisRepresentation -[calls]-> map_id
  galoisRepresentation -[calls]-> map_comp
```

---

## 7. What actually changed

| Metric | Before | After |
|---|---:|---:|
| Nodes | 4,564 | 4,564 |
| Edges | 15,050 | 15,054 |
| Gap declarations | 50 | **49** |
| `unverified_sorry` edges | 50 | **49** |
| Tainted nodes | 538 | **537** |
| Tainted local modules | 92 / 271 | 92 / 271 |
| Tainted local declarations | 446 / 3,732 | **445 / 3,732** |
| Declaration-only taint | 72 | **71** |
| Lean `sorry` occurrences | 60 | **56** |
| Communities | 11 | 10 |

Graph diff:

```text
Nodes added: 0  |  removed: 0
Edges added: 5  |  removed: 1
```

Anonymous instances now use a stable signature-based ID rather than a line number, so the `DecidableEq (AlgebraicClosure ℚ)` instance keeps the same node ID even though the proof added 16 lines above it.

The important diff is:

```text
- galoisRepresentation -[unverified_sorry]-> galoisRepresentation
+ galoisRepresentation -[calls]-> Points.map_id
+ galoisRepresentation -[calls]-> Points.map_comp
+ galoisRepresentation -[calls]-> Points.map
+ galoisRepresentation -[calls]-> SemialgHom.comp
+ galoisRepresentation -[calls]-> AlgHom.map_zero
```

### Why the global taint only moved by one

`galoisRepresentation` has a **437-node downstream blast radius**, but only **one node was uniquely tainted by this gap**: the instance itself.

The other 436 nodes in its cone remain tainted. `Torsion.lean` still contains five other gaps:

- `n_torsion_finite`
- `n_torsion_card`
- `group_theory_lemma`
- `Module.Finite` instance
- `galoisRep`

Many of those 436 nodes are also tainted by other bottleneck modules, but the module-local effect is already enough to mask the removal: the module stays tainted, so its downstream cone stays tainted.

That is the core result: **proof-gap taint is module-coherent.** Removing one leaf from a module that still has other `sorry`s does not clear the module, so it does not clear the module's downstream cone.

This is also why the repo-wide numbers are misleading if you only count individual `sorry`s:

- 50 gap declarations,
- 538 tainted nodes,
- but only **72 declarations** are tainted once import/containment noise is removed,
- and the top three `Flat.lean` gaps have nearly identical cone sizes (473–474 nodes each), so they are a single bottleneck cluster rather than three independent risks.

---

## 8. Verification gates

```text
$ gm check --graph graphenium-out/graph.json \
    --min-resolution 0 --max-ambiguous 100

Import resolution: 100% (1519/1519)
Call resolution: [SKIPPED] (AST-only)
Combined resolution: 97% (threshold: 0%)
Ambiguous edges: 49 (threshold: 100 max)

✓ Trust check PASSED
```

`gm doctor`:

```text
graph file (graphenium-out/graph.json) ... OK — 4564 nodes, 15054 edges, 10 communities (AST-only)
- Imports resolved: 100% (1519/1519)
- Calls resolved: 49% (4676/9591)
- Methods resolved: 58% (68/117)
- Ambiguous edges: 49
- Unresolved references: 0
```

---

## 9. Reproduce

```bash
cd /tmp/FLT

# Re-index
gm run . --no-semantic --no-viz

# Quality gates
gm doctor --graph graphenium-out/graph.json --resolution
gm check --graph graphenium-out/graph.json --min-resolution 0 --max-ambiguous 100

# Proof-gap map
gm query "proof-gap" --datalog "?- has_proof_gap(X)." --graph graphenium-out/graph.json
gm query "taint" --datalog "?- tainted_by_proof_gap(X)." --graph graphenium-out/graph.json

# Lean verification
lake build FLT.EllipticCurve.Torsion
```

---

## 10. What I would attack next

1. **`Flat.lean`**: 3 gaps, 473–474-node cones each. Clearing this cluster should produce the first real drop in the global taint number.
2. **`TateCurve.lean`**: 11 gaps. This is the densest bottleneck in the repo.
3. **`Torsion.lean`**: 5 gaps remain after this proof. `galoisRep` alone has a declaration-only cone of 9.

The repo-wide lesson is simple: **the unit of proof-gap taint is the module, not the `sorry`.** A single easy lemma is a nice demo, but the taint number only moves when a whole bottleneck module goes green.
