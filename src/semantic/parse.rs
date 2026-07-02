/// Multi-layer JSON extraction from Claude's response text.
/// Parsing layers (tried in order):
///   1. Direct `serde_json::from_str` on the full text.
///   2. Extract the first ```` ```json … ``` ```` (or ```` ``` … ``` ````) block.
///   3. Slice from the first `{` to the last `}` and parse that substring.
/// After extracting a raw JSON value, each node, edge and hyperedge is
/// validated individually — items that fail are silently dropped, keeping all
/// valid portions of a partially-correct response.
use serde::Deserialize;

use crate::model::{Confidence, Edge, ExtractionResult, FileType, HyperEdge, Node};

// ── Internal raw types ────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
struct RawExtraction {
    #[serde(default)]
    nodes: Vec<serde_json::Value>,
    #[serde(default)]
    edges: Vec<serde_json::Value>,
    #[serde(default)]
    hyperedges: Vec<serde_json::Value>,
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Parse an `ExtractionResult` out of `text`, trying multiple strategies.
/// Returns an empty result if all layers fail.
pub fn parse_extraction(text: &str) -> ExtractionResult {
    build_result(extract_json(text))
}

// ── Layer selection ───────────────────────────────────────────────────────────

fn extract_json(text: &str) -> RawExtraction {
    // Layer 1: direct parse
    if let Ok(r) = serde_json::from_str::<RawExtraction>(text) {
        return r;
    }

    // Layer 2: code block
    if let Some(s) = extract_code_block(text) {
        if let Ok(r) = serde_json::from_str::<RawExtraction>(s) {
            return r;
        }
    }

    // Layer 3: brace extraction
    if let Some(s) = extract_braces(text) {
        if let Ok(r) = serde_json::from_str::<RawExtraction>(s) {
            return r;
        }
    }

    RawExtraction::default()
}

/// Extract the content of the first ` ```json ... ``` ` or ` ``` ... ``` ` block.
fn extract_code_block(text: &str) -> Option<&str> {
    let start = text.find("```")?;
    let after = &text[start + 3..];
    // Skip optional language tag (e.g. "json\n")
    let body_start = after.find('\n').map(|i| i + 1).unwrap_or(0);
    let body = &after[body_start..];
    let end = body.find("```")?;
    Some(body[..end].trim())
}

/// Slice from the first `{` to the last `}` (inclusive).
fn extract_braces(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end >= start {
        Some(&text[start..=end])
    } else {
        None
    }
}

// ── Item builders ─────────────────────────────────────────────────────────────

fn build_result(raw: RawExtraction) -> ExtractionResult {
    let mut result = ExtractionResult::new();
    for v in raw.nodes {
        if let Some(n) = build_node(&v) {
            result.nodes.push(n);
        }
    }
    for v in raw.edges {
        if let Some(e) = build_edge(&v) {
            result.edges.push(e);
        }
    }
    for v in raw.hyperedges {
        if let Some(h) = build_hyperedge(&v) {
            result.hyperedges.push(h);
        }
    }
    result
}

fn str_field(v: &serde_json::Value, key: &str) -> Option<String> {
    v[key].as_str().map(str::to_owned)
}

fn build_node(v: &serde_json::Value) -> Option<Node> {
    let id = str_field(v, "id").filter(|s| !s.is_empty())?;
    let label = str_field(v, "label").unwrap_or_else(|| id.clone());
    let source_file = str_field(v, "source_file").unwrap_or_default();
    let source_location = str_field(v, "source_location").unwrap_or_default();
    let file_type = parse_file_type(v["file_type"].as_str().unwrap_or("code"));
    let mut node =
        Node::new(id, label, file_type, source_file).with_source_location(source_location);
    // LLM-generated nodes get provenance
    node.extractor = Some("llm".to_string());
    node.resolution_status = Some("inferred".to_string());
    Some(node)
}

fn build_edge(v: &serde_json::Value) -> Option<Edge> {
    let source = str_field(v, "source").filter(|s| !s.is_empty())?;
    let target = str_field(v, "target").filter(|s| !s.is_empty())?;
    let relation = str_field(v, "relation").unwrap_or_else(|| "uses".to_string());
    let source_file = str_field(v, "source_file").unwrap_or_default();
    let confidence = parse_confidence(v["confidence"].as_str().unwrap_or("INFERRED"));
    let confidence_score = v["confidence_score"]
        .as_f64()
        .map(|s| s.clamp(0.0, 1.0))
        .unwrap_or_else(|| confidence.default_score());

    let mut edge = Edge::new(source, target, relation, confidence, source_file);
    edge.confidence_score = confidence_score;
    // LLM-generated edges get provenance based on their confidence level
    edge.extractor = Some("llm".to_string());
    edge.resolution_status = Some(
        match edge.confidence {
            crate::model::Confidence::Extracted => "resolved",
            crate::model::Confidence::Inferred => "inferred",
            crate::model::Confidence::Ambiguous => "ambiguous",
        }
        .to_string(),
    );
    Some(edge)
}

fn build_hyperedge(v: &serde_json::Value) -> Option<HyperEdge> {
    let id = str_field(v, "id").filter(|s| !s.is_empty())?;
    let label = str_field(v, "label").unwrap_or_else(|| id.clone());
    let relation = str_field(v, "relation").unwrap_or_else(|| "participate_in".to_string());
    let source_file = str_field(v, "source_file").unwrap_or_default();
    let confidence = parse_confidence(v["confidence"].as_str().unwrap_or("INFERRED"));
    let nodes: Vec<String> = v["nodes"]
        .as_array()?
        .iter()
        .filter_map(|n| n.as_str().map(str::to_owned))
        .collect();
    if nodes.len() < 3 {
        return None; // hyperedges require at least 3 participants
    }
    Some(HyperEdge::new(
        id,
        label,
        nodes,
        relation,
        confidence,
        source_file,
    ))
}

fn parse_file_type(s: &str) -> FileType {
    match s.to_lowercase().as_str() {
        "document" => FileType::Document,
        "paper" => FileType::Paper,
        "image" => FileType::Image,
        "rationale" => FileType::Rationale,
        _ => FileType::Code,
    }
}

fn parse_confidence(s: &str) -> Confidence {
    match s.to_uppercase().as_str() {
        "EXTRACTED" => Confidence::Extracted,
        "AMBIGUOUS" => Confidence::Ambiguous,
        _ => Confidence::Inferred,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_json() -> &'static str {
        r#"{"nodes":[{"id":"a_foo","label":"Foo","file_type":"code","source_file":"a.py"}],"edges":[{"source":"a_foo","target":"b_bar","relation":"calls","confidence":"EXTRACTED","confidence_score":1.0,"source_file":"a.py"}],"hyperedges":[]}"#
    }

    // ── Layer 1: direct JSON ───────────────────────────────────────────────────

    #[test]
    fn layer1_direct_parse() {
        let r = parse_extraction(minimal_json());
        assert_eq!(r.nodes.len(), 1);
        assert_eq!(r.edges.len(), 1);
        assert_eq!(r.nodes[0].id, "a_foo");
    }

    // ── Layer 2: code block extraction ────────────────────────────────────────

    #[test]
    fn layer2_json_code_block() {
        let text = format!("Here is the result:\n```json\n{}\n```", minimal_json());
        let r = parse_extraction(&text);
        assert_eq!(r.nodes.len(), 1);
        assert_eq!(r.edges.len(), 1);
    }

    #[test]
    fn layer2_plain_code_block() {
        let text = format!("Result:\n```\n{}\n```", minimal_json());
        let r = parse_extraction(&text);
        assert_eq!(r.nodes.len(), 1);
    }

    // ── Layer 3: brace extraction ─────────────────────────────────────────────

    #[test]
    fn layer3_embedded_json() {
        let text = format!("Here you go: {} and some trailing text", minimal_json());
        let r = parse_extraction(&text);
        assert_eq!(r.nodes.len(), 1);
    }

    // ── Partial validation ────────────────────────────────────────────────────

    #[test]
    fn empty_id_node_dropped() {
        let text = r#"{"nodes":[{"id":"","label":"X","file_type":"code","source_file":"f.py"}],"edges":[],"hyperedges":[]}"#;
        let r = parse_extraction(text);
        assert!(r.nodes.is_empty());
    }

    #[test]
    fn missing_source_edge_dropped() {
        let text = r#"{"nodes":[],"edges":[{"source":"","target":"b","relation":"calls","confidence":"EXTRACTED","confidence_score":1.0,"source_file":"f.py"}],"hyperedges":[]}"#;
        let r = parse_extraction(text);
        assert!(r.edges.is_empty());
    }

    #[test]
    fn hyperedge_with_two_nodes_dropped() {
        let text = r#"{"nodes":[],"edges":[],"hyperedges":[{"id":"h1","label":"L","nodes":["a","b"],"relation":"r","confidence":"INFERRED","confidence_score":0.5,"source_file":"f.py"}]}"#;
        let r = parse_extraction(text);
        assert!(r.hyperedges.is_empty());
    }

    #[test]
    fn hyperedge_with_three_nodes_kept() {
        let text = r#"{"nodes":[],"edges":[],"hyperedges":[{"id":"h1","label":"L","nodes":["a","b","c"],"relation":"r","confidence":"INFERRED","confidence_score":0.5,"source_file":"f.py"}]}"#;
        let r = parse_extraction(text);
        assert_eq!(r.hyperedges.len(), 1);
    }

    #[test]
    fn valid_and_invalid_items_mixed() {
        let text = r#"{
            "nodes":[
                {"id":"ok","label":"OK","file_type":"code","source_file":"f.py"},
                {"id":"","label":"Bad","file_type":"code","source_file":"f.py"}
            ],
            "edges":[],
            "hyperedges":[]
        }"#;
        let r = parse_extraction(text);
        assert_eq!(r.nodes.len(), 1);
        assert_eq!(r.nodes[0].id, "ok");
    }

    #[test]
    fn source_location_is_preserved_when_present() {
        let text = r#"{"nodes":[{"id":"ok","label":"OK","file_type":"code","source_file":"f.py","source_location":"L12:C1-L18:C4"}],"edges":[],"hyperedges":[]}"#;
        let r = parse_extraction(text);
        assert_eq!(r.nodes[0].source_location, "L12:C1-L18:C4");
    }

    // ── Confidence and file-type parsing ──────────────────────────────────────

    #[test]
    fn confidence_variants_parsed() {
        assert!(matches!(
            parse_confidence("EXTRACTED"),
            Confidence::Extracted
        ));
        assert!(matches!(parse_confidence("INFERRED"), Confidence::Inferred));
        assert!(matches!(
            parse_confidence("AMBIGUOUS"),
            Confidence::Ambiguous
        ));
        assert!(matches!(parse_confidence("unknown"), Confidence::Inferred));
        assert!(matches!(
            parse_confidence("extracted"),
            Confidence::Extracted
        ));
    }

    #[test]
    fn file_type_variants_parsed() {
        assert!(matches!(parse_file_type("code"), FileType::Code));
        assert!(matches!(parse_file_type("document"), FileType::Document));
        assert!(matches!(parse_file_type("paper"), FileType::Paper));
        assert!(matches!(parse_file_type("image"), FileType::Image));
        assert!(matches!(parse_file_type("rationale"), FileType::Rationale));
        assert!(matches!(parse_file_type("unknown"), FileType::Code));
    }

    #[test]
    fn confidence_score_clamped() {
        let text = r#"{"nodes":[],"edges":[{"source":"a","target":"b","relation":"calls","confidence":"EXTRACTED","confidence_score":99.0,"source_file":"f.py"}],"hyperedges":[]}"#;
        let r = parse_extraction(text);
        assert_eq!(r.edges[0].confidence_score, 1.0);
    }

    // ── Unparseable input ─────────────────────────────────────────────────────

    #[test]
    fn completely_unparseable_returns_empty() {
        let r = parse_extraction("I cannot comply with this request.");
        assert!(r.nodes.is_empty());
        assert!(r.edges.is_empty());
    }

    #[test]
    fn empty_input_returns_empty() {
        let r = parse_extraction("");
        assert!(r.is_empty());
    }
}
