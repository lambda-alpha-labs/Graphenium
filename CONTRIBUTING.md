# Contributing to Graphenium

Thanks for your interest in contributing. Graphenium is a Rust-native
structural memory engine for AI coding agents, and contributions that
make it better at that job are welcome.

## Quick start

```sh
# Clone and build
git clone https://github.com/lambda-alpha-labs/Graphenium
cd Graphenium
cargo build

# Run the tests
cargo test

# Install the binary
cargo install --path .

# Generate a graph
gm run . --no-semantic --no-viz

# Query it
gm query "test" --budget 100
```

## What to work on

- **Good first issues** are tagged in the issue tracker. Start there.
- **Language extractors**: adding support for a new language via
  tree-sitter is one of the highest-impact contributions. See existing
  extractors in `src/extract/` for patterns to follow.
- **MCP integrations**: docs and setup helpers for new AI tools.
- **Fixtures**: small, well-understood test repos that exercise
  specific extraction or clustering behaviour.

## Module reference

Key modules and their primary types:

- `src/trust.rs` — EvidenceSpan, Claim, ResolutionReport
- `src/harness.rs` — TrustCheckResult, check_resolution_quality
- `src/policy.rs` — Policy enum, evaluate_policies
- `src/analyze/verifier.rs` — VerificationPlan, plan_verification
- `src/cluster/drift.rs` — DriftEvent, detect_drift
- `src/extract/ci.rs` — CI config parsing, CiTarget
- `src/resolver.rs` — Cross-file reference resolution (Stack Graphs), `CrossFileReference`
- `src/telemetry.rs` — OpenTelemetry trace import, `RuntimeOverlay`, hot-path analysis
- `src/cache/query.rs` — Salsa-based incremental computation engine
- `src/embed.rs` — TF text embeddings and Node2Vec structural embeddings
- `src/ranking.rs` — `QueryMode` enum, hybrid retrieval scoring
- `src/analyze/query.rs` — Datalog query engine with tokenizer, parser, interpreter

## Adding a language extractor

1. Add the tree-sitter grammar crate to `Cargo.toml`.
2. Create a new file in `src/extract/` following the pattern of
   `rust_lang.rs` or `go.rs`.
3. Register it in `src/extract/mod.rs`.
4. Add tests with small fixture files under `test/fixtures/`.
5. Update the language support table in `README.md`.

## Opening an issue

Before opening a bug report, check if there's already an existing
issue. Include:

- Your OS and architecture
- Rust version (`rustc --version`)
- Steps to reproduce
- Expected vs actual behaviour
- If relevant, a minimal repo or file that triggers the issue

## Submitting a PR

- Keep PRs focused. One change per PR.
- Run `cargo test` and `cargo fmt` before submitting.
- If adding a feature, include tests.
- If changing extraction behaviour, include a fixture that demonstrates
  the change.

## Code of Conduct

This project follows the [Contributor Covenant](CODE_OF_CONDUCT.md).
