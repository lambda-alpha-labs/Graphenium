# Security Policy

Graphenium is designed to govern and inspect source code in local developer environments and CI/CD pipelines. Because it acts as an external structural gate for AI agents, Graphenium enforces a strict, local-first security model to protect proprietary enterprise intellectual property (IP).

---

## 1. Local-First IP Guarantee

Graphenium's core AST-parsing and Stack Graphs resolution pipelines run **100% offline on your local machine**. 
*   **Zero Network Leakage:** By default, Graphenium does not send your source code, file structures, or metadata to any remote service.
*   **Free and Offline:** No API keys, subscriptions, or network connections are required to parse your codebase, run Datalog linter rules, or enforce pre-flight architectural policies.

---

## 2. Semantic Extraction Data Risks

Source code is only transmitted to external networks when you explicitly enable the **optional semantic pass** (`gm run` without the `--no-semantic` flag) and configure a remote provider (e.g., Anthropic, OpenAI, DeepSeek) with an active API key.

When the semantic pass is active:
*   Small snippets of source files, documents, and images are transmitted to the configured LLM endpoint to infer high-level conceptual relationships (such as cross-file delegation patterns).
*   **Mitigation Strategy:** If you are operating in a regulated or highly secure corporate environment, you should enforce AST-only execution by default:
    ```sh
    gm run . --no-semantic --no-viz
    ```
*   Ensure your team's `.grapheniumignore` is configured strictly before enabling any semantic extraction.

---

## 3. Automated Sensitive File Exclusion

Graphenium implements automatic, pattern-based exclusions to prevent parsing or indexing files that contain credentials, keys, or secrets. 

The sensitive-file detector is maintained locally in:
```text
src/detect/sensitive.rs
```

Graphenium will automatically skip files matching patterns such as:
*   `.env` and `.env.*` files.
*   Private key files (`.pem`, `.key`, `id_rsa`, `id_ed25519`).
*   Configured credential stores (`.aws/credentials`, `.netrc`, `.pgpass`).
*   Cloud service account descriptors (`service_account.json`).
*   Files prefixed with `secret`, `credential`, `password`, or `token`.

If you discover a sensitive filename pattern that is not covered by default, please open a pull request updating the `SENSITIVE_PATTERNS` registry in `src/detect/sensitive.rs` along with a corresponding unit test.

---

## 4. Academic Paper Classification

Graphenium can classify Markdown and plain-text files as academic papers by scanning for scholarly markers (such as arXiv identifiers, DOIs, and LaTeX citation blocks). 

These classified files are treated as passive documentation assets, not executable source code. The same sensitive-file pattern rules apply: if an academic paper file matches a sensitive pattern, Graphenium will skip indexing it entirely.

---

## 5. Security Best Practices for Teams

To maintain structural integrity without exposing sensitive metadata, observe the following rules:

1.  **Exclude Build Outputs and Dependencies:** Ensure your `.grapheniumignore` blocks all compiled artifacts, lockfiles, and third-party directories (such as `node_modules/`, `target/`, `.venv/`, `dist/`). This prevents Graphenium from wasting memory indexing untrusted library code.
2.  **Audit the Index Artifact:** The generated index file (`graphenium-out/graph.json`) contains the structural blueprint of your software. Do not publish or commit this artifact to public repositories if you do not want to disclose your system's private module layouts.
3.  **Validate Environment Health:** Run `gm doctor` to confirm that sensitive files are excluded and that Graphenium is operating on a clean, localized AST index.
4.  **Sandbox Agent Execution:** Since MCP allows agents to execute queries, run Graphenium within the same secure sandboxed environment where you run your compiler and test suite.

---

## 6. What to Cover in Security Reviews

When auditing Graphenium for corporate or enterprise deployment, ensure your review covers the following:

1.  **Is semantic extraction disabled or approved?** Verify that CI/CD runs are hardcoded to `--no-semantic`.
2.  **Is Graphenium's local-first execution verified?** Confirm that firewall rules block outward telemetry from local `gm` execution paths.
3.  **Is `.grapheniumignore` configured correctly?** Audit the ignore patterns to ensure no secret vaults or raw logs are swept into the index.
4.  **Who has access to Graphenium's MCP server?** Ensure the local stdin/stdout port used by Graphenium is strictly bounded to the user's localized agent process.

---

## 7. Reporting a Vulnerability

Please do not open public GitHub issues for sensitive security reports.

To report a vulnerability privately, email our security team at: **security@graphenium.dev**

In your report, please provide:
*   A description of the vulnerability and its potential impact (e.g., source metadata exposure, missing sensitive pattern matches).
*   Step-by-step instructions to reproduce the behavior.
*   Your Graphenium compilation details (`gm --version`).