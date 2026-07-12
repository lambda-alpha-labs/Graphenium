# Programmatic Integration: Graphenium Harness Adapter

Graphenium can be embedded directly inside an AI coding platform, developer workspace, or custom agent runner. By consuming Graphenium as a programmatic library, your platform can maintain a real-time, AST-proven index of the workspace and run strict, mechanical pre-flight checks as files are opened, edited, or saved.

---

## 1. Dependency Configuration

To keep compilation times fast and exclude the background MCP server and file-watching dependencies, build with Graphenium's lean `harness` feature flag. Declare the dependency in your `Cargo.toml` alongside your target language engines:

```toml
[dependencies]
graphenium = { git = "https://github.com/lambda-alpha-labs/Graphenium", default-features = false, features = ["harness", "lang-rust", "lang-python", "lang-csharp"] }
```

---

## 2. Core Library APIs

Graphenium exposes its indexing, patching, and verification engines through clean Rust structures. The key types reside in:
*   `graphenium::model` — Core data structures (`Node`, `Edge`, `GrapheniumGraph`, `ExtractionResult`, `FileType`, `Confidence`).
*   `graphenium::detect` — Workspace file detectors and file-type classifiers.
*   `graphenium::extract` — AST parsers and syntax extraction configurations.
*   `graphenium::build` — Assembly pipelines to merge AST metadata into GrapheniumGraph states.
*   `graphenium::cluster` — Domain partitioners (Louvain clustering).
*   `graphenium::policy` — Declarative policy modelers (`ArchRule`, `ArchPolicyConfig`).
*   `graphenium::harness` — Pre-flight policy solvers and post-facto compliance verifiers.

---

## 3. The Programmatic Event Loop

An embedded harness adapter typically maps code editing events to Graphenium's structural engine as follows:

```rust
use std::path::{Path, PathBuf};
use graphenium::{
    detect::{detect, DetectOptions},
    extract::{extract_all, extract_file, ExtractOptions},
    build::{build_merged},
    cluster::{cluster, ClusterOptions, CommunityStats},
    model::{GrapheniumGraph, Edge, Node, FileType, Confidence},
    export::json::to_json,
};

/// 1. Initialize Baseline Index (Run once when workspace opens)
pub fn initialize_index(root: &Path) -> Result<(GrapheniumGraph, Vec<CommunityStats>), String> {
    // Walk directory and classify file types (respecting ignores)
    let (files, _warnings) = detect(root, &DetectOptions::default())
        .map_err(|e| e.to_string())?;

    // Extract raw AST structure from all code files
    let ast_result = extract_all(&files, &ExtractOptions::default());

    // Compile physical AST symbols and boundaries
    let (mut graph, _stats) = build_merged([ast_result]);
    graph.set_ast_only(true);

    // Partition code into cohesive structural domains
    let domain_stats = cluster(&mut graph, &ClusterOptions::default());

    Ok((graph, domain_stats))
}

/// 2. Real-Time Incremental Patching (Run on file save/modify)
pub fn patch_file(graph: &mut GrapheniumGraph, file_path: &Path) -> Result<(), String> {
    let file = graphenium::detect::DetectedFile {
        path: file_path.to_path_buf(),
        file_type: FileType::Code,
    };

    // Re-extract only the modified file
    let result = extract_file(&file, &ExtractOptions::default());
    if result.is_empty() {
        return Ok(()); // Skip if empty or unsupported
    }

    let source_file = file_path.to_string_lossy().to_string();

    // Surgically replace stale AST contributions and re-resolve local boundaries
    graph.replace_file_extraction(&source_file, &result);

    Ok(())
}
```

---

## 4. Programmatic Gating and Policy Enforcements

Graphenium enables you to prevent architectural erosion by checking proposed agent designs pre-flight and verifying implemented code post-edit.

### Phase A: Pre-Flight Policy Gating
Before an agent writes any physical edits, require it to submit its change design as a virtual plan. Load your `.graphenium/policy.json` rules and run Graphenium's pre-flight logic solver:

```rust
use std::path::Path;

use graphenium::{
    analyze::delta::{evaluate_delta_gate, DeltaGateReport},
    harness::{validate_plan, validate_plan_preflight, PreFlightReport},
    policy::ArchPolicyConfig,
};

/// Full pre-flight orchestration: explicit policy rules + dynamic delta gating.
pub fn check_preflight_design(
    graph: &GrapheniumGraph,
    project_root: &Path,
    plan_id: &str,
) -> Result<PreFlightReport, String> {
    validate_plan(graph, plan_id, project_root)
}

/// Policy-only check (no delta gating).
pub fn check_policy_only(
    graph: &GrapheniumGraph,
    project_root: &Path,
    plan_id: &str,
) -> Result<PreFlightReport, String> {
    let policy_config = ArchPolicyConfig::load_for_project(project_root)
        .map_err(|e| e.to_string())?;
    Ok(validate_plan_preflight(graph, plan_id, &policy_config.rules))
}

/// Topological delta gate only.
pub fn check_delta_gate(
    graph: &GrapheniumGraph,
    plan_id: &str,
) -> Result<DeltaGateReport, graphenium::GrapheniumError> {
    evaluate_delta_gate(graph, plan_id, -0.02, 5.0)
}
```

*If `report.passes` is false, block execution. Feed `report.violations` back to the agent so it can re-plan its design.*

---

### Phase B: Post-Edit Compliance Auditing
After the agent implements its changes, compile the updated physical code and verify that its edits conform to the approved design plan:

```rust
use graphenium::harness::{verify_plan, PlanVerificationReport};

pub fn audit_post_edit_compliance(
    graph: &GrapheniumGraph,
    plan_id: &str,
) -> PlanVerificationReport {
    // Compares planned symbols to implemented code, detecting scope creep
    verify_plan(graph, plan_id)
}
```

If `report.passes_compliance` is false, Graphenium has detected scope creep (unplanned file modifications) or missing implementations. The build should be failed or blocked.

---

## 5. Serializing and Hot-Swapping the Index

If your platform runs Graphenium's engine inside a background daemon or parallel process, save updated index states atomically to disk. This allows background MCP servers to hot-swap their state without restarts:

```rust
pub fn persist_index_atomic(graph: &GrapheniumGraph, output_path: &Path) -> Result<(), String> {
    // Generate compact, un-spaced JSON
    let json = to_json(graph).map_err(|e| e.to_string())?;

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    // Write to a temporary file, then execute atomic rename
    let tmp = output_path.with_extension("json.tmp");
    std::fs::write(&tmp, &json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, output_path).map_err(|e| e.to_string())?;

    Ok(())
}
```

---

## 6. Manual Overrides and Corrections

If Graphenium's static AST extraction cannot automatically detect a dynamic, framework-injected, or runtime dependency, allow your platform's operators or agentic systems to write manual, AST-proven overrides:

```rust
/// Inject a verified, AST-proven boundary override into Graphenium's index
pub fn inject_manual_dependency(
    graph: &mut GrapheniumGraph,
    source_symbol: &str,
    target_symbol: &str,
    relation: &str,
    source_file: &str,
) -> bool {
    let edge = Edge::extracted(source_symbol, target_symbol, relation, source_file);

    // Returns false if source_symbol or target_symbol do not exist as physical nodes
    graph.add_edge(edge)
}
```