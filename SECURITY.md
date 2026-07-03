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

## Academic paper classification

Graphenium's file classifier (`src/detect/paper.rs`) can detect academic
papers in the repository by scanning for scholarly markers (arXiv IDs,
DOIs, LaTeX citations, proceedings indicators). Papers classified this
way are linked into the graph as `FileType::Paper` nodes alongside source
code. These nodes follow the same sensitive-file exclusion rules: if a
paper file matches a pattern in `sensitive.rs`, it is skipped. Papers are
treated as documentation assets, not source code, and are never submitted
to any remote service during AST-only scans.

## Scope

Graphenium is a local tool. It does not send your source code to any
remote service unless you explicitly configure semantic extraction with
an API key and provider. The AST-only pipeline runs entirely on your
machine.
