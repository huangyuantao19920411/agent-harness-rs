use serde::{Deserialize, Serialize};

/// LLM-driven context compaction settings (Codex `/responses/compact` inspired).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionConfig {
    /// Enable LLM summarization when approaching token budget.
    pub enabled: bool,
    /// Compact when estimated tokens exceed this fraction of `max_context_tokens`.
    pub trigger_ratio: f32,
    /// Recent messages kept verbatim after compaction (tool results stay intact).
    pub keep_recent_messages: usize,
    /// Fall back to heuristic sliding-window if LLM compaction fails.
    pub fallback_heuristic: bool,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            trigger_ratio: 0.75,
            keep_recent_messages: 6,
            fallback_heuristic: true,
        }
    }
}

impl CompactionConfig {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    pub fn trigger_tokens(&self, max_context_tokens: usize) -> usize {
        ((max_context_tokens as f32) * self.trigger_ratio) as usize
    }
}
