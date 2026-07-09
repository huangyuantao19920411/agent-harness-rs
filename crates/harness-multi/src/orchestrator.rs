use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgentTask {
    pub id: String,
    pub description: String,
    pub agent_role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: String,
    pub output: String,
    pub success: bool,
}

/// Orchestrator decomposes a goal into sub-tasks and aggregates results.
pub struct Orchestrator {
    max_subagents: usize,
}

impl Orchestrator {
    pub fn new(max_subagents: usize) -> Self {
        Self { max_subagents }
    }

    pub fn plan(&self, goal: &str) -> Vec<SubAgentTask> {
        vec![
            SubAgentTask {
                id: "1".into(),
                description: format!("Analyze goal: {goal}"),
                agent_role: "planner".into(),
            },
            SubAgentTask {
                id: "2".into(),
                description: "Execute planned steps".into(),
                agent_role: "worker".into(),
            },
        ]
        .into_iter()
        .take(self.max_subagents)
        .collect()
    }

    pub fn aggregate(&self, results: &[TaskResult]) -> String {
        results
            .iter()
            .map(|r| format!("[{}:{}] {}", r.task_id, if r.success { "ok" } else { "fail" }, r.output))
            .collect::<Vec<_>>()
            .join("\n")
    }
}
