use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// SQLite episodic memory configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    pub enabled: bool,
    /// SQLite database file path.
    pub db_path: PathBuf,
    /// Max memories injected into system prompt at session start.
    pub max_recall: usize,
    /// Extract and persist memories when a turn completes successfully.
    pub extract_on_complete: bool,
    /// Recall memories from all sessions (not only current session_id).
    pub global_recall: bool,
    /// Max new memories extracted per session completion.
    pub max_extract: usize,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            db_path: PathBuf::from(".agent/memory.db"),
            max_recall: 8,
            extract_on_complete: true,
            global_recall: true,
            max_extract: 5,
        }
    }
}

impl MemoryConfig {
    pub fn enabled_at(path: impl AsRef<Path>) -> Self {
        Self {
            enabled: true,
            db_path: path.as_ref().to_path_buf(),
            ..Default::default()
        }
    }

    pub fn from_env() -> Self {
        if let Ok(path) = std::env::var("MEMORY_PATH") {
            return Self::enabled_at(path);
        }
        Self::default()
    }
}
