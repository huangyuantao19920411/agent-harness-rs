use std::sync::Arc;

use harness_core::{
    AgentLoop, AgentRequest, HarnessConfig, LoopOutcome, ModelProvider, Result,
};
use harness_tools::ToolRegistry;
use harness_trace::Tracer;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::orchestrator::{Orchestrator, SubAgentTask, TaskResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiAgentOutcome {
    pub goal: String,
    pub subtask_results: Vec<TaskResult>,
    pub aggregated: String,
}

/// Runs sub-agents through the shared AgentLoop (Codex-style thread delegation).
pub struct MultiAgentRunner<M: ModelProvider> {
    model: Arc<M>,
    tools: ToolRegistry,
    config: HarnessConfig,
    orchestrator: Orchestrator,
}

impl<M: ModelProvider + Send + Sync + 'static> MultiAgentRunner<M> {
    pub fn new(model: M, tools: ToolRegistry, config: HarnessConfig, max_subagents: usize) -> Self {
        Self {
            model: Arc::new(model),
            tools,
            config,
            orchestrator: Orchestrator::new(max_subagents),
        }
    }

    /// Decompose goal → run each sub-agent → aggregate (sequential execution).
    pub async fn run(&self, goal: &str) -> Result<MultiAgentOutcome> {
        let tasks = self.orchestrator.plan(goal);
        info!(count = tasks.len(), "multi-agent plan ready");

        let mut results = Vec::new();
        for task in &tasks {
            let outcome = self.run_subagent(task).await?;
            results.push(TaskResult {
                task_id: task.id.clone(),
                output: outcome.response.output,
                success: true,
            });
        }

        let aggregated = self.orchestrator.aggregate(&results);
        Ok(MultiAgentOutcome {
            goal: goal.to_string(),
            subtask_results: results,
            aggregated,
        })
    }

    async fn run_subagent(&self, task: &SubAgentTask) -> Result<LoopOutcome> {
        let tracer = Tracer::new();
        let loop_engine = AgentLoop::with_arc(
            Arc::clone(&self.model),
            self.tools.clone(),
            tracer,
            self.config.clone(),
        );

        let system = format!(
            "You are a sub-agent with role '{}'. Focus only on your assigned sub-task. \
             Use tools when needed. Be concise.",
            task.agent_role
        );

        loop_engine
            .run(AgentRequest {
                input: task.description.clone(),
                system_prompt: Some(system),
                session_id: None,
            })
            .await
    }
}
