use std::sync::Arc;

use crate::compaction::{compact_context, should_compact};
use crate::guardian::GuardianReviewer;
use crate::memory::extract_and_store;
use crate::provider::{CompletionRequest, ModelProvider, ModelResponse, ToolCallRef, ToolSchemaRef};
use crate::tool_orchestrator::ToolOrchestrator;
use crate::{HarnessError, Result};
use crate::config::HarnessConfig;
use crate::context::ContextManager;
use crate::message::Message;
use crate::types::{AgentRequest, AgentResponse, LoopOutcome};
use harness_memory::{format_memories_for_prompt, recall_for_session, MemoryStore};
use harness_tools::{LoadSkillTool, ListSkillsTool, SkillRegistry, ToolRegistry};
use harness_trace::{TraceEvent, Tracer};
use tracing::info;
use uuid::Uuid;

/// ReAct-style Agent Loop: reason → act (tool) → observe → repeat.
pub struct AgentLoop<M: ModelProvider> {
    model: Arc<M>,
    orchestrator: ToolOrchestrator,
    tracer: Tracer,
    config: HarnessConfig,
    memory_store: Option<Arc<MemoryStore>>,
    skill_registry: Option<Arc<SkillRegistry>>,
}

impl<M: ModelProvider + Send + Sync + 'static> AgentLoop<M> {
    pub fn new(model: M, tools: ToolRegistry, tracer: Tracer, config: HarnessConfig) -> Self {
        Self::with_arc(Arc::new(model), tools, tracer, config)
    }

    pub fn with_arc(
        model: Arc<M>,
        mut tools: ToolRegistry,
        tracer: Tracer,
        config: HarnessConfig,
    ) -> Self {
        let skill_registry = if config.skills.enabled {
            let registry = Arc::new(SkillRegistry::load(&config.skills));
            if !registry.is_empty() {
                tools.register(Arc::new(LoadSkillTool::new(registry.clone())));
                tools.register(Arc::new(ListSkillsTool::new(registry.clone())));
            }
            for err in registry.errors() {
                tracing::warn!(%err, "invalid skill skipped");
            }
            if registry.is_empty() {
                None
            } else {
                info!(count = registry.len(), "skills loaded");
                Some(registry)
            }
        } else {
            None
        };

        let mut orchestrator = ToolOrchestrator::new(
            tools,
            config.exec_policy.clone(),
            config.guardian.clone(),
        )
        .with_tracer(tracer.clone());

        if config.guardian.enabled {
            let guardian = GuardianReviewer::new(Arc::clone(&model), config.guardian.clone());
            orchestrator = orchestrator.with_approval(Arc::new(guardian));
        }

        let memory_store = if config.memory.enabled {
            match MemoryStore::open(&config.memory.db_path) {
                Ok(store) => Some(Arc::new(store)),
                Err(e) => {
                    tracing::warn!(%e, "failed to open memory store, continuing without memory");
                    None
                }
            }
        } else {
            None
        };

        Self {
            model,
            orchestrator,
            tracer,
            config,
            memory_store,
            skill_registry,
        }
    }

    pub fn orchestrator(&self) -> &ToolOrchestrator {
        &self.orchestrator
    }

    pub fn memory_store(&self) -> Option<&MemoryStore> {
        self.memory_store.as_deref()
    }

    pub async fn run(&self, request: AgentRequest) -> Result<LoopOutcome> {
        let session_id = request
            .session_id
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        let max_msgs = self.config.max_context_tokens / 512;
        let mut ctx = ContextManager::new(max_msgs.max(4))
            .with_token_budget(self.config.max_context_tokens)
            .with_tool_result_limit(self.config.max_tool_result_chars)
            .with_auto_heuristic(!self.config.compaction.enabled);

        let mut system = request
            .system_prompt
            .unwrap_or_else(|| self.default_system_prompt());

        // Phase 0: recall episodic memories into system prompt
        if let Some(store) = &self.memory_store {
            if let Ok(memories) = recall_for_session(store, &session_id, &self.config.memory) {
                if !memories.is_empty() {
                    let block = format_memories_for_prompt(&memories);
                    system = format!("{system}\n\n{block}");
                    self.tracer
                        .record(TraceEvent::MemoryRecalled {
                            session_id: session_id.clone(),
                            count: memories.len(),
                            preview: memories
                                .first()
                                .map(|m| m.content.chars().take(80).collect())
                                .unwrap_or_default(),
                        })
                        .await;
                }
            }
        }

        // Inject skills catalog (metadata only — full body via load_skill tool)
        if self.config.skills.inject_catalog {
            if let Some(skills) = &self.skill_registry {
                let catalog = skills.format_catalog();
                if !catalog.is_empty() {
                    system = format!("{system}\n\n{catalog}");
                }
            }
        }

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

            if should_compact(&ctx, self.config.max_context_tokens, &self.config.compaction) {
                if let Some(result) = compact_context(
                    &mut ctx,
                    &self.model,
                    self.config.max_context_tokens,
                    &self.config.compaction,
                )
                .await?
                {
                    self.tracer
                        .record(TraceEvent::ContextCompacted {
                            iteration: iterations,
                            messages_before: result.messages_before,
                            messages_after: result.messages_after,
                            tokens_before: result.tokens_before,
                            tokens_after: result.tokens_after,
                            summary_preview: result.summary.chars().take(200).collect(),
                        })
                        .await;
                }
            }

            let tool_schemas: Vec<ToolSchemaRef> = self
                .orchestrator
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

                    // Phase 2: extract + persist memories after successful turn
                    if let Some(store) = &self.memory_store {
                        match extract_and_store(
                            &self.model,
                            store,
                            &session_id,
                            ctx.messages(),
                            &self.config.memory,
                            Some(iterations),
                        )
                        .await
                        {
                            Ok(count) if count > 0 => {
                                self.tracer
                                    .record(TraceEvent::MemoryPersisted {
                                        session_id: session_id.clone(),
                                        count,
                                    })
                                    .await;
                            }
                            Ok(_) => {}
                            Err(e) => info!(%e, "memory extraction skipped"),
                        }
                    }

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

                        let observation = match self.execute_tool(&call, iterations).await {
                            Ok(obs) => obs,
                            Err(HarnessError::PolicyDenied(reason)) => {
                                format!("[policy denied: {reason}]")
                            }
                            Err(e) => return Err(e),
                        };

                        self.tracer
                            .record(TraceEvent::ToolResult {
                                iteration: iterations,
                                name: call.name.clone(),
                                result: observation.clone(),
                            })
                            .await;

                        if call.name == "load_skill" {
                            if let Some(name) = call.arguments.get("name").and_then(|v| v.as_str()) {
                                if let Some(skill) = self.skill_registry.as_ref().and_then(|r| r.get(name)) {
                                    self.tracer
                                        .record(TraceEvent::SkillLoaded {
                                            name: name.to_string(),
                                            path: skill.path.display().to_string(),
                                        })
                                        .await;
                                }
                            }
                        }

                        ctx.push(Message::tool(observation, call.id));
                    }
                }
            }
        }
    }

    async fn execute_tool(&self, call: &ToolCallRef, iteration: u32) -> Result<String> {
        self.orchestrator.execute(call, iteration).await
    }

    fn default_system_prompt(&self) -> String {
        "You are a helpful agent. Use available tools when needed. Think step by step.".into()
    }
}
