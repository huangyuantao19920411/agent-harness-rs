use std::collections::HashMap;
use crate::skills::config::SkillsConfig;
use crate::skills::loader::{discover_skill_files, parse_skill_file};
use crate::skills::Skill;
use crate::skills::SkillError;

/// Loaded skills indexed by name (Codex progressive disclosure).
#[derive(Debug, Default)]
pub struct SkillRegistry {
    skills: HashMap<String, Skill>,
    errors: Vec<String>,
}

impl SkillRegistry {
    pub fn load(config: &SkillsConfig) -> Self {
        let mut registry = Self::default();
        if !config.enabled {
            return registry;
        }

        for root in &config.search_paths {
            if !root.exists() {
                continue;
            }
            for path in discover_skill_files(root) {
                match std::fs::read_to_string(&path) {
                    Ok(content) => match parse_skill_file(&path, &content) {
                        Ok(skill) => {
                            registry.skills.insert(skill.name.clone(), skill);
                        }
                        Err(e) => registry.errors.push(format!("{}: {e}", path.display())),
                    },
                    Err(e) => registry.errors.push(format!("{}: {e}", path.display())),
                }
            }
        }
        registry
    }

    pub fn from_skills(skills: impl IntoIterator<Item = Skill>) -> Self {
        let mut registry = Self::default();
        for skill in skills {
            registry.skills.insert(skill.name.clone(), skill);
        }
        registry
    }

    pub fn len(&self) -> usize {
        self.skills.len()
    }

    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    pub fn errors(&self) -> &[String] {
        &self.errors
    }

    pub fn list(&self) -> Vec<&Skill> {
        let mut skills: Vec<_> = self.skills.values().collect();
        skills.sort_by(|a, b| a.name.cmp(&b.name));
        skills
    }

    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.skills.get(name)
    }

    /// Metadata block for system prompt (name + description + path only).
    pub fn format_catalog(&self) -> String {
        if self.skills.is_empty() {
            return String::new();
        }

        let mut lines = vec![
            "## Skills".into(),
            "The following skills are available. Use the `load_skill` tool to read full instructions when a task matches a skill description.".into(),
            String::new(),
        ];

        for skill in self.list() {
            lines.push(format!(
                "- **{}**: {} (file: {})",
                skill.name,
                skill.description,
                skill.path.display()
            ));
        }

        lines.join("\n")
    }

    /// Full skill content for agent consumption.
    pub fn load_content(&self, name: &str) -> Result<String, SkillError> {
        let skill = self
            .get(name)
            .ok_or_else(|| SkillError::NotFound(name.to_string()))?;

        Ok(format!(
            "# Skill: {}\n\n{}\n\n---\n\n{}",
            skill.name, skill.description, skill.body
        ))
    }
}
