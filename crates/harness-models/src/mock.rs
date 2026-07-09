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
        let is_compaction = request.tools.is_empty()
            && request.messages.first().map_or(false, |m| {
                m.role == Role::System && m.content.contains("conversation summarizer")
            });

        if is_compaction {
            let transcript_len = request
                .messages
                .iter()
                .map(|m| m.content.len())
                .sum::<usize>();
            return Ok(CompletionResult {
                response: ModelResponse::Text(format!(
                    "Mock compaction summary ({} chars of history).",
                    transcript_len
                )),
            });
        }

        let is_guardian = request.tools.is_empty()
            && request.messages.first().map_or(false, |m| {
                m.role == Role::System && m.content.contains("security reviewer")
            });

        if is_guardian {
            let user = request
                .messages
                .iter()
                .find(|m| m.role == Role::User)
                .map(|m| m.content.to_lowercase())
                .unwrap_or_default();

            let (decision, reason) = if user.contains("\"command\":\"rm\"")
                || user.contains("rm -rf")
                || user.contains("curl")
                || user.contains("wget")
                || user.contains("chmod")
            {
                ("DENY", "mock guardian: potentially destructive command")
            } else {
                ("APPROVE", "mock guardian: low-risk command")
            };

            return Ok(CompletionResult {
                response: ModelResponse::Text(format!(
                    "DECISION: {decision}\nREASON: {reason}"
                )),
            });
        }

        let is_memory = request.tools.is_empty()
            && request.messages.first().map_or(false, |m| {
                m.role == Role::System && m.content.contains("memory extractor")
            });

        if is_memory {
            return Ok(CompletionResult {
                response: ModelResponse::Text(
                    r#"[{"kind":"preference","content":"User prefers Rust for backend development","importance":0.85},{"kind":"fact","content":"Project uses agent-harness-rs workspace","importance":0.7}]"#.into(),
                ),
            });
        }

        let last_user = request
            .messages
            .iter()
            .rev()
            .find(|m| m.role == Role::User)
            .map(|m| m.content.to_lowercase())
            .unwrap_or_default();

        let has_tool_result = request.messages.iter().any(|m| m.role == Role::Tool);

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

#[cfg(test)]
mod tests {
    use super::*;
    use harness_core::{Message, COMPACTION_SYSTEM_PROMPT, GUARDIAN_SYSTEM_PROMPT};

    #[tokio::test]
    async fn mock_handles_compaction() {
        let model = MockModel;
        let result = model
            .complete(CompletionRequest {
                messages: vec![
                    Message::system(COMPACTION_SYSTEM_PROMPT),
                    Message::user("transcript here"),
                ],
                tools: vec![],
            })
            .await
            .unwrap();
        match result.response {
            ModelResponse::Text(s) => assert!(s.contains("Mock compaction summary")),
            _ => panic!("expected text summary"),
        }
    }

    #[tokio::test]
    async fn mock_guardian_denies_rm() {
        let model = MockModel;
        let result = model
            .complete(CompletionRequest {
                messages: vec![
                    Message::system(GUARDIAN_SYSTEM_PROMPT),
                    Message::user("Tool: sandbox_exec\nArguments: {\"command\":\"rm\",\"args\":[\"-rf\",\"/\"]}"),
                ],
                tools: vec![],
            })
            .await
            .unwrap();
        match result.response {
            ModelResponse::Text(s) => {
                assert!(s.contains("DENY"));
            }
            _ => panic!("expected text"),
        }
    }
}
