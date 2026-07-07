use std::sync::Arc;

use async_trait::async_trait;
use harness_core::{AgentLoop, AgentRequest, HarnessConfig};
use harness_mcp::{connect_and_register, McpClientConfig};
use harness_models::ModelBackend;
use harness_tools::{Tool, ToolRegistry, ToolSchema};
use harness_trace::Tracer;
use tracing_subscriber::EnvFilter;

struct ListDirTool;

#[async_trait]
impl Tool for ListDirTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "list_dir".into(),
            description: "List files in a directory".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Directory path" }
                },
                "required": ["path"]
            }),
        }
    }

    async fn execute(&self, args: &serde_json::Value) -> harness_tools::Result<String> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");
        let entries: Vec<String> = std::fs::read_dir(path)
            .map_err(|e| harness_tools::ToolError::Execution(e.to_string()))?
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        Ok(entries.join(", "))
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args: Vec<String> = std::env::args().collect();
    let input = args.get(1).cloned().unwrap_or_else(|| {
        "Please list files in the current directory".to_string()
    });

    let model = ModelBackend::from_env();
    eprintln!("Using model backend: {}", model.name());

    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(ListDirTool));

    if let Ok(mcp_cmd) = std::env::var("MCP_SERVER_COMMAND") {
        let mcp_args: Vec<String> = std::env::var("MCP_SERVER_ARGS")
            .unwrap_or_default()
            .split_whitespace()
            .map(String::from)
            .collect();

        let mut config = McpClientConfig::new(mcp_cmd);
        for arg in mcp_args {
            config = config.arg(arg);
        }

        match connect_and_register(config, &mut registry).await {
            Ok(client) => {
                eprintln!(
                    "MCP connected: {} ({} tools)",
                    client.server_name().unwrap_or("unknown"),
                    registry.schemas().len()
                );
            }
            Err(e) => eprintln!("MCP connection failed: {e}"),
        }
    }

    let tracer = Tracer::new();
    let loop_engine = AgentLoop::new(model, registry, tracer.clone(), HarnessConfig::default());

    let outcome = loop_engine
        .run(AgentRequest {
            input,
            system_prompt: Some(
                "You are a coding agent harness demo. Use tools when helpful.".into(),
            ),
        })
        .await
        .expect("agent loop failed");

    println!("=== Agent Response ===");
    println!("{}", outcome.response.output);
    println!();
    println!(
        "Iterations: {}, Tool calls: {}",
        outcome.response.iterations, outcome.response.tool_calls
    );

    let events = tracer.events().await;
    if !events.is_empty() {
        println!();
        println!("=== Trace ({} events) ===", events.len());
        for event in events {
            println!("{event:?}");
        }
    }
}
