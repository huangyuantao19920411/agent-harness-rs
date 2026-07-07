//! LLM provider adapters.

mod deepseek;
mod error;
mod mock;
mod provider;

pub use deepseek::DeepSeekModel;
pub use mock::MockModel;
pub use provider::{AnyModel, ModelBackend};

pub type Result<T> = std::result::Result<T, error::ModelError>;
