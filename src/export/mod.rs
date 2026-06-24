pub mod html;
pub mod html_template;
pub mod json;

use std::fs;
use std::path::{Path, PathBuf};

use crate::model::graph::GrapheniumGraph;

// ── Public types ───────────────────────────────────────────────────────────────

/// Paths of the files written by [`export`].
#[derive(Debug, Clone)]
pub struct ExportPaths {
    pub json: PathBuf,
    pub html: PathBuf,
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Write `graph.json` and `graph.html` to `out_dir`.
///
/// `out_dir` is created if it does not exist.
pub fn export(graph: &GrapheniumGraph, out_dir: &Path, title: &str) -> crate::Result<ExportPaths> {
    fs::create_dir_all(out_dir)?;

    let json_path = out_dir.join("graph.json");
    fs::write(&json_path, json::to_json(graph)?)?;

    let html_path = out_dir.join("graph.html");
    fs::write(&html_path, html::to_html(graph, title)?)?;

    Ok(ExportPaths {
        json: json_path,
        html: html_path,
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Edge, FileType, Node};
    use tempfile::TempDir;

    fn simple_graph() -> GrapheniumGraph {
        let mut g = GrapheniumGraph::new();
        g.upsert_node(Node::new("a", "A", FileType::Code, "a.rs"));
        g.upsert_node(Node::new("b", "B", FileType::Code, "b.rs"));
        g.add_edge(Edge::extracted("a", "b", "calls", "a.rs"));
        g
    }

    #[test]
    fn export_creates_both_files() {
        let tmp = TempDir::new().unwrap();
        let g = simple_graph();
        let paths = export(&g, tmp.path(), "Test Graph").unwrap();
        assert!(paths.json.exists());
        assert!(paths.html.exists());
    }

    #[test]
    fn json_file_is_valid_json() {
        let tmp = TempDir::new().unwrap();
        let g = simple_graph();
        let paths = export(&g, tmp.path(), "T").unwrap();
        let content = std::fs::read_to_string(paths.json).unwrap();
        assert!(serde_json::from_str::<serde_json::Value>(&content).is_ok());
    }

    #[test]
    fn html_file_contains_title() {
        let tmp = TempDir::new().unwrap();
        let g = simple_graph();
        let paths = export(&g, tmp.path(), "My Title").unwrap();
        let content = std::fs::read_to_string(paths.html).unwrap();
        assert!(content.contains("My Title"));
    }

    #[test]
    fn creates_out_dir_if_missing() {
        let tmp = TempDir::new().unwrap();
        let out = tmp.path().join("nested").join("output");
        let g = simple_graph();
        export(&g, &out, "T").unwrap();
        assert!(out.exists());
    }
}
