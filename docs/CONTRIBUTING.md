# Contributing to Graphenium

Thank you for contributing to Graphenium. 

Graphenium is a Rust-native structural containment layer and pre-flight linter for AI coding agents. The most valuable contributions improve Graphenium's ability to mechanically enforce engineering safety, protect module boundaries, and audit agent-generated PRs before they introduce codebase erosion.

---

## Quick Start

```sh
# Clone Graphenium
git clone https://github.com/lambda-alpha-labs/Graphenium
cd Graphenium

# Verify the test suite
cargo test

# Build and install the command-line interface
cargo install --locked --path .

# Initialize and generate a structural index on Graphenium itself
gm init
gm run . --no-semantic --no-viz

# Run a declarative linter check
gm query --datalog "?- is_orphan(X)."
```

---

## High-Impact Contribution Areas

We prioritize contributions that strengthen Graphenium's role as an external governance gate:

| Area | Why It Matters |
|---|---|
| **Language Extractors** | Enhancing Tree-sitter configurations increases Graphenium's AST-proven baseline coverage, reducing the surface area of semantic guesswork. |
| **Cross-File Resolution** | Refining Stack Graphs and identifier scoping decreases ambiguity, allowing agents to accurately trace dependency closures. |
| **Transitive Constraint Solving** | Adding standard library Datalog predicates (`stdlib.dl`) enables teams to define and enforce increasingly complex architectural patterns. |
| **Planning Workspaces** | Improving the design-then-verify loop prevents AI assistants from committing scope creep or modifying unapproved files. |
| **CI Gating & Tool Integration** | Standardizing pre-flight hooks and PR compliance audits makes it easier for engineering teams to adopt automated containment. |

---

## Module Reference Guide

Contributors working in the codebase should refer to the following structural module map:

| Module | Architectural Role | Key Concepts |
|---|---|---|
| `src/trust.rs` | Source-backed evidence tracking | `EvidenceSpan`, `Claim`, `ResolutionReport` |
| `src/harness.rs` | Pre-flight checks and post-edit compliance | `PreFlightReport`, `verify_plan`, compliance auditing |
| `src/policy.rs` | Declarative architecture boundaries | `ArchRule`, `ArchPolicyConfig` (layering rules, banned symbols) |
| `src/analyze/verifier.rs` | Post-edit verification planning | `VerificationPlan` (prioritized checklists for changed symbols) |
| `src/analyze/surprise.rs` | Architectural anomaly detection | `SurpriseEdge` (flagging peripheral-to-hub jumps, cross-boundary dependencies) |
| `src/cluster/drift.rs` | Structural drift warnings | `DriftEvent`, `detect_drift` (tracking boundary moves and split/merge changes) |
| `src/extract/ci.rs` | Verification target extraction | Parses GitHub Actions, Makefiles, and package configurations into job dependencies |
| `src/extract/csharp_project.rs` | Compiled boundary parsing | Solution (`.sln`) and project (`.csproj`) boundary parsing |
| `src/resolver.rs` | Deterministic cross-file binding | Binding raw import nodes to concrete AST targets across file boundaries |
| `src/telemetry.rs` | Live traffic overlays (Experimental) | OpenTelemetry integration to identify hot runtime execution paths |
| `src/cache/query.rs` | Memoized static extraction | Salsa-backed incremental extraction tracking |
| `src/ranking.rs` | HOT node discovery and filtering | Down-ranking framework/import noise, identifying namespace aggregation hubs |
| `src/analyze/query.rs` | Declarative Datalog query layer | Core parser and interpreter evaluating transitive query structures |

---

## Adding a Language Extractor

To extend Graphenium's AST-proven baseline to a new language:

1.  **Grammar Registration:** Add the target tree-sitter grammar crate to Graphenium's dependencies in `Cargo.toml`.
2.  **Configuration definition:** Create a configuration factory in `src/extract/config.rs` defining the node kinds representing class structures, function declarations, imports, and calls.
3.  **Extractor Registration:** Create the extractor module under `src/extract/` (following existing patterns like `src/extract/rust_lang.rs` or `src/extract/go.rs` if custom symbol groupings are required). Register the new extractor inside `src/extract/mod.rs`.
4.  **Fixture Creation:** Place minimal source fixtures inside Graphenium's test fixture folder to exercise the new parser.
5.  **Integration Testing:** Add extraction validation tests confirming that structural nodes, import edges, and local call boundaries are resolved with appropriate confidence scores.
6.  **CI Validation:** Ensure all tests pass, clippy is clean, and code is formatted before submission.

---

## Pull Request Checklist

Before submitting a pull request, run the following verification steps locally:

```sh
# Ensure the test suite passes cleanly
cargo test

# Validate clippy checks
cargo clippy --lib

# Confirm code formatting is standard
cargo fmt --all -- --check
```

### Pull Request Expectations:
*   **Highly Focused Scope:** Keep PRs isolated to a single feature or bug fix.
*   **Test-Backed:** Include integration tests and parser fixtures for any extraction changes.
*   **Documentation-Aligned:** Update relevant Markdown files under `docs/` if modifying CLI flags or user-facing policy schemas.
*   **Safety-First:** Avoid changes that compromise Graphenium's strict, compiler-level `EXTRACTED` trust guarantees.

---

## Bug Reports

If you encounter an issue, please file a report on GitHub with the following details:
1.  **Environment details:** OS, architecture, Rust version, Graphenium version (`gm --version`), and target language family.
2.  **Step-by-step reproduction:** Exact CLI command or MCP tool sequence that triggered the failure.
3.  **Config schema:** The contents of your local `.graphenium/policy.json` (if applicable).
4.  **Logs/Errors:** Raw CLI or compiler output logs illustrating the failure.
5.  **Minimal Example:** A small, reproducible snippet of the codebase files that triggered the parser or solver anomaly.

---

## Documentation Guidelines

Documentation must prioritize safe, deterministic agent behavior. 
*   Avoid describing Graphenium as a "visual codebase map" or "AI search engine." 
*   Explain features in terms of **external governance, structural verification, pre-flight linting, and compile-time boundaries**.
*   Write guides that teach users how to construct strict engineering policy contracts rather than passive query workflows.
