//! Multi-Agent orchestration: task decomposition and subagent delegation.

mod orchestrator;
mod runner;

pub use orchestrator::{Orchestrator, SubAgentTask, TaskResult};
pub use runner::{MultiAgentOutcome, MultiAgentRunner};
