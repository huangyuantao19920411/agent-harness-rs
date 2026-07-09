mod config;
mod loader;
mod registry;
mod tool;

pub use config::SkillsConfig;
pub use loader::{discover_skill_files, parse_skill_file};
pub use registry::SkillRegistry;
pub use tool::{ListSkillsTool, LoadSkillTool};

use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
    pub body: String,
}

#[derive(Debug, Error)]
pub enum SkillError {
    #[error("skill not found: {0}")]
    NotFound(String),
    #[error("invalid SKILL.md: {0}")]
    InvalidFrontmatter(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
