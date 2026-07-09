use serde::{Deserialize, Serialize};

use harness_memory::MemoryConfig;
use harness_tools::SkillsConfig;

use crate::compaction_config::CompactionConfig;
use crate::guardian_config::GuardianConfig;
use crate::policy::{ApprovalMode, ExecPolicy};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessConfig {
    pub max_iterations: u32,
    pub max_context_tokens: usize,
    pub max_tool_result_chars: usize,
    pub exec_policy: ExecPolicy,
    pub compaction: CompactionConfig,
    pub guardian: GuardianConfig,
    pub memory: MemoryConfig,
    pub skills: SkillsConfig,
}

impl Default for HarnessConfig {
    fn default() -> Self {
        Self {
            max_iterations: 10,
            max_context_tokens: 8192,
            max_tool_result_chars: 4000,
            exec_policy: ExecPolicy {
                mode: ApprovalMode::Prompt,
                ..ExecPolicy::default()
            },
            compaction: CompactionConfig::default(),
            guardian: GuardianConfig::default(),
            memory: MemoryConfig::default(),
            skills: SkillsConfig::default(),
        }
    }
}

impl HarnessConfig {
    /// Production-style config with Guardian + optional memory/skills from env.
    pub fn with_guardian() -> Self {
        let memory = MemoryConfig::from_env();
        let skills = SkillsConfig::from_env();
        Self {
            memory,
            skills,
            ..Default::default()
        }
    }

    /// Demo / CI: no guardian, auto-approve everything policy allows.
    pub fn permissive() -> Self {
        Self {
            exec_policy: ExecPolicy {
                mode: ApprovalMode::Auto,
                ..ExecPolicy::default()
            },
            guardian: GuardianConfig::disabled(),
            ..Default::default()
        }
    }
}
