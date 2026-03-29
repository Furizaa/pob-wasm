#[derive(Debug, thiserror::Error)]
pub enum CalcError {
    #[error("Parse error: {0}")]
    Parse(#[from] ParseError),

    #[error("Data error: {0}")]
    Data(#[from] DataError),
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("XML error: {0}")]
    Xml(String),

    #[error("Missing required attribute '{attr}' on element '{element}'")]
    MissingAttr { element: String, attr: String },

    #[error("Invalid value '{value}' for '{field}': {reason}")]
    InvalidValue {
        field: String,
        value: String,
        reason: String,
    },

    #[error("Base64 decode error: {0}")]
    Base64(String),
}

#[derive(Debug, thiserror::Error)]
pub enum DataError {
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Unknown gem: {0}")]
    UnknownGem(String),

    #[error("Unknown passive node: {0}")]
    UnknownNode(u32),
}
