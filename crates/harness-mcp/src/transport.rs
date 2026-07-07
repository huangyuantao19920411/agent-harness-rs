use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use tracing::{debug, warn};

use crate::error::{McpError, Result};
use crate::protocol::{JsonRpcRequest, JsonRpcResponse, JSONRPC_VERSION};

/// Stdio transport for MCP JSON-RPC communication.
pub struct StdioTransport {
    stdin: Mutex<ChildStdin>,
    stdout: Mutex<BufReader<ChildStdout>>,
    _child: Child,
}

impl StdioTransport {
    pub async fn spawn(command: &str, args: &[&str]) -> Result<Self> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| McpError::Transport(format!("spawn {command}: {e}")))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| McpError::Transport("no stdin".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| McpError::Transport("no stdout".into()))?;

        Ok(Self {
            stdin: Mutex::new(stdin),
            stdout: Mutex::new(BufReader::new(stdout)),
            _child: child,
        })
    }

    pub async fn request(&self, id: u64, method: &str, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let req = JsonRpcRequest {
            jsonrpc: JSONRPC_VERSION,
            id,
            method: method.into(),
            params,
        };

        let line = serde_json::to_string(&req)
            .map_err(|e| McpError::Protocol(format!("serialize: {e}")))?;

        debug!(method, id, "mcp request");
        {
            let mut stdin = self.stdin.lock().await;
            stdin
                .write_all(line.as_bytes())
                .await
                .map_err(|e| McpError::Transport(format!("write: {e}")))?;
            stdin
                .write_all(b"\n")
                .await
                .map_err(|e| McpError::Transport(format!("write newline: {e}")))?;
            stdin
                .flush()
                .await
                .map_err(|e| McpError::Transport(format!("flush: {e}")))?;
        }

        let response = self.read_response(id).await?;
        if let Some(err) = response.error {
            return Err(McpError::Rpc {
                code: err.code,
                message: err.message,
            });
        }
        response
            .result
            .ok_or_else(|| McpError::Protocol("empty result".into()))
    }

    pub async fn notify(&self, method: &str, params: Option<serde_json::Value>) -> Result<()> {
        let notification = serde_json::json!({
            "jsonrpc": JSONRPC_VERSION,
            "method": method,
            "params": params,
        });
        let line = serde_json::to_string(&notification)
            .map_err(|e| McpError::Protocol(format!("serialize: {e}")))?;

        let mut stdin = self.stdin.lock().await;
        stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| McpError::Transport(format!("write: {e}")))?;
        stdin
            .write_all(b"\n")
            .await
            .map_err(|e| McpError::Transport(format!("write newline: {e}")))?;
        stdin.flush().await.map_err(|e| McpError::Transport(format!("flush: {e}")))
    }

    async fn read_response(&self, expected_id: u64) -> Result<JsonRpcResponse> {
        let mut stdout = self.stdout.lock().await;
        let mut line = String::new();

        loop {
            line.clear();
            let n = stdout
                .read_line(&mut line)
                .await
                .map_err(|e| McpError::Transport(format!("read: {e}")))?;
            if n == 0 {
                return Err(McpError::Transport("server closed stdout".into()));
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let response: JsonRpcResponse = serde_json::from_str(trimmed)
                .map_err(|e| McpError::Protocol(format!("parse response: {e}; line={trimmed}")))?;

            if response.id == Some(expected_id) {
                return Ok(response);
            }
            warn!(expected_id, got_id = ?response.id, "skipping unrelated jsonrpc message");
        }
    }
}
