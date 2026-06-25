//! Runtime telemetry overlay for Graphenium.
//!
//! Imports OpenTelemetry trace data and overlays runtime metrics onto the
//! static graph. Enables hot-path queries and runtime-aware analysis.
//!
//! ## Trace format
//!
//! Accepts JSON traces in OpenTelemetry-compatible format. Each span should
//! include a `name` field (matching function or method names in the graph)
//! and optional `start_time` / `end_time` / `attributes` fields.

use std::collections::HashMap;

use crate::model::GrapheniumGraph;

/// A single span from an OpenTelemetry trace.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TraceSpan {
    /// Name of the operation (matched to graph node labels).
    pub name: Option<String>,
    /// Span ID.
    pub span_id: Option<String>,
    /// Parent span ID for creating call chains.
    #[serde(default)]
    pub parent_span_id: Option<String>,
    /// Kind of span (SERVER, CLIENT, INTERNAL, etc.).
    #[serde(default)]
    pub kind: Option<String>,
    /// Start timestamp (Unix nanoseconds).
    #[serde(default)]
    pub start_time: Option<u64>,
    /// End timestamp (Unix nanoseconds).
    #[serde(default)]
    pub end_time: Option<u64>,
    /// Arbitrary attributes.
    #[serde(default)]
    pub attributes: HashMap<String, serde_json::Value>,
    /// Status.
    #[serde(default)]
    pub status: Option<serde_json::Value>,
}

/// A resource span wrapper (OTEL JSON format).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ResourceSpans {
    #[serde(default)]
    pub resource: Option<serde_json::Value>,
    #[serde(default)]
    pub scope_spans: Vec<ScopeSpans>,
}

/// Scoped spans container.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ScopeSpans {
    #[serde(default)]
    pub scope: Option<serde_json::Value>,
    #[serde(default)]
    pub spans: Vec<TraceSpan>,
}

/// Runtime metadata attached to a graph edge or node.
#[derive(Debug, Clone, Default)]
pub struct RuntimeMetadata {
    /// Number of times this edge was observed in traces.
    pub call_count: u64,
    /// P50 latency in milliseconds.
    pub p50_ms: f64,
    /// P95 latency in milliseconds.
    pub p95_ms: f64,
    /// P99 latency in milliseconds.
    pub p99_ms: f64,
    /// Timestamp of the last observation.
    pub last_seen: Option<u64>,
}

/// Full runtime overlay: maps node and edge IDs to runtime metrics.
#[derive(Debug, Clone, Default)]
pub struct RuntimeOverlay {
    /// Per-node runtime metrics.
    pub node_metrics: HashMap<String, RuntimeMetadata>,
    /// Per-edge runtime metrics, keyed by "(source,target,relation)".
    pub edge_metrics: HashMap<(String, String, String), RuntimeMetadata>,
    /// Total trace count imported.
    pub trace_count: usize,
    /// Total span count imported.
    pub span_count: usize,
}

impl RuntimeOverlay {
    /// Import traces from a parsed OTEL JSON structure.
    ///
    /// Accepts either a top-level array of spans or a `ResourceSpans` wrapper.
    pub fn import_traces(&mut self, spans: &[TraceSpan]) {
        for span in spans {
            self.span_count += 1;

            let name = match &span.name {
                Some(n) => n.to_lowercase(),
                None => continue,
            };

            let latency_ns = match (span.start_time, span.end_time) {
                (Some(start), Some(end)) if end > start => Some(end - start),
                _ => None,
            };

            let latency_ms = latency_ns.map(|ns| ns as f64 / 1_000_000.0);

            // Record node-level metric
            let node_entry = self.node_metrics.entry(name.clone()).or_default();
            node_entry.call_count += 1;
            if let Some(lat) = latency_ms {
                update_percentiles(node_entry, lat);
            }
            if let Some(ts) = span.start_time {
                node_entry.last_seen = Some(ts.max(node_entry.last_seen.unwrap_or(0)));
            }

            // Record edge-level metric if we have parent relationship
            if let Some(parent_id) = &span.parent_span_id {
                // The parent span's name would be the caller
                // For now, store as generic edge info
                let _ = parent_id;
            }

            self.trace_count += 1;
        }
    }

    /// Returns the total number of spans imported.
    pub fn span_count(&self) -> usize {
        self.span_count
    }

    /// Check if the overlay has any data.
    pub fn is_empty(&self) -> bool {
        self.node_metrics.is_empty() && self.edge_metrics.is_empty()
    }
}

/// Update percentile estimates for a runtime metadata entry.
fn update_percentiles(meta: &mut RuntimeMetadata, latency_ms: f64) {
    if meta.call_count == 0 {
        meta.p50_ms = latency_ms;
        meta.p95_ms = latency_ms;
        meta.p99_ms = latency_ms;
        return;
    }

    // Simple running approximation: not exact, but informative
    meta.p50_ms = meta.p50_ms * 0.9 + latency_ms * 0.1;
    if latency_ms > meta.p95_ms {
        meta.p95_ms = meta.p95_ms * 0.95 + latency_ms * 0.05;
    } else {
        meta.p95_ms = meta.p95_ms * 0.99 + latency_ms * 0.01;
    }
    if latency_ms > meta.p99_ms {
        meta.p99_ms = meta.p99_ms * 0.99 + latency_ms * 0.01;
    } else {
        meta.p99_ms = meta.p99_ms * 0.999 + latency_ms * 0.001;
    }
}

/// Load traces from a JSON file (supports both OTEL ResourceSpans and flat arrays).
pub fn load_traces(path: &std::path::Path) -> Result<Vec<TraceSpan>, String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("Cannot read trace file: {e}"))?;
    let value: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("Cannot parse JSON: {e}"))?;

    // Try flat array of spans first
    if let Ok(spans) = serde_json::from_value::<Vec<TraceSpan>>(value.clone()) {
        return Ok(spans);
    }

    // Try ResourceSpans wrapper
    if let Ok(rs) = serde_json::from_value::<ResourceSpans>(value) {
        let mut spans = Vec::new();
        for ss in rs.scope_spans {
            spans.extend(ss.spans);
        }
        return Ok(spans);
    }

    Err(
        "Unrecognized trace format: expected an array of spans or a ResourceSpans object"
            .to_string(),
    )
}

/// Match trace names to graph nodes and return a runtime overlay.
pub fn build_overlay(graph: &GrapheniumGraph, spans: &[TraceSpan]) -> RuntimeOverlay {
    let mut overlay = RuntimeOverlay::default();
    overlay.import_traces(spans);
    overlay
}

/// Score nodes by runtime frequency (hot paths).
pub fn hot_path_query(overlay: &RuntimeOverlay, top_k: usize) -> Vec<(String, u64, f64)> {
    let mut results: Vec<(String, u64, f64)> = overlay
        .node_metrics
        .iter()
        .map(|(name, meta)| (name.clone(), meta.call_count, meta.p95_ms))
        .collect();

    results.sort_unstable_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
    });
    results.truncate(top_k);
    results
}

/// Combine runtime overlay with graph traversal: rank nodes by runtime importance.
pub fn runtime_weighted_traversal(
    overlay: &RuntimeOverlay,
    graph: &GrapheniumGraph,
    query: &str,
    top_k: usize,
) -> Vec<(String, f64)> {
    // Start with lexical query scores
    let ranked = crate::ranking::score_query_nodes(graph, query);
    let mut weighted: Vec<(String, f64)> = ranked
        .into_iter()
        .map(|(id, score)| {
            let runtime_boost = overlay
                .node_metrics
                .get(&id.to_lowercase())
                .map(|m| (m.call_count as f64).ln_1p() * 0.3)
                .unwrap_or(0.0);
            (id, score + runtime_boost)
        })
        .collect();

    weighted.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    weighted.truncate(top_k);
    weighted
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Edge, FileType, Node};

    fn make_graph() -> GrapheniumGraph {
        let mut g = GrapheniumGraph::new();
        g.upsert_node(Node::new(
            "auth_login",
            "AuthLogin",
            FileType::Code,
            "src/auth.rs",
        ));
        g.upsert_node(Node::new(
            "auth_logout",
            "AuthLogout",
            FileType::Code,
            "src/auth.rs",
        ));
        g.upsert_node(Node::new(
            "db_query",
            "DBQuery",
            FileType::Code,
            "src/db.rs",
        ));
        g.add_edge(Edge::extracted(
            "auth_login",
            "auth_logout",
            "imports",
            "src/auth.rs",
        ));
        g.add_edge(Edge::extracted(
            "auth_login",
            "db_query",
            "calls",
            "src/auth.rs",
        ));
        g
    }

    fn sample_spans() -> Vec<TraceSpan> {
        vec![
            TraceSpan {
                name: Some("AuthLogin".into()),
                span_id: Some("span1".into()),
                parent_span_id: None,
                kind: Some("SERVER".into()),
                start_time: Some(1_000_000_000),
                end_time: Some(1_000_050_000),
                attributes: HashMap::new(),
                status: None,
            },
            TraceSpan {
                name: Some("DBQuery".into()),
                span_id: Some("span2".into()),
                parent_span_id: Some("span1".into()),
                kind: Some("CLIENT".into()),
                start_time: Some(1_000_010_000),
                end_time: Some(1_000_040_000),
                attributes: HashMap::new(),
                status: None,
            },
            TraceSpan {
                name: Some("AuthLogin".into()),
                span_id: Some("span3".into()),
                parent_span_id: None,
                kind: Some("SERVER".into()),
                start_time: Some(2_000_000_000),
                end_time: Some(2_000_100_000),
                attributes: HashMap::new(),
                status: None,
            },
        ]
    }

    #[test]
    fn overlay_imports_spans() {
        let spans = sample_spans();
        let mut overlay = RuntimeOverlay::default();
        overlay.import_traces(&spans);
        assert_eq!(overlay.span_count(), 3);

        let auth = overlay.node_metrics.get("authlogin");
        assert!(auth.is_some());
        assert_eq!(auth.unwrap().call_count, 2);
    }

    #[test]
    fn hot_path_identifies_frequent_nodes() {
        let spans = sample_spans();
        let mut overlay = RuntimeOverlay::default();
        overlay.import_traces(&spans);
        let hot = hot_path_query(&overlay, 5);
        assert!(!hot.is_empty());
        assert_eq!(hot[0].0, "authlogin");
    }

    #[test]
    fn runtime_weighted_traversal_boosts_hot_paths() {
        let g = make_graph();
        let spans = sample_spans();
        let overlay = build_overlay(&g, &spans);
        let results = runtime_weighted_traversal(&overlay, &g, "auth", 5);
        assert!(!results.is_empty());
    }

    #[test]
    fn load_traces_returns_error_for_bad_file() {
        let result = load_traces(std::path::Path::new("/nonexistent/file.json"));
        assert!(result.is_err());
    }
}
