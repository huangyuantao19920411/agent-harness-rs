use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Skills discovery and injection configuration (Codex / Cursor compatible).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsConfig {
    pub enabled: bool,
    /// Directories recursively scanned for `SKILL.md` files.
    pub search_paths: Vec<PathBuf>,
    /// Inject skill catalog (name + description) into the system prompt.
    pub inject_catalog: bool,
}

impl Default for SkillsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            search_paths: default_search_paths(),
            inject_catalog: true,
        }
    }
}

impl SkillsConfig {
    pub fn from_env() -> Self {
        let enabled = std::env::var("SKILLS_ENABLED")
            .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or_else(|_| !std::env::var("SKILLS_PATH").is_err());

        let mut search_paths = default_search_paths();
        if let Ok(extra) = std::env::var("SKILLS_PATH") {
            for p in extra.split(':').filter(|s| !s.is_empty()) {
                search_paths.push(PathBuf::from(p));
            }
        }

        Self {
            enabled,
            search_paths,
            inject_catalog: true,
        }
    }

    pub fn enabled_with_paths(paths: impl IntoIterator<Item = impl Into<PathBuf>>) -> Self {
        Self {
            enabled: true,
            search_paths: paths.into_iter().map(Into::into).collect(),
            inject_catalog: true,
        }
    }
}

fn default_search_paths() -> Vec<PathBuf> {
    vec![
        PathBuf::from(".agents/skills"),
        PathBuf::from(".cursor/skills"),
    ]
}
