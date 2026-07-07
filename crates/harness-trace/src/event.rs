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
}
