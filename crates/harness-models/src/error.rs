use thiserror::Error;

#[derive(Debug, Error)]
pub enum ModelError {
    #[error("api error: {0}")]
    Api(String),

    #[error("invalid response: {0}")]
    InvalidResponse(String),
}
