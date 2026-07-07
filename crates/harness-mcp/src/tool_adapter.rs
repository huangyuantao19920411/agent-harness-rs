use std::sync::Arc;

use async_trait::async_trait;
use harness_tools::{Tool, ToolRegistry, ToolSchema};
use serde_json::json;

use crate::client::McpClient;
use crate::error::Result;
use crate::protocol::McpTool;

/// Wraps a remote MCP tool as a local [`Tool`].
pub struct McpToolWrapper {
    tool: McpTool,
    client: Arc<McpClient>,
}

impl McpToolWrapper {
    pub fn new(tool: McpTool, client: Arc<McpClient>) -> Self {
        Self { tool, client }
    }
}

#[async_trait]
impl Tool for McpToolWrapper {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.tool.name.clone(),
            description: self
                .tool
                .description
                .clone()
                .unwrap_or_else(|| format!("MCP tool: {}", self.tool.name)),
            parameters: json!({
                "type": self.tool.input_schema.schema_type,
                "properties": self.tool.input_schema.properties,
                "required": self.tool.input_schema.required,
            }),
        }
    }

    async fn execute(&self, args: &serde_json::Value) -> harness_tools::Result<String> {
        self.client
            .call_tool(&self.tool.name, args.clone())
            .await
            .map_err(|e| harness_tools::ToolError::Execution(e.to_string()))
    }
}

/// Discover tools from an MCP server and register them into a [`ToolRegistry`].
pub async fn register_mcp_tools(client: Arc<McpClient>, registry: &mut ToolRegistry) -> Result<usize> {
    let tools = client.list_tools().await?;
    let count = tools.len();

    for tool in tools {
        registry.register(Arc::new(McpToolWrapper::new(tool, client.clone())));
    }

    Ok(count)
}

/// Helper to connect and register in one step.
pub async fn connect_and_register(
    config: crate::client::McpClientConfig,
    registry: &mut ToolRegistry,
) -> Result<Arc<McpClient>> {
    let client = Arc::new(McpClient::connect(config).await?);
    register_mcp_tools(client.clone(), registry).await?;
    Ok(client)
}
