use harness_core::{
    CompletionRequest, CompletionResult, ModelProvider, ModelResponse, Role, ToolCallRef,
};
use uuid::Uuid;

/// Deterministic mock model for demos and tests.
pub struct MockModel;

#[async_trait::async_trait]
impl ModelProvider for MockModel {
    async fn complete(
        &self,
        request: CompletionRequest,
    ) -> std::result::Result<CompletionResult, String> {
        let last_user = request
            .messages
            .iter()
            .rev()
            .find(|m| m.role == Role::User)
            .map(|m| m.content.to_lowercase())
            .unwrap_or_default();

        let has_tool_result = request
            .messages
            .iter()
            .any(|m| m.role == Role::Tool);

        if last_user.contains("list") && !request.tools.is_empty() && !has_tool_result {
            let tool = &request.tools[0];
            return Ok(CompletionResult {
                response: ModelResponse::ToolCalls(vec![ToolCallRef {
                    id: Uuid::new_v4().to_string(),
                    name: tool.name.clone(),
                    arguments: serde_json::json!({ "path": "." }),
                }]),
            });
        }

        Ok(CompletionResult {
            response: ModelResponse::Text(
                "Harness loop completed. Connect a real model provider for production use."
                    .into(),
            ),
        })
    }
}
