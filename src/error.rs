use thiserror::Error;

#[derive(Error, Debug)]
pub enum GrapheniumError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("API error (status {status}): {message}")]
    Api { status: u16, message: String },

    #[error("Node not found: {0}")]
    NodeNotFound(String),

    #[error("Graph file not found: {0}")]
    GraphNotFound(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Watch error: {0}")]
    Watch(String),

    #[error("Cache error: {0}")]
    Cache(String),

    #[error("MCP server error: {0}")]
    Serve(String),
}
