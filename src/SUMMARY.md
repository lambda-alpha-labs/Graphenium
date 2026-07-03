# Graphenium Implementation Summary (v0.14.0 → v0.16.0)

## Releases

| Version | Tag | Commit | Status |
|---|---|---|---|
| v0.14.0 | v0.14.0 | bd62970 | Released on GitHub |
| v0.15.0 | v0.15.0 | 857a7d6 | Not released |
| v0.16.0 | v0.16.0 | 1b241d3 | Not released |

## C# Support (v0.14.0)

**Files created/modified:**
- `src/extract/csharp_project.rs` — parses `.sln` and `.csproj` files (`CSharpWorkspace`, `CSharpProject`)
- `src/extract/ci.rs` — `csproj_to_extraction()` injects project boundary nodes + `depends_on` edges
- `src/main.rs` — C# project scanning wired into `cmd_run` build pipeline and `cmd_graph_build_map`
- `src/extract/mod.rs` — C# extraction regression test `extract_csharp_file`

## Enhancements a-l (v0.15.0)

**Phase 1 (a–d):** Commit 857a7d6
- **a)** Parser failure logger — `src/extract/walker.rs`, `rust_lang.rs`, `go.rs` emit `eprintln!` on parser/set_language failures
- **b)** C# extraction test — `src/extract/mod.rs` (lines 177–203) verifies class+method extraction in namespace context
- **c)** Language integrity check — `src/doctor.rs` function `check_language_extraction_integrity()` cross-references detected languages vs extracted symbols
- **d)** `Cargo.toml`: `strip = "debuginfo"` (not `true`), `tree-sitter-c-sharp` pinned to `0.23.1`

**Phase 2–3 (g, f, i, h):** Same commit
- **g)** `src/ranking.rs`: Python stdlib modules (os, sys, ast, unittest, asyncio, etc.) added to `FRAMEWORK_LABELS`. `src/detect/mod.rs`: Windows backslash→forward-slash normalization in `.grapheniumignore` matching
- **f)** `src/serve/handlers.rs`: `is_node_in_module()` checks path, qualified_label, and label. `module_dependencies()` now supports both path and namespace matching plus `depends_on` edges
- **i)** `src/serve/handlers.rs`: `resolve_symbols_to_ids()` fuzzy multi-symbol resolver in helpers section (moved out of `#[tool(tool_box)]`)
- **h)** `src/serve/handlers.rs`: `blast_radius()` now shows AST-only safety warning when no call graph exists

**Enhancements (j, e):** Commit b9176ca
- **j)** `src/serve/traversal.rs`: `find_structural_references()` returns containers/imports/inheritance. MCP tool `references_to` in handlers.rs
- **e)** `src/analyze/impact.rs`: `format_safe_diff()` truncates diffs >500 changes, prioritizing removed > community moves > additions

## Remaining Work Plan (v0.16.0)

All 6 phases implemented in commit 1b241d3:

**Phase 1:** `plan_id: Option<String>` on `Node` and `Edge`. Plan nodes filtered out in `filtered_node_ids` 
**Phase 2:** `SubsystemExplanation` struct, `explain_subsystem()`, `format_explanation_report()` with 5-section Markdown (hierarchy, community, callers, production files, test files) + budget truncation
**Phase 3:** `PlanVerificationReport` struct, `verify_plan()` checks implemented vs missing nodes + unplanned file changes
**Phase 4:** 4 MCP tools: `explain_change`, `create_planning_workspace`, `add_planned_symbol`, `get_plan_details`
**Phase 5:** CLI: `gm explain <symbol>`, `gm check --plan <id>` with `dispatch_commands`
**Phase 6:** `test_planning_workspace_lifecycle` integration test

## Other Notable Changes

- `src/cache/manager.rs` — `CacheManager` with `new()`, `load_ast()/save_ast()`, `load_semantic()/save_semantic()`, atomic temp-then-rename writes. 4 unit tests
- `src/watch.rs` — `manifest_path` fixed from `graph.manifest.json` to `manifest.json`, `extract_all` call updated with `ExtractOptions`, `read_cache_dir` removed
- `src/serve/handlers.rs` — `graph.graph` changed to `graph.as_ref()`, `read_dir(&path)` (not `read_dir(path)`), `resolve_symbols_to_ids` and `is_node_in_module` moved to constructor section
- All helper functions moved OUTSIDE `#[tool(tool_box)]` blocks — regular `fn` impl items, not MCP tools

## Key Decisions

1. Helper functions (`is_node_in_module`, `resolve_symbols_to_ids`) moved OUT of `#[tool(tool_box)]` into the constructor/helpers `impl GrapheniumServer` block (regular `fn`s, not MCP tools). The MCP tool attributes (`#[tool(description)], #[tool(param)]`) are ONLY valid on functions intended as MCP tools.

2. `plan_id` filtering done inline in `filtered_node_ids` (no new parameter) to avoid breaking all 15+ call sites. Plan nodes are silently filtered out of all standard queries by default.

3. `format_safe_diff` uses a 500-change budget limit, sampled from removed > community > added priority.

4. `CSharpWorkspace` and `CSharpProject` are the actual struct names (not `CSharpProjectInfo` as the original plan described).
