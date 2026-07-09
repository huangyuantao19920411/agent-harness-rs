use std::sync::Arc;

use async_trait::async_trait;
use harness_core::{AgentLoop, AgentRequest, HarnessConfig};
use harness_mcp::{connect_and_register, McpClientConfig};
use harness_models::ModelBackend;
use harness_sandbox::SandboxScheduler;
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

struct SandboxExecTool {
    scheduler: Arc<SandboxScheduler>,
}

#[async_trait]
impl Tool for SandboxExecTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "sandbox_exec".into(),
            description: "Execute a command in a sandbox. Routes by task_type: trusted/code=process, untrusted/shell=K8s MicroVM. Shell commands are checked by exec policy.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "task_type": {
                        "type": "string",
                        "enum": ["trusted", "code", "untrusted", "shell"],
                        "description": "trusted/code=process sandbox, untrusted/shell=K8s MicroVM"
                    },
                    "command": { "type": "string", "description": "Command to run" },
                    "args": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Command arguments"
                    }
                },
                "required": ["task_type", "command"]
            }),
        }
    }

    async fn execute(&self, args: &serde_json::Value) -> harness_tools::Result<String> {
        let task_type = args
            .get("task_type")
            .and_then(|v| v.as_str())
            .unwrap_or("trusted");
        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| harness_tools::ToolError::InvalidArguments("command required".into()))?;

        let cmd_args: Vec<String> = args
            .get("args")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let arg_refs: Vec<&str> = cmd_args.iter().map(String::as_str).collect();

        let result = self
            .scheduler
            .exec(task_type, command, &arg_refs)
            .await
            .map_err(|e| harness_tools::ToolError::Execution(e.to_string()))?;

        let mut output = result.stdout;
        if !result.stderr.is_empty() {
            if !output.is_empty() {
                output.push('\n');
            }
            output.push_str("[stderr] ");
            output.push_str(&result.stderr);
        }
        if result.timed_out {
            output.push_str("\n[timed out]");
        }
        Ok(output)
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

    let scheduler = Arc::new(
        SandboxScheduler::with_defaults().expect("failed to init sandbox scheduler"),
    );

    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(ListDirTool));
    registry.register(Arc::new(SandboxExecTool {
        scheduler: scheduler.clone(),
    }));

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
                    "MCP connected: {} ({} tools total)",
                    client.server_name().unwrap_or("unknown"),
                    registry.schemas().len()
                );
            }
            Err(e) => eprintln!("MCP connection failed: {e}"),
        }
    }

    let tracer = Tracer::new();
    if let Ok(trace_path) = std::env::var("TRACE_PATH") {
        tracer
            .enable_persistence(&trace_path)
            .await
            .unwrap_or_else(|e| eprintln!("trace persistence: {e}"));
        eprintln!("Trace JSONL: {trace_path}");
    }

    let harness_config = if std::env::var("GUARDIAN_DISABLED").is_ok() {
        eprintln!("Guardian: disabled (permissive mode)");
        HarnessConfig::permissive()
    } else {
        eprintln!("Guardian: enabled (LLM review for unknown shell commands)");
        HarnessConfig::with_guardian()
    };

    if harness_config.memory.enabled {
        eprintln!("Memory: enabled ({})", harness_config.memory.db_path.display());
    }

    if harness_config.skills.enabled {
        eprintln!(
            "Skills: enabled (paths: {})",
            harness_config
                .skills
                .search_paths
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    let session_id = std::env::var("SESSION_ID").ok();

    let loop_engine = AgentLoop::new(model, registry, tracer.clone(), harness_config);

    let outcome = loop_engine
        .run(AgentRequest {
            input,
            session_id,
            system_prompt: Some(
                "You are a coding agent harness demo. Use list_dir or sandbox_exec tools when helpful. \
                 sandbox_exec routes by task_type: trusted=local process, untrusted=K8s MicroVM."
                    .into(),
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
        println!("{}", tracer.replay_summary().await);
    }
}
