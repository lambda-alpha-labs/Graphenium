# Continuous Architectural Containment and Governance

Traditional continuous integration (CI) environments verify syntax, type safety, and runtime behavior:
*   *Did compilation succeed?*
*   *Did tests pass?*
*   *Did code formatters and style linters succeed?*

For AI-generated changes, these checks are necessary but fundamentally insufficient. Agents can write code that passes tests while quietly eroding your architecture—bypassing module boundaries, introducing transitive circular dependencies, or modifying unplanned files. 

Graphenium integrates into your CI/CD pipeline and git hooks as an **external structural gate**, mechanically verifying the architectural integrity of agent-authored PRs before they reach a human reviewer.

---

## 1. The Automated Gating & Verification Loop

To prevent vibe-coding in active repositories, Graphenium enforces a structured, four-phase containment lifecycle:

```text
1. DESIGN PHASE (Pre-Flight)
   └── Agent declares virtual plan ──► Verified via Datalog policy solver

2. IMPLEMENTATION PHASE (Write)
   └── Agent writes physical code ──► Bounded within declared file scope

3. AUDIT PHASE (Post-Edit)
   └── gm check --plan             ──► Verifies actual changes match virtual spec

4. REVIEW PHASE (Merge)
   └── Risk-sorted PR comment      ──► Summarizes transitive impact & test plan
```

---

## 2. Pre-Flight Architectural Policies

Architectural constraints are declared in `.graphenium/policy.json` at the root of the repository. If present, Graphenium's pre-flight engine runs these rules against the agent's proposed plan *before* any physical files are modified.

### Policy Configuration Schema (`.graphenium/policy.json`):
```json
{
  "rules": [
    {
      "type": "forbidden_dependency",
      "from_pattern": "src/controllers/**",
      "to_pattern": "src/db/**",
      "reason": "Controllers must use services, not access DB directly"
    },
    {
      "type": "strict_layering",
      "layers": [
        "src/serve/**",
        "src/analyze/**",
        "src/extract/**",
        "src/model/**"
      ],
      "reason": "Respect Graphenium's tiered architecture: serve -> analyze -> extract -> model"
    },
    {
      "type": "banned_symbol",
      "symbol_label": "LegacyRawSql",
      "reason": "Use the repository abstraction instead"
    }
  ]
}
```

### Supported Rule Types:
*   **`forbidden_dependency`:** Blocks any direct import or call from files matching `from_pattern` to files matching `to_pattern` (supports standard glob wildcards).
*   **`strict_layering`:** Enforces a rigid top-down hierarchy. No code in `layer[i]` may depend on `layer[j]` where `j < i`. Graphenium runs its local Datalog solver to prove transitive, multi-hop bypasses (`bypasses_layer`).
*   **`banned_symbol`:** Rejects any design plan that attempts to introduce, modify, or reference a restricted identifier (e.g., deprecated utilities, legacy database connectors).

---

## 3. When Policy Gating Runs

Graphenium enforces these policies at multiple checkpoints in the development lifecycle:

| Gate Entry Point | Execution Context | Gating Behavior |
|---|---|---|
| **`validate_plan` (MCP)** | Pre-Flight (Agent Session) | Runs explicit pre-flight checks on a proposed planning workspace. |
| **`add_planned_symbol`** | Pre-Flight (Agent Session) | Automatic pre-flight hook. If the proposed class/method violates policy, Graphenium returns `PRE_FLIGHT_VIOLATION` and blocks the plan. |
| **`gm check --plan <id>`** | CI / Pre-Commit Hook | Double Gate: Evaluates pre-flight design compliance first, then performs a post-facto scope-creep audit. |
| **`agent_change_gate`** | CI / PR Pipeline | Evaluates global trust quality metrics (import resolution ratio, maximum allowed ambiguity). |

---

## 4. Baseline Index Verification Gate

To ensure your codebase index remains healthy and that your agent is not operating on incomplete static analysis data, run Graphenium's baseline quality gate in CI:

```sh
gm check --graph graphenium-out/graph.json --min-resolution 80 --max-ambiguous 10
```

### Evaluation Parameters:
*   `--min-resolution <pct>`: Fails the build if the AST import resolution ratio drops below the target percentage (default: 80%).
*   `--max-ambiguous <count>`: Fails the build if Graphenium detects more than `<count>` unresolved name collisions (default: 10).
*   `--strict`: Exits non-zero on any parser or configuration warnings.

---

## 5. Incremental Diff Audits

To audit a completed PR, generate a baseline index snapshot before the agent starts its task:

```sh
gm snapshot create before-change
```

After the agent completes its edits, compile the updated index and run an impact-aware structural diff:

```sh
gm run . --update --no-semantic --no-viz
gm diff --before graphenium-snapshots/before-change.json --after graphenium-out/graph.json --impact --review-plan
```

This generates a structured, risk-sorted review plan prioritizing removed public symbols, circular dependencies, and cross-boundary coupling.

---

## 6. Pull Request Review Template

Configure your agent or CI pipeline to append Graphenium's structural audit to every pull request description:

```text
### Graphenium Structural Containment Audit

- [ ] **Pre-Flight Policy Check:** PASSED / FAILED (policy.json rules)
- [ ] **Scope-Creep Verification:** PASSED / FAILED (unplanned file modifications)

#### Codebase Modifications
*   **Added Symbols:** `[list of added classes/methods]`
*   **Removed Symbols:** `[list of removed classes/methods]`
*   **Transitive Downstream Impact:** `[count] affected callers`
*   **High-Risk Callers:** `[list of high-degree dependent classes]`

#### Automated Verification Plan
1. **Must-Read Files:** `[list of modified or affected source files]`
2. **Covering Test Targets:** `[list of associated test files to execute]`
3. **Ambiguity Review:** `[list of unresolved identifier collisions to inspect]`
```

---

## 7. GitHub Actions Workflow Integration

Incorporate Graphenium into your standard pull request pipeline using the following workflow configuration:

```yaml
name: Graphenium Structural Gate

on:
  pull_request:
    branches: [main]

jobs:
  graphenium-gate:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout Code
        uses: actions/checkout@v4

      - name: Setup Rust Toolchain
        uses: dtolnay/rust-toolchain@stable

      - name: Install Graphenium CLI
        run: cargo install --locked --path .

      - name: Initialize Workspace
        run: gm init

      - name: Compile Codebase Index
        run: gm run . --no-semantic --no-viz

      - name: Enforce Resolution and Policy Gates
        run: |
          gm check --graph graphenium-out/graph.json --min-resolution 80 --max-ambiguous 5 --strict
```

---

## 8. What Graphenium Gating Does Not Replace

Graphenium is designed to enforce structural and architectural boundaries. It is a complementary containment layer and does not replace:
*   **Unit & Integration Tests:** Graphenium verifies *decoupling and structure*; tests verify *behavior and state*.
*   **Static Security Scanners (SAST):** Graphenium blocks *architectural drift*; SAST scanners block *vulnerabilities*.
*   **Human Code Review:** Graphenium generates a *prioritized verification plan*; humans execute *contextual design review*.