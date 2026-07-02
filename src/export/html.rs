/// HTML export: wraps the graph JSON in a self-contained vis.js viewer.
use crate::model::graph::GrapheniumGraph;

use super::{html_template::HTML_TEMPLATE, json};

// ── Public API ────────────────────────────────────────────────────────────────

/// Render the graph as a self-contained HTML page.
/// The compact graph JSON is injected into the template at the
/// `{{GRAPH_DATA}}` placeholder; `title` replaces `{{TITLE}}`.
pub fn to_html(graph: &GrapheniumGraph, title: &str) -> crate::Result<String> {
    let json_str = json::to_json_compact(graph)?;
    let html = HTML_TEMPLATE
        .replace("{{TITLE}}", title)
        .replace("{{GRAPH_DATA}}", &json_str);
    Ok(html)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Edge, FileType, Node};

    fn simple_graph() -> GrapheniumGraph {
        let mut g = GrapheniumGraph::new();
        let mut n = Node::new("a_foo", "Foo", FileType::Code, "a.rs");
        n.community = Some(0);
        g.upsert_node(n);
        g.upsert_node(Node::new("a_bar", "Bar", FileType::Code, "a.rs"));
        g.add_edge(Edge::extracted("a_foo", "a_bar", "calls", "a.rs"));
        g
    }

    #[test]
    fn html_contains_title() {
        let g = simple_graph();
        let html = to_html(&g, "My Graph").unwrap();
        assert!(html.contains("My Graph"));
    }

    #[test]
    fn html_contains_graph_data() {
        let g = simple_graph();
        let html = to_html(&g, "Test").unwrap();
        assert!(html.contains("a_foo"));
        assert!(html.contains("a_bar"));
        assert!(html.contains("EXTRACTED"));
    }

    #[test]
    fn no_unresolved_placeholders() {
        let g = simple_graph();
        let html = to_html(&g, "Test").unwrap();
        assert!(!html.contains("{{TITLE}}"));
        assert!(!html.contains("{{GRAPH_DATA}}"));
    }

    #[test]
    fn empty_graph_renders() {
        let g = GrapheniumGraph::new();
        let html = to_html(&g, "Empty").unwrap();
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(!html.contains("{{"));
    }

    #[test]
    fn html_is_well_formed_skeleton() {
        let g = simple_graph();
        let html = to_html(&g, "T").unwrap();
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("</html>"));
        assert!(html.contains("vis-network"));
    }
}
