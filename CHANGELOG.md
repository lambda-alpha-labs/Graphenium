# Changelog

All notable changes to Graphenium are documented in this file.

## [0.1.0] (unreleased)

### Added

- AST extraction for 9 languages via tree-sitter: Python, JavaScript,
  TypeScript, Rust, Go, Java, C, C++, C#
- MCP server with 13 graph tools: `query_graph`, `get_node`,
  `get_neighbors`, `get_community`, `god_nodes`, `graph_stats`,
  `shortest_path`, `architecture_summary`, `summarize_file`,
  `reload_graph`, `add_node`, `add_edge`, `remove_edge`
- Query CLI (`gm query`) with keyword-scored BFS/DFS traversal
- Louvain community detection and clustering
- AST-only extraction mode (no API key required)
- Semantic extraction mode with support for Anthropic, OpenAI,
  DeepSeek, OpenRouter, and OpenAI-compatible providers
- Watch mode (`gm watch`) with automatic rebuilds on file changes
- Incremental update support via mtime manifest
- Export to JSON and Markdown graph report
- Cross-file relationship inference
- God node and surprising connection detection
- Sensitive file detection and automatic skipping
- `.grapheniumignore` support for excluding directories
- Confidence model: EXTRACTED, INFERRED, AMBIGUOUS edge labelling
- Label disambiguation and qualified labels for duplicate symbol names
