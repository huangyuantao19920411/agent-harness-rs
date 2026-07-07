//! Agent Harness core: Agent Loop and Context Engineering.

mod config;
mod context;
mod error;
mod loop_engine;
mod message;
mod provider;
mod types;

pub use config::HarnessConfig;
pub use context::ContextManager;
pub use error::{HarnessError, Result};
pub use loop_engine::AgentLoop;
pub use message::{AssistantToolCall, Message, Role};
pub use provider::{
    CompletionRequest, CompletionResult, ModelProvider, ModelResponse, ToolCallRef,
    ToolSchemaRef,
};
pub use types::{AgentRequest, AgentResponse, LoopOutcome};
