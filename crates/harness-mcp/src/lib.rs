//! Model Context Protocol (MCP) client for Agent Harness.
//!
//! Implements JSON-RPC 2.0 over stdio to connect to MCP servers and
//! bridge remote tools into the local [`ToolRegistry`].

mod client;
mod error;
mod protocol;
mod tool_adapter;
mod transport;

pub use client::{McpClient, McpClientConfig};
pub use error::{McpError, Result};
pub use protocol::{McpTool, McpToolInputSchema};
pub use tool_adapter::{connect_and_register, register_mcp_tools, McpToolWrapper};
