use thiserror::Error;

pub type Result<T> = std::result::Result<T, McpError>;

#[derive(Debug, Error)]
pub enum McpError {
    #[error("transport error: {0}")]
    Transport(String),

    #[error("protocol error: {0}")]
    Protocol(String),

    #[error("rpc error {code}: {message}")]
    Rpc { code: i64, message: String },

    #[error("tool error: {0}")]
    Tool(String),

    #[error("timeout after {0}s")]
    Timeout(u64),
}
