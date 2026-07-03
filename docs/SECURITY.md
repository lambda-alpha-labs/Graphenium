# Security Policy

Graphenium processes source code from user repositories. Its default operating model is local-first.

## Local-first guarantee

The AST-only pipeline runs locally. It does not send source code to a remote service.

Source code is sent to a remote provider only when the user explicitly configures semantic extraction with an API key and provider.

## Sensitive file handling

Graphenium skips files whose names match known sensitive patterns, such as names containing token, secret, or private key signals.

Sensitive-file detection is maintained in:

```text
src/detect/sensitive.rs
```

If a filename pattern should be skipped by default, add the pattern and a test.

## Reporting a vulnerability

Do not open a public issue for sensitive security reports.

Report privately to the maintainer with:

- description of the behavior
- steps to reproduce
- Graphenium version from `gm --version`
- whether the issue is in file detection, extraction, export, or MCP server behavior
- whether source content or secret material could be exposed

## Supported versions

Only the latest release receives security patches. Users should run the most recent version.

## Academic paper classification

Graphenium can classify academic papers in repositories by scanning for scholarly markers. These files are treated as documentation assets, not source code.

The same sensitive-file rules apply. A paper file that matches sensitive patterns is skipped.

## Semantic extraction risk

When semantic extraction is enabled, source excerpts may be sent to the configured provider.

Teams should:

- confirm provider policy before use
- avoid semantic extraction for sensitive repositories unless approved
- prefer AST-only mode for regulated code
- review `.grapheniumignore` before enabling semantic extraction

## Security best practices

- Start with `gm run . --no-semantic --no-viz`.
- Keep `.grapheniumignore` strict.
- Exclude secrets, generated credentials, build outputs, and vendored code.
- Run `gm doctor` before trusting output.
- Do not publish `graphenium-out` artifacts if they reveal sensitive structure.
- Treat manually added graph edges as auditable records.
- Review MCP access in shared environments.

## What to include in security reviews

1. What files are scanned?
2. What files are ignored?
3. Is semantic extraction disabled or approved?
4. Is the graph artifact safe to store?
5. Who can access the MCP server?
6. Are sensitive filenames correctly excluded?
7. Are logs free of source or secret leakage?

## Scope

Graphenium is a local developer tool and MCP server. Security issues include, but are not limited to:

- sensitive files not being skipped
- source code leaking in errors or logs
- graph output exposing secrets
- MCP server exposing unintended files or metadata
- semantic extraction sending source without clear user action
