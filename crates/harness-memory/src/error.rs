use thiserror::Error;

pub type Result<T> = std::result::Result<T, MemoryError>;

#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("model error: {0}")]
    Model(String),

    #[error("memory error: {0}")]
    Other(String),
}
