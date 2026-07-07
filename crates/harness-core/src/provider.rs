use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::message::Message;

#[derive(Debug, Clone)]
pub struct CompletionRequest {
    pub messages: Vec<Message>,
    pub tools: Vec<ToolSchemaRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSchemaRef {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRef {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone)]
pub enum ModelResponse {
    Text(String),
    ToolCalls(Vec<ToolCallRef>),
}

pub struct CompletionResult {
    pub response: ModelResponse,
}

#[async_trait]
pub trait ModelProvider: Send + Sync {
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResult, String>;
}
