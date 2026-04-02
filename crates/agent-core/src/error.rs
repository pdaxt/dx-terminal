use thiserror::Error;

#[derive(Debug, Error)]
pub enum DxosError {
    #[error("API error: {0}")]
    Api(String),

    #[error("Tool execution failed: {tool} — {message}")]
    Tool { tool: String, message: String },

    #[error("Permission denied: {tool} requires {required} (current: {current})")]
    Permission {
        tool: String,
        required: String,
        current: String,
    },

    #[error("Session error: {0}")]
    Session(String),

    #[error("Config error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Turn limit exceeded after {iterations} iterations")]
    TurnLimitExceeded { iterations: usize },
}

pub type Result<T> = std::result::Result<T, DxosError>;
