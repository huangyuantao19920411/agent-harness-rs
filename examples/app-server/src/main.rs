use harness_app_server::AppServer;
use harness_core::HarnessConfig;
use harness_models::MockModel;
use harness_tools::{Tool, ToolRegistry, ToolSchema};
use async_trait::async_trait;
use std::sync::Arc;

struct EchoTool;

#[async_trait]
impl Tool for EchoTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "echo".into(),
            description: "Echo input back".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string" }
                },
                "required": ["message"]
            }),
        }
    }

    async fn execute(&self, args: &serde_json::Value) -> harness_tools::Result<String> {
        Ok(args
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string())
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    eprintln!("harness-app-server: JSON-RPC over stdio (Codex App Server inspired)");
    eprintln!("Methods: initialize, thread/start, turn/submit, thread/list");

    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(EchoTool));

    let server = AppServer::new(MockModel, tools, HarnessConfig::default());
    server.run_stdio().await.expect("app server failed");
}
