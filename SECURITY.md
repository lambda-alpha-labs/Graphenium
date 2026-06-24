# Security Policy

## Reporting a vulnerability

Graphenium processes source code from your repositories. If you discover
a case where Graphenium mishandles sensitive files (for example, failing
to skip files that should be excluded, or leaking file contents in error
messages or graph output), please report it.

**Do not open a public issue.** Instead, email the details to the
maintainer. Include:

- A description of the behaviour
- Steps to reproduce
- The Graphenium version (`gm --version`)
- Whether the issue is in file detection, extraction, export, or the
  MCP server

## Sensitive file detection

Graphenium automatically skips files whose names match known sensitive
patterns (e.g. files containing "token", "secret", "private_key").
This list is maintained in `src/detect/sensitive.rs`.

If you find a filename pattern that should be skipped by default, please
contribute a PR to `src/detect/sensitive.rs` with the pattern and a test.

## Supported versions

Only the latest release receives security patches. We recommend running
the most recent version.

## Scope

Graphenium is a local tool. It does not send your source code to any
remote service unless you explicitly configure semantic extraction with
an API key and provider. The AST-only pipeline runs entirely on your
machine.
