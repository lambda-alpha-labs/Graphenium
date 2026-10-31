//! # Graphenium
//!
//! The elemental knowledge graph engine for your codebase.
//!
//! Graphenium builds a persistent, queryable knowledge graph from source code
//! and documents.  It can be used as a standalone CLI tool (`gm`) or embedded
//! as a library in AI coding harnesses.
//!
//! ## Library usage
//!
//! ```rust,no_run
//! use graphenium::extract::{extract_file, ExtractOptions};
//! use graphenium::build::build_from_extraction;
//! use graphenium::detect::DetectedFile;
//! use graphenium::model::FileType;
//!
//! let file = DetectedFile {
//!     path: "src/main.rs".into(),
//!     file_type: FileType::Code,
//! };
//! let result = extract_file(&file, &ExtractOptions::default());
//! let (mut graph, stats) = build_from_extraction(&result);
//! assert!(graph.node_count() > 0);
//! ```
//!
//! ## Feature flags
//!
//! - `harness` — compatibility marker for core graph-engine embedding. Use
//!   `default-features = false` to omit the MCP server, watch mode, and
//!   semantic HTTP client.
//! - `mcp` — MCP server (`rmcp`, `arc-swap`, `tokio`).
//! - `watch` — file-watching mode (`notify`, `tokio`).
//! - `semantic` — remote LLM extraction (`reqwest`, `base64`, `tokio`).
//! - Language features (`lang-python`, `lang-rust`, etc.) — enable specific
//!   tree-sitter grammars. All enabled by default.

// Design-level lints that are intentional across the public API surface.
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]
#![allow(clippy::should_implement_trait)]

pub mod analyze;
pub mod build;
pub mod cache;
pub mod cluster;
pub mod detect;
pub mod doctor;
pub mod embed;
pub mod error;
pub mod export;
pub mod extract;
pub mod graph_integrity;
pub mod harness;
pub mod model;
pub mod policy;
pub mod ranking;
pub mod report;
pub mod resolver;
#[cfg(feature = "semantic")]
pub mod semantic;
#[cfg(feature = "mcp")]
pub mod serve;
pub mod telemetry;
pub mod trust;
pub mod validate;
#[cfg(feature = "watch")]
pub mod watch;

pub use error::GrapheniumError;

/// Convenience alias for `Result<T, GrapheniumError>`.
pub type Result<T> = std::result::Result<T, GrapheniumError>;

// ── Re-exports for library consumers ─────────────────────────────────────

pub use analyze::{analyze, evaluate_delta_gate, AnalysisResult, DeltaGateReport};
pub use build::{build_from_extraction, build_merged, BuildStats};
pub use cluster::{cluster, ClusterOptions, CommunityStats};
pub use detect::DetectedFile;
pub use extract::{extract_all, extract_file, ExtractMode, ExtractOptions};
pub use model::{
    Confidence, Edge, ExtractionResult, FileType, GrapheniumGraph, HyperEdge, Node, ReplaceStats,
};
