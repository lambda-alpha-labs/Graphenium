# Trust Benchmark Suite

This directory contains benchmark fixtures and tests for the trust and
resolution systems. Tests validate:
- Evidence span detection and staleness
- Resolution report accuracy
- Trust gate policies
- Safest-path traversal against known graph configurations

## Running

```sh
cargo test --lib trust
cargo test --lib harness
cargo test --lib policy
```
