use serde::{Deserialize, Serialize};

use crate::message::Message;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRequest {
    pub input: String,
    pub system_prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    pub output: String,
    pub iterations: u32,
    pub tool_calls: u32,
}

#[derive(Debug, Clone)]
pub struct LoopOutcome {
    pub response: AgentResponse,
    pub messages: Vec<Message>,
}
