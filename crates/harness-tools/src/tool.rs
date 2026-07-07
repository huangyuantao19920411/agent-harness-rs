use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::schema::ToolSchema;
use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

pub struct ToolContext;

#[async_trait]
pub trait Tool: Send + Sync {
    fn schema(&self) -> ToolSchema;

    async fn execute(&self, args: &serde_json::Value) -> Result<String>;
}
