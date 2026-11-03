use thiserror::Error;

pub type Result<T> = std::result::Result<T, HermesError>;

#[derive(Error, Debug)]
pub enum HermesError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Database / SQLite FTS5 error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("HTTP / LLM Provider request error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Provider resolution failed for '{provider}': {reason}")]
    ProviderResolution { provider: String, reason: String },

    #[error("Tool execution error in '{tool}': {message}")]
    ToolExecution { tool: String, message: String },

    #[error("Sandbox security violation: {0}")]
    SandboxViolation(String),

    #[error("Human-in-the-Loop approval rejected action '{action}': {reason}")]
    ApprovalRejected { action: String, reason: String },

    #[error("OpenHuman Harness capacity reached (max {max} concurrent agents)")]
    HarnessCapacityReached { max: usize },

    #[error("Unauthorized gateway access from user '{user}' on platform '{platform}'")]
    UnauthorizedGateway { platform: String, user: String },

    #[error("Agent interrupted or cancelled")]
    Interrupted,
}
