//! Tool Registry and Function Calling support.

mod error;
mod registry;
mod schema;
mod tool;

pub use error::ToolError;
pub use registry::ToolRegistry;
pub use schema::ToolSchema;
pub use tool::{Tool, ToolCall, ToolContext};

pub type Result<T> = std::result::Result<T, error::ToolError>;
