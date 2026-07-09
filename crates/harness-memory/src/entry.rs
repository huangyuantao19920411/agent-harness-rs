use serde::{Deserialize, Serialize};

/// Kind of episodic memory entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    Fact,
    Preference,
    Task,
    Error,
}

impl MemoryKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fact => "fact",
            Self::Preference => "preference",
            Self::Task => "task",
            Self::Error => "error",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "preference" => Self::Preference,
            "task" => Self::Task,
            "error" => Self::Error,
            _ => Self::Fact,
        }
    }
}

/// A single persisted memory record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub session_id: String,
    pub kind: MemoryKind,
    pub content: String,
    pub source_turn: Option<u32>,
    pub created_at: String,
    pub importance: f32,
}

/// Payload extracted by the LLM before persistence (phase 1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedMemory {
    pub kind: MemoryKind,
    pub content: String,
    #[serde(default = "default_importance")]
    pub importance: f32,
}

fn default_importance() -> f32 {
    0.5
}
