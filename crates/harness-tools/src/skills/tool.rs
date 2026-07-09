use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;

use crate::schema::ToolSchema;
use crate::skills::registry::SkillRegistry;
use crate::tool::Tool;
use crate::{Result, ToolError};

/// Tool to load full skill instructions on demand (progressive disclosure).
pub struct LoadSkillTool {
    registry: Arc<SkillRegistry>,
}

impl LoadSkillTool {
    pub fn new(registry: Arc<SkillRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl Tool for LoadSkillTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "load_skill".into(),
            description: "Load full instructions for a skill by name. Use when a user task matches a skill from the Skills catalog.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Skill name from the Skills catalog"
                    }
                },
                "required": ["name"]
            }),
        }
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<String> {
        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("name required".into()))?;

        self.registry
            .load_content(name)
            .map_err(|e| ToolError::Execution(e.to_string()))
    }
}

/// List available skills (metadata only).
pub struct ListSkillsTool {
    registry: Arc<SkillRegistry>,
}

impl ListSkillsTool {
    pub fn new(registry: Arc<SkillRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl Tool for ListSkillsTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "list_skills".into(),
            description: "List available agent skills (name, description, path).".into(),
            parameters: json!({
                "type": "object",
                "properties": {}
            }),
        }
    }

    async fn execute(&self, _args: &serde_json::Value) -> Result<String> {
        Ok(self.registry.format_catalog())
    }
}
