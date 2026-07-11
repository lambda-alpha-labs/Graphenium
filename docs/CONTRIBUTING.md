# Contributing

Thank you for contributing to Graphenium.

Graphenium is a Rust-native trust and verification layer for AI coding agents. The best contributions make agents safer, more accurate, more context-efficient, and more transparent when changing code.

## Quick start

```sh
git clone https://github.com/lambda-alpha-labs/Graphenium
cd Graphenium
cargo build
cargo test
cargo install --locked --path .
gm run . --no-semantic --no-viz
gm query "test" --budget 100
```

## High-impact contribution areas

| Area | Why it matters |
|---|---|
| Language extractors | Better source-backed coverage means safer agents |
| Cross-file resolution | More resolved relationships means fewer guesses |
| Trust reporting | Agents need confidence and provenance surfaced clearly |
| MCP tools | Better tool ergonomics improve agent behavior |
| Planning workspaces | More robust design-then-verify loops reduce risky edits |
| Benchmarks | Real tasks show whether Graphenium improves agent outcomes |
| Fixtures | Small test repos catch regressions and edge cases |
| Documentation | Better docs help teams adopt safe agent workflows |

## Module reference

| Module | Key concepts |
|---|---|
| `src/trust.rs` | EvidenceSpan, Claim, ResolutionReport |
| `src/harness.rs` | TrustCheckResult, pre-flight plan validation, post-facto plan verification |
| `src/policy.rs` | Trust quality policy evaluation and `ArchRule` / `ArchPolicyConfig` schema |
| `src/analyze/verifier.rs` | VerificationPlan and post-edit review planning |
| `src/analyze/surprise.rs` | Architectural anomaly scoring |
| `src/cluster/drift.rs` | DriftEvent and architecture drift detection |
| `src/extract/ci.rs` | CI config parsing and build/test targets |
| `src/extract/csharp_project.rs` | C# solution and project parsing |
| `src/resolver.rs` | Cross-file reference resolution |
| `src/telemetry.rs` | OpenTelemetry trace import and runtime overlay |
| `src/cache/query.rs` | Salsa-backed incremental computation |
| `src/embed.rs` | Text and structural embeddings |
| `src/ranking.rs` | Query modes and hybrid retrieval |
| `src/analyze/query.rs` | Datalog parser and interpreter |

## Adding a language extractor

1. Add the tree-sitter grammar crate to `Cargo.toml`.
2. Create a new extractor file in `src/extract/`.
3. Follow existing patterns such as Rust, Go, Python, or C# extractors.
4. Register the extractor in `src/extract/mod.rs`.
5. Add fixture files under the test fixtures directory.
6. Add tests for extracted nodes, edges, and confidence.
7. Update the language support table in the README.
8. Run tests, clippy, and formatting.

## Pull request checklist

Before submitting:

```sh
cargo test
cargo clippy --lib
cargo fmt
```

Your PR should:

- stay focused on one change
- include tests for new behavior
- include fixtures for extraction changes
- update documentation when behavior changes
- avoid weakening trust semantics
- explain impact on agent workflows

## Bug reports

Include:

- OS and architecture
- Rust version
- Graphenium version
- command run
- expected behavior
- actual behavior
- minimal repo or file when possible
- whether the issue involves detection, extraction, export, MCP, or trust gates

## Documentation contributions

Docs should prioritize safe agent behavior.

Good docs explain:

- when to query the graph
- how to interpret trust
- when to inspect source
- how to verify after editing
- where Graphenium has limitations

## Code of conduct

This project follows the Contributor Covenant. See `docs/CODE_OF_CONDUCT.md`.
