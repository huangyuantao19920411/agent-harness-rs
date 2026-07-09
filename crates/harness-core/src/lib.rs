//! Agent Harness core: Agent Loop and Context Engineering.

mod compaction;
mod compaction_config;
mod config;
mod context;
mod error;
mod guardian;
mod guardian_config;
mod loop_engine;
mod memory;
mod message;
mod policy;
mod provider;
mod tool_orchestrator;
mod types;

pub use compaction::{
    compact_context, compact_with_model, format_transcript, should_compact, split_for_compaction,
    CompactionResult, COMPACTION_SYSTEM_PROMPT,
};
pub use compaction_config::CompactionConfig;
pub use config::HarnessConfig;
pub use context::{estimate_message_tokens, ContextManager};
pub use error::{HarnessError, Result};
pub use guardian::{
    parse_guardian_response, review_with_model, GuardianDecision, GuardianReviewer,
    GUARDIAN_SYSTEM_PROMPT,
};
pub use guardian_config::{ApprovalResult, GuardianConfig};
pub use loop_engine::AgentLoop;
pub use memory::extract_and_store;
pub use message::{AssistantToolCall, Message, Role};
pub use policy::{ApprovalMode, ExecPolicy, PolicyDecision};
pub use provider::{
    CompletionRequest, CompletionResult, ModelProvider, ModelResponse, ToolCallRef,
    ToolSchemaRef,
};
pub use tool_orchestrator::{ApprovalHandler, AutoApprove, DenyAll, ToolOrchestrator};
pub use types::{AgentRequest, AgentResponse, LoopOutcome};
