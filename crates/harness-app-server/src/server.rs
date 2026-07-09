use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::sync::Arc;

use harness_core::{AgentLoop, AgentRequest, HarnessConfig, ModelProvider};
use harness_tools::ToolRegistry;
use harness_trace::Tracer;
use serde_json::{json, Value};
use tokio::sync::Mutex;
use tracing::{error, info};
use uuid::Uuid;

use crate::protocol::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};

struct ThreadState {
    tracer: Tracer,
}

/// Codex-inspired App Server: JSON-RPC 2.0 over stdio.
pub struct AppServer<M: ModelProvider> {
    model: Arc<M>,
    tools: ToolRegistry,
    config: HarnessConfig,
    threads: Mutex<HashMap<String, ThreadState>>,
}

impl<M: ModelProvider + 'static> AppServer<M> {
    pub fn new(model: M, tools: ToolRegistry, config: HarnessConfig) -> Self {
        Self {
            model: Arc::new(model),
            tools,
            config,
            threads: Mutex::new(HashMap::new()),
        }
    }

    /// Run the server loop reading JSON-RPC from stdin, writing to stdout.
    pub async fn run_stdio(&self) -> io::Result<()> {
        let stdin = io::stdin();
        let mut stdout = io::stdout();

        for line in stdin.lock().lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            let response = match serde_json::from_str::<JsonRpcRequest>(&line) {
                Ok(req) => self.handle_request(req).await,
                Err(e) => JsonRpcResponse::err(None, -32700, format!("parse error: {e}")),
            };

            let out = serde_json::to_string(&response).map_err(io::Error::other)?;
            writeln!(stdout, "{out}")?;
            stdout.flush()?;
        }

        Ok(())
    }

    async fn handle_request(&self, req: JsonRpcRequest) -> JsonRpcResponse {
        let id = req.id.clone();
        match req.method.as_str() {
            "initialize" => JsonRpcResponse::ok(
                id,
                json!({
                    "capabilities": {
                        "threads": true,
                        "turns": true,
                        "trace": true,
                    },
                    "server": "harness-app-server",
                    "version": "0.1.0",
                }),
            ),
            "thread/start" => self.thread_start(id, &req.params).await,
            "turn/submit" => self.turn_submit(id, &req.params).await,
            "thread/list" => {
                let threads = self.threads.lock().await;
                JsonRpcResponse::ok(
                    id,
                    json!({ "threads": threads.keys().collect::<Vec<_>>() }),
                )
            }
            _ => JsonRpcResponse::err(id, -32601, format!("method not found: {}", req.method)),
        }
    }

    async fn thread_start(&self, id: Option<Value>, params: &Value) -> JsonRpcResponse {
        let thread_id = Uuid::new_v4().to_string();
        let tracer = Tracer::new();

        if let Some(path) = params.get("trace_path").and_then(|v| v.as_str()) {
            if let Err(e) = tracer.enable_persistence(path).await {
                error!(%e, "trace persistence failed");
            }
        }

        self.threads.lock().await.insert(
            thread_id.clone(),
            ThreadState {
                tracer: tracer.clone(),
            },
        );

        info!(%thread_id, "thread started");
        JsonRpcResponse::ok(id, json!({ "thread_id": thread_id }))
    }

    async fn turn_submit(&self, id: Option<Value>, params: &Value) -> JsonRpcResponse {
        let thread_id = match params.get("thread_id").and_then(|v| v.as_str()) {
            Some(t) => t.to_string(),
            None => return JsonRpcResponse::err(id, -32602, "thread_id required"),
        };
        let input = match params.get("input").and_then(|v| v.as_str()) {
            Some(i) => i.to_string(),
            None => return JsonRpcResponse::err(id, -32602, "input required"),
        };

        let tracer = {
            let threads = self.threads.lock().await;
            match threads.get(&thread_id) {
                Some(t) => t.tracer.clone(),
                None => return JsonRpcResponse::err(id, -32602, "unknown thread_id"),
            }
        };

        self.emit_notification("turn/started", json!({ "thread_id": thread_id }));

        let loop_engine = AgentLoop::with_arc(
            Arc::clone(&self.model),
            self.tools.clone(),
            tracer.clone(),
            self.config.clone(),
        );

        let system = params
            .get("system_prompt")
            .and_then(|v| v.as_str())
            .map(String::from);

        match loop_engine
            .run(AgentRequest {
                input,
                system_prompt: system,
                session_id: Some(thread_id.clone()),
            })
            .await
        {
            Ok(outcome) => {
                self.emit_notification(
                    "turn/completed",
                    json!({
                        "thread_id": thread_id,
                        "output": outcome.response.output,
                        "iterations": outcome.response.iterations,
                        "tool_calls": outcome.response.tool_calls,
                    }),
                );

                let replay = tracer.replay_summary().await;
                JsonRpcResponse::ok(
                    id,
                    json!({
                        "thread_id": thread_id,
                        "output": outcome.response.output,
                        "iterations": outcome.response.iterations,
                        "tool_calls": outcome.response.tool_calls,
                        "trace_summary": replay,
                    }),
                )
            }
            Err(e) => {
                error!(%e, "turn failed");
                JsonRpcResponse::err(id, -32000, e.to_string())
            }
        }
    }

    fn emit_notification(&self, method: &str, params: Value) {
        let note = JsonRpcNotification::new(method, params);
        if let Ok(line) = serde_json::to_string(&note) {
            let mut stdout = io::stdout();
            let _ = writeln!(stdout, "{line}");
            let _ = stdout.flush();
        }
    }
}
