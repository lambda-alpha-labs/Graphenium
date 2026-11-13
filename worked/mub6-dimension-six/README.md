# Two exact obstructions in dimension six

### A three-layer, CI-verified Graphenium showcase for the MUB-6 problem: exact arithmetic, Lean certificates, and an embedded Datalog obstruction check.

---

## TL;DR

- The Fourier pair \(\{I_6,F_6\}\) has exactly **16** orthonormal bases unbiased to both members, and **no two of those bases are mutually unbiased**.
- Tao's spectral matrix \(S_6\) has exactly **90** vectors unbiased to both \(I_6\) and \(S_6\), and **no two of those 90 vectors are orthogonal** — so the pair cannot even be extended to a third basis.
- The showcase is three independent layers: a Python exact/numerically certified generator, Graphenium's **embedded Datalog** clique checker, and a Mathlib-free Lean 4 kernel build.
- Graphenium recovers the **16 F6 bases** and **0 MUB pairs between bases** in about **1 second**, with **no Soufflé installation**.
- Lean checks the S6 algebra with **8,100 exact inner products** and the graph certificates with pure kernel `decide`: **no `sorry`, no `native_decide`, no Mathlib** in the core build.
- The C exhaustive search finds exactly **48 Björck biunimodular sequences**.
- `make verify` runs all four checks on a clean Ubuntu runner via GitHub Actions.

**Repository:** [`lambda-alpha-labs/MUB-6`](https://github.com/lambda-alpha-labs/MUB-6)

---

## 1. The setup

The dimension-six MUB problem asks how many orthonormal bases can be pairwise mutually unbiased. The full classification is open, but two finite obstructions are exact and checkable:

1. **The Fourier pair.** Take the standard basis \(I_6\) and the Fourier matrix \(F_6\). There are exactly 48 vectors unbiased to both. They form 16 orthonormal bases, but no two of those 16 bases are mutually unbiased.
2. **Tao's spectral matrix.** Take \(I_6\) and Tao's \(S_6\). There are exactly 90 vectors unbiased to both, but no two of them are orthogonal. A third basis would need six mutually orthogonal vectors from that set, so the pair cannot be extended.

The showcase deliberately separates what is **assumed from the literature** from what is **verified locally**:

- The completeness of the 48-vector and 90-vector sets is the published input (Grassl/Björck; Brierley–Weigert).
- The exact coordinates, graph construction, clique enumeration, and combinatorial obstruction are re-checked here.

That boundary is documented in the [MUB-6 README](https://github.com/lambda-alpha-labs/MUB-6#trust-boundary).

---

## 2. Results

| Data set | Vectors | Orthogonal pairs | MUB pairs | 6-cliques (bases) | MUB pairs between bases |
|---|---:|---:|---:|---:|---:|
| \(F_6\) | 48 | 300 | 144 | 16 | **0** |
| Tao \(S_6\) | 90 | **0** | 1125 | **0** | **0** |

For \(F_6\), every vector is exactly unbiased to \(I_6\) and \(F_6\). The graph is computed in double precision with a certified separation margin:

- every non-edge is at least `0.4307...` away from both thresholds `0` and `6`;
- every edge is within `9.8e-15` of its threshold.

For \(S_6\), the orthogonality graph is computed in **exact integer arithmetic** (no floating point at all), and it is empty.

---

## 3. Three corrections that made the data exact

The showcase documents three corrections that were necessary while implementing the pipeline. This is the kind of detail that usually gets buried in a private notebook.

### 3.1 The \(U_9\) table had two sign errors

The supplied table had the wrong \(i\sqrt{15}\) sign on the two entries involving \(\omega^2\beta\) and \(\omega^2\bar\beta\). The corrected table is the one used in `Algebra.lean` and `python/generate_facts.py`. With the uncorrected table the seed vectors do not have unit-modulus entries.

### 3.2 The \(S_6\) representative must be row-permuted

The standard thesis/paper representative is not the one the supplied 90 seeds are unbiased to. The seeds are unbiased to the row permutation

\[
P=(0,1,2,5,4,3)
\]

of \(S_6\). The matrix \(PS_6\) is Hadamard-equivalent to \(S_6\), and any of the 60 equivalent working row orders gives the same obstruction.

### 3.3 The \(F_6\) vectors are not in the \(r_\pm\) alphabet

The original \(r_\pm\) alphabet produces only 12 of the 48 vectors. The complete set is the Björck biunimodular set, over the alphabet

\[
\mathcal A=\{d^m\zeta^k:-2\le m\le 2,\ 0\le k<12\},
\qquad
d=\frac{1-\sqrt3}{2}+i\sqrt{\frac{\sqrt3}{2}},
\quad \zeta=e^{2\pi i/12}.
\]

The 48 codes are embedded in the Lean certificate; the exhaustive C search that found them is included and reports exactly 48 solutions.

---

## 4. The Lean certificate

The core Lean build is deliberately Mathlib-free. `Algebra.lean` checks, by pure kernel `decide`, that:

- all 90 S6 vectors have unit entries;
- all \(90\times 6 = 540\) inner products with \(S_6\) have squared modulus exactly 6 after the common scaling;
- no two of the 90 S6 vectors are orthogonal — an 8,100-fold kernel computation, not a mask lookup.

`MasterProof.lean` then checks the finite graph certificates:

- the S6 orthogonality mask cross-check is identically zero;
- the 16 F6 bases are six-element cliques with distinct vertices;
- no two of the 16 F6 bases are completely joined in the MUB relation.

The compiled core depends only on `propext` (and `Quot.sound` where needed), not on `sorryAx` and not on `Lean.ofReduceBool` from `native_decide`.

`Eisenstein.lean` is separate. It uses Mathlib for `nlinarith`, `interval_cases`, and `norm_num`, and proves the arithmetic lemma

```lean
theorem eisNorm_ne_six (a b : ℤ) : a ^ 2 - a * b + b ^ 2 ≠ 6
```

plus the finite residue check `∀ a b : ZMod 4, a^2 - a*b + b^2 ≠ 2`.

The core build takes about one minute on a laptop and six jobs in CI.

---

## 5. Graphenium's embedded Datalog layer

This is the part that makes MUB-6 a Graphenium showcase rather than just a Lean project.

The original reference program used Soufflé syntax. The repository now runs the same clique logic entirely inside Graphenium:

```bash
gm query "" --datalog "$(cat graphenium/mub_cliques.dl)" \
  --graph graphenium-out/f6_graph.json
```

`graphenium/build_graph.py` converts the exact `ortho` and `mub` facts into small Graphenium graph JSON files. The Datalog program then enumerates:

- every 6-clique in the orthogonality graph (an orthonormal basis);
- every complete bipartite \(K_{6,6}\) between two bases (a MUB pair).

The bridge maps the mathematical relations onto Graphenium's existing graph EDB:

| Mathematical relation | Graphenium relation | Direction |
|---|---|---|
| orthogonality | `calls` | both directions |
| mutual unbiasedness | `imports` | both directions |
| canonical order \(i < j\) | `contains` | one direction |

Interleaving the canonical-order atoms with the adjacency atoms keeps every partial clique in increasing vertex order, so the join stays small. On Graphenium 0.24.0:

- **F6:** 16 bases, 0 MUB pairs between bases, about **1.1 s**;
- **S6:** no basis and no orthogonal pair, well under a second.

No Soufflé binary is required, and the generated graph/log files use only relative paths.

### 5.1 Semantic trust audit of the Lean certificate

The token-based Lean extractor flags `sorry`/`admit`. It cannot see a theorem that compiles but inherits a custom certificate axiom or a `native_decide` shortcut. Generate a `#print axioms` report with the project's own Lean toolchain, then audit the theorem's dependency cone:

```bash
# Ingest the report (raw text or lean-axioms.v1 JSON)
gm run . --no-semantic --no-viz --lean-axioms mub6-axioms.json

# Audit the certificate theorem
gm trust --theorem MUB6.no_four_mubs_dim6 --forbid-native-decide --json
```

The report classifies each inherited axiom:

| Class | Examples | Meaning |
|---|---|---|
| `standard_kernel` | `propext`, `Classical.choice`, `Quot.sound` | Trusted Lean kernel |
| `incomplete_proof_gap` | `sorryAx` | Incomplete proof |
| `native_eval_shortcut` | `Lean.ofReduceBool`, `Lean.trustCompiler` | `native_decide` trust |
| `external_axiom` | Certificate axioms such as `EdgeAtlasCertificate` | Must be reviewed or allowlisted |

`gm trust` exits non-zero on `sorryAx`, non-allowlisted external axioms, or (with `--forbid-native-decide`) native-decision shortcuts. This is the mechanical counterpart to the prose claim that the core certificate depends only on `propext` / `Quot.sound` and not on `sorryAx` or `native_decide`: the claim becomes a CI-checkable graph fact.

---

## 6. Indexing the showcase with Graphenium

Running Graphenium on the showcase itself gives a small, clean structural index:

| Metric | Value |
|---|---:|
| AST symbols | 91 nodes |
| Structural boundaries | 172 edges |
| Communities | 6 |
| Import resolution | 16 / 16 (**100%**) |
| Ambiguous edges | **0** |
| Trust check | **PASS** |

That index covers the Python generator, the C search tool, the Graphenium bridge and `.dl` program, the Lean core, and the CI/Makefile wiring. It is a useful smoke test that the showcase has no unresolved imports or ambiguous graph edges of its own.

---

## 7. Reproduce

```bash
git clone https://github.com/lambda-alpha-labs/MUB-6
cd MUB-6

# Python facts + certificates, C search, Graphenium Datalog, Lean core
make verify
```

Individual layers:

```bash
python3 python/generate_facts.py
make c-search
make datalog
make lean
```

The C search compiles with `cc -O3` and reports exactly 48 solutions. The Datalog layer requires `gm` on `PATH`; set `GM=/path/to/gm` to override it. The Lean layer uses the pinned `lean-toolchain` and is Mathlib-free by default.

---

## 8. What this demonstrates

This is a useful complement to the larger self-analysis and FLT case studies:

- **Exact arithmetic is practical.** The S6 layer is exact integer arithmetic in \(\mathbb Z[\sqrt5,i\sqrt3,i\sqrt{15}]\), scaled by 8, not a floating-point approximation.
- **Finite graph certificates can be kernel-checked.** Lean verifies the combinatorial obstruction without `native_decide`.
- **Graphenium's embedded Datalog is real.** The clique program runs in `gm`, not an external dependency, and the bridge keeps generated artifacts portable.
- **CI can check the whole stack.** Python, C, Graphenium, and Lean all run from one `make verify` on a fresh Ubuntu runner.

The honest limitation is the same one documented by the repository: the completeness of the 48/90 vector sets is taken from the literature. The code verifies the exact coordinates, the graph construction, the clique enumeration, and the obstruction — not the full dimension-six MUB classification.
