# Performance and Efficiency Benchmarking

Traditional RAG and search tools are measured solely by keyword retrieval accuracy. Graphenium, as an engineering containment linter, is benchmarked by a more critical engineering metric:

> **Tokens-to-Verifiable-Plan (TTVP)**

This guide outlines Graphenium's performance metrics, baseline scaling footprints, and benchmarking methodologies.

---

## 1. The TTVP Metric: Context Optimization

AI context windows are expensive in both API costs and model reasoning latency. When an agent is forced to recursively read raw source files to understand structural relationships (such as transitive call chains), it consumes tens of thousands of tokens before writing its first line of code.

Graphenium solves this by compressing your codebase's structure into a local AST index. Instead of reading files, the agent queries Graphenium's pre-computed boundaries via MCP. This replaces brute-force file reads with precise, token-efficient structural metadata.

```text
Brute-Force Workflow:
Agent reads 20 raw files (5,000 lines)  ──► Consumes ~30,000 tokens

Graphenium Workflow:
Agent queries local AST index           ──► Consumes ~1,200 tokens
Agent reads only 2 targeted files       ──► Consumes ~3,000 tokens
Total Context Saved                     ──► ~85% Reduction
```

Our documented baseline heuristic is **4 characters per token** for Graphenium's compact ASCII and JSON metadata outputs.

---

## 2. AST Index Scaling Baselines

Graphenium is evaluated against its own repository structure as a standard baseline:

| Metric | Baseline Value |
| :--- | :--- |
| **Files Indexed** | 41 source files |
| **AST Symbols (Nodes)** | 1,211 |
| **Structural Boundaries (Edges)** | 3,083 |
| **Cohesive Domains (Communities)** | 19 |
| **AST-Proven (`EXTRACTED`) Connections** | 1,328 (43%) |
| **Semantic (`INFERRED`) Connections** | 1,755 (57%) |
| **Average Query Payload Size** | 6,600 to 8,700 characters |
| **Approximate Query Token Cost** | 1,650 to 2,200 tokens |

---

## 3. Structural Index Performance Matrix

The following table documents execution latencies and payload weights for common pre-flight and diagnostic queries executed on our baseline index:

| Query Type | Command / Tool | Payload Weight (chars) | Approx. Tokens | Latency (ms) |
| :--- | :--- | ---: | ---: | ---: |
| **Transitive Impact Analysis** | `gm query "replace_file_extraction" --mode hybrid` | 8,674 | 2,170 | 27 ms |
| **Structural Domain Context** | `get_community(id: 1)` | 6,690 | 1,670 | 18 ms |
| **Module Dependency Mapping** | `module_dependencies("serve", "model")` | 8,395 | 2,100 | 22 ms |
| **Symbol Neighborhood Scope** | `analyse_symbol("node_data")` | 8,570 | 2,140 | 20 ms |
| **Declarative Transitive Check** | `gm query --datalog "?- calls_transitive('a', X)."` | 4,200 | 1,050 | 12 ms |
| **Datalog Layer Bypass Proof** | `gm query --datalog "?- bypasses_layer('a', 'b', 'c')."` | 1,200 | 300 | 8 ms |

*All benchmarks executed locally on a standard 8-core ARM workstation.*

---

## 4. Benchmarking Methodology

To run Graphenium's automated performance suite and compile an efficiency report:

```sh
# Ensure you are on a release-profile build for accurate latency testing
cargo build --release

# Generate a fresh baseline index
gm run . --no-semantic --no-viz

# Run the performance suite
chmod +x scripts/run_benchmarks.sh
./scripts/run_benchmarks.sh

# Export structured JSON performance metrics
./scripts/run_benchmarks.sh --json
```

---

## 5. What "Good" Looks Like: Target Bounds

When evaluating Graphenium's gating pipelines on a new repository, aim for the following target bounds:

| Metric | Healthy Bounds | Engineering Reason |
| :--- | :--- | :--- |
| **Query Payload Size** | Under 10,000 characters | Ensures metadata fits comfortably within agent context windows without crowding out reasoning space. |
| **Query Execution Latency** | Under 50 ms | Keeps MCP tool calls interactive, preventing background agent stalls. |
| **Datalog Solver Execution** | Under 100 ms (Step budget: 1,000) | Ensures transitive layering proofs complete instantly during pre-flight policy evaluations. |
| **AST Import Resolution Ratio** | Over 80% (`gm doctor --resolution`) | Indicates that Graphenium has successfully mapped the project's physical import boundaries. |

### Concerning Signals to Monitor:
*   **Payloads Exceeding 20,000 Characters:** Indicates the query keyword is too broad or the target module's dependency fan-out is too dense. Refine queries using path scoping (`--path-prefix`) or relation filters.
*   **Execution Latency Exceeding 500 ms:** Usually indicates Graphenium is parsing non-source directories (such as third-party packages or build folders). Ensure `.grapheniumignore` is properly configured.
*   **High Ambiguity Counts:** Indicates extensive identifier collisions (many identically named methods across different folders). Resolve by instructing the agent to target scoped qualified labels (`qualified_label`) instead of short names.

---

## 6. Topological Integrity and Compliance Scoring

Graphenium uses a 5-point quality scale to measure how effectively it prevents architectural drift during agentic edits:

| Score | Verification Quality | Meaning |
| :---: | :--- | :--- |
| **0** | **None** | The index was ignored; the agent wrote code blindly using unverified filesystem context. |
| **1** | **Keyword Match** | The agent queried name strings but did not verify dependencies or transitive paths. |
| **2** | **Direct Call Validation** | The agent verified direct callers and read targeted files, but bypassed pre-flight planning. |
| **3** | **Boundary Compliance** | The agent verified boundaries, read target files, and validated its design against local policies. |
| **4** | **Transitive Path Proof** | The agent resolved multi-hop paths, verified its plan pre-flight, and completed a scope-creep audit. |
| **5** | **Fully Contained Execution** | Pre-flight policy checks passed via Datalog, the physical code conformed strictly to the virtual plan, and post-edit verification verified no scope creep occurred. |

---

## 7. Graphenium vs. Traditional Tooling Matrix

The following matrix illustrates how Graphenium compares to standard codebase search and indexing tools during an active agentic workflow:

| Feature | Graphenium | `grep` / `ripgrep` | `ast-grep` | RAG Vector Indexes |
| :--- | :--- | :--- | :--- | :--- |
| **Primary Workflow Focus** | **Write-Safety & Gating** | Text Retrieval | Pattern Matching | Semantic Search |
| **Local-First Execution** | Yes (Tree-sitter) | Yes | Yes | Often Remote |
| **Transitive Path Solving** | **Yes (Datalog solver)** | No | No | No |
| **Virtual Planning specs** | **Yes (`plan_id` Workspaces)** | No | No | No |
| **PR Scope-Creep Audits** | **Yes (AST Diff Comparisons)** | No | No | No |
| **Boundary Gating Policies** | **Yes (Declarative JSON)** | No | No | No |
| **Token-Optimized Payloads** | **Yes (Symbol Compression)** | No | No | No |
