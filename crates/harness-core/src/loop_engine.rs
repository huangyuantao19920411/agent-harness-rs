use crate::provider::{CompletionRequest, ModelProvider, ModelResponse, ToolCallRef, ToolSchemaRef};
use crate::{HarnessError, Result};
use crate::config::HarnessConfig;
use crate::context::ContextManager;
use crate::message::Message;
use crate::types::{AgentRequest, AgentResponse, LoopOutcome};
use harness_tools::ToolRegistry;
use harness_trace::{TraceEvent, Tracer};
use tracing::info;

/// ReAct-style Agent Loop: reason → act (tool) → observe → repeat.
pub struct AgentLoop<M: ModelProvider> {
    model: M,
    tools: ToolRegistry,
    tracer: Tracer,
    config: HarnessConfig,
}

impl<M: ModelProvider> AgentLoop<M> {
    pub fn new(model: M, tools: ToolRegistry, tracer: Tracer, config: HarnessConfig) -> Self {
        Self {
            model,
            tools,
            tracer,
            config,
        }
    }

    pub async fn run(&self, request: AgentRequest) -> Result<LoopOutcome> {
        let mut ctx = ContextManager::new(self.config.max_context_tokens / 512);
        let system = request
            .system_prompt
            .unwrap_or_else(|| self.default_system_prompt());
        ctx.reset_with(Message::system(system), Message::user(request.input));

        let mut iterations = 0u32;
        let mut tool_calls = 0u32;

        loop {
            iterations += 1;
            if iterations > self.config.max_iterations {
                return Err(HarnessError::MaxIterationsExceeded(
                    self.config.max_iterations,
                ));
            }

            let tool_schemas: Vec<ToolSchemaRef> = self
                .tools
                .schemas()
                .into_iter()
                .map(|s| ToolSchemaRef {
                    name: s.name,
                    description: s.description,
                    parameters: s.parameters,
                })
                .collect();

            let completion = self
                .model
                .complete(CompletionRequest {
                    messages: ctx.messages().to_vec(),
                    tools: tool_schemas,
                })
                .await
                .map_err(HarnessError::Model)?;

            match completion.response {
                ModelResponse::Text(answer) => {
                    self.tracer
                        .record(TraceEvent::FinalAnswer {
                            iteration: iterations,
                            content: answer.clone(),
                        })
                        .await;
                    ctx.push(Message::assistant(answer.clone()));
                    return Ok(LoopOutcome {
                        response: AgentResponse {
                            output: answer,
                            iterations,
                            tool_calls,
                        },
                        messages: ctx.messages().to_vec(),
                    });
                }
                ModelResponse::ToolCalls(calls) => {
                    if calls.is_empty() {
                        return Err(HarnessError::Model(
                            "model returned empty tool_calls".into(),
                        ));
                    }

                    ctx.push(Message::assistant_tool_calls(&calls));

                    for call in calls {
                        tool_calls += 1;
                        info!(tool = %call.name, "executing tool");
                        self.tracer
                            .record(TraceEvent::ToolCall {
                                iteration: iterations,
                                name: call.name.clone(),
                                arguments: call.arguments.clone(),
                            })
                            .await;

                        let observation = self.execute_tool(&call).await?;
                        self.tracer
                            .record(TraceEvent::ToolResult {
                                iteration: iterations,
                                name: call.name.clone(),
                                result: observation.clone(),
                            })
                            .await;

                        ctx.push(Message::tool(observation, call.id));
                    }
                }
            }
        }
    }

    async fn execute_tool(&self, call: &ToolCallRef) -> Result<String> {
        self.tools
            .execute(&call.name, &call.arguments)
            .await
            .map_err(HarnessError::Tool)
    }

    fn default_system_prompt(&self) -> String {
        "You are a helpful agent. Use available tools when needed. Think step by step.".into()
    }
}
