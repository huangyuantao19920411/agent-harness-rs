use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::json;
use tracing::info;

use crate::error::{McpError, Result};
use crate::protocol::{
    ClientCapabilities, ClientInfo, InitializeParams, InitializeResult, McpTool,
    ToolCallParams, ToolCallResult, ToolsListResult, PROTOCOL_VERSION,
};
use crate::transport::StdioTransport;

/// Configuration for connecting to an MCP server.
#[derive(Debug, Clone)]
pub struct McpClientConfig {
    pub command: String,
    pub args: Vec<String>,
    pub client_name: String,
    pub client_version: String,
    pub timeout_secs: u64,
}

impl McpClientConfig {
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            args: Vec::new(),
            client_name: "agent-harness-rs".into(),
            client_version: env!("CARGO_PKG_VERSION").into(),
            timeout_secs: 30,
        }
    }

    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }
}

/// MCP client over stdio transport.
pub struct McpClient {
    transport: StdioTransport,
    next_id: AtomicU64,
    server_name: Option<String>,
}

impl McpClient {
    pub async fn connect(config: McpClientConfig) -> Result<Self> {
        let arg_refs: Vec<&str> = config.args.iter().map(String::as_str).collect();
        let transport = StdioTransport::spawn(&config.command, &arg_refs).await?;

        let mut client = Self {
            transport,
            next_id: AtomicU64::new(1),
            server_name: None,
        };

        client.initialize(&config).await?;
        Ok(client)
    }

    async fn initialize(&mut self, config: &McpClientConfig) -> Result<()> {
        let id = self.next_id();
        let params = InitializeParams {
            protocol_version: PROTOCOL_VERSION.into(),
            capabilities: ClientCapabilities::default(),
            client_info: ClientInfo {
                name: config.client_name.clone(),
                version: config.client_version.clone(),
            },
        };

        let result: InitializeResult = self
            .request_raw(id, "initialize", Some(serde_json::to_value(params).unwrap()))
            .await?;

        self.server_name = result.server_info.map(|s| s.name);
        if let Some(name) = &self.server_name {
            info!(server = %name, version = %result.protocol_version, "mcp connected");
        }

        self.transport
            .notify("notifications/initialized", None)
            .await?;

        Ok(())
    }

    pub async fn list_tools(&self) -> Result<Vec<McpTool>> {
        let id = self.next_id();
        let result: ToolsListResult = self.request_raw(id, "tools/list", None).await?;
        Ok(result.tools)
    }

    pub async fn call_tool(&self, name: &str, arguments: serde_json::Value) -> Result<String> {
        let id = self.next_id();
        let params = ToolCallParams {
            name: name.into(),
            arguments,
        };

        let result: ToolCallResult = self
            .request_raw(id, "tools/call", Some(serde_json::to_value(params).unwrap()))
            .await?;

        if result.is_error {
            let msg = result
                .content
                .first()
                .and_then(|c| c.text.clone())
                .unwrap_or_else(|| "tool returned error".into());
            return Err(McpError::Tool(msg));
        }

        let text = result
            .content
            .iter()
            .filter_map(|c| c.text.as_ref())
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");

        Ok(if text.is_empty() {
            json!({"status": "ok"}).to_string()
        } else {
            text
        })
    }

    pub fn server_name(&self) -> Option<&str> {
        self.server_name.as_deref()
    }

    fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    async fn request_raw<T: serde::de::DeserializeOwned>(
        &self,
        id: u64,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<T> {
        let value = self.transport.request(id, method, params).await?;
        serde_json::from_value(value)
            .map_err(|e| McpError::Protocol(format!("deserialize {method}: {e}")))
    }
}
