use thiserror::Error;

pub type Result<T> = std::result::Result<T, SandboxError>;

#[derive(Debug, Error)]
pub enum SandboxError {
    #[error("execution failed: {0}")]
    Execution(String),

    #[error("timeout after {0}s")]
    Timeout(u64),

    #[error("resource limit exceeded: {0}")]
    ResourceLimit(String),

    #[error("sandbox not available: {0}")]
    NotAvailable(String),
}
