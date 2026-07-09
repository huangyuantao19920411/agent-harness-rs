use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TraceEvent {
    ToolCall {
        iteration: u32,
        name: String,
        arguments: serde_json::Value,
    },
    ToolResult {
        iteration: u32,
        name: String,
        result: String,
    },
    FinalAnswer {
        iteration: u32,
        content: String,
    },
    ContextCompacted {
        iteration: u32,
        messages_before: usize,
        messages_after: usize,
        tokens_before: usize,
        tokens_after: usize,
        summary_preview: String,
    },
    ToolApprovalReview {
        iteration: u32,
        name: String,
        approved: bool,
        reviewer: String,
        reason: String,
    },
    MemoryRecalled {
        session_id: String,
        count: usize,
        preview: String,
    },
    MemoryPersisted {
        session_id: String,
        count: usize,
    },
    SkillLoaded {
        name: String,
        path: String,
    },
}
