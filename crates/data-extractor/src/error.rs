#[derive(Debug, thiserror::Error)]
pub enum ExtractError {
    #[error("GGPK I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("File not found in GGPK: {0}")]
    FileNotFound(String),

    #[error("dat64 parse error in {file}: {message}")]
    Dat64Parse { file: String, message: String },

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
}
