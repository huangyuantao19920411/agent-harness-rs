use thiserror::Error;

pub type Result<T> = std::result::Result<T, HarnessError>;

#[derive(Debug, Error)]
pub enum HarnessError {
    #[error("max iterations ({0}) exceeded")]
    MaxIterationsExceeded(u32),

    #[error("model error: {0}")]
    Model(String),

    #[error("tool error: {0}")]
    Tool(#[from] harness_tools::ToolError),

    #[error("context error: {0}")]
    Context(String),
}
