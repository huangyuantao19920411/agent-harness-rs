use std::sync::Arc;

use tracing::{info, warn};

use crate::guardian_config::{ApprovalResult, GuardianConfig};
use crate::message::Message;
use crate::provider::{CompletionRequest, ModelProvider, ModelResponse};
use crate::tool_orchestrator::ApprovalHandler;

/// System prompt for Guardian tool approval review (separate LLM call, no tools).
pub const GUARDIAN_SYSTEM_PROMPT: &str = "\
You are a security reviewer for an autonomous software agent harness. \
Evaluate whether the proposed tool call is safe to execute in a sandboxed environment.

Approve ONLY if the action is:
- Read-only or low-risk (listing files, reading code, running tests)
- Consistent with typical software development tasks
- Unlikely to exfiltrate secrets or damage the system

Deny if the action:
- Deletes, overwrites, or encrypts user data destructively
- Downloads and executes remote code
- Modifies system configuration, credentials, or network settings
- Appears malicious, obfuscated, or unrelated to the stated task

Respond in EXACTLY this format (no extra text):
DECISION: APPROVE
REASON: <one sentence>

OR

DECISION: DENY
REASON: <one sentence>";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardianDecision {
    Approve { reason: String },
    Deny { reason: String },
}

/// Parse Guardian LLM output into a structured decision.
pub fn parse_guardian_response(text: &str) -> GuardianDecision {
    let upper = text.to_uppercase();

    if upper.contains("DECISION: DENY") || upper.contains("DECISION:DENY") {
        let reason = extract_reason(text).unwrap_or_else(|| "guardian denied".into());
        return GuardianDecision::Deny { reason };
    }

    if upper.contains("DECISION: APPROVE") || upper.contains("DECISION:APPROVE") {
        let reason = extract_reason(text).unwrap_or_else(|| "guardian approved".into());
        return GuardianDecision::Approve { reason };
    }

    // Conservative default: deny ambiguous responses
    GuardianDecision::Deny {
        reason: format!("ambiguous guardian response: {}", text.chars().take(120).collect::<String>()),
    }
}

fn extract_reason(text: &str) -> Option<String> {
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.to_uppercase().starts_with("REASON:") {
            return Some(trimmed[7..].trim().to_string());
        }
    }
    None
}

/// Call the model to review a tool invocation.
pub async fn review_with_model<M: ModelProvider>(
    model: &Arc<M>,
    tool_name: &str,
    arguments: &serde_json::Value,
    policy_reason: &str,
) -> std::result::Result<GuardianDecision, String> {
    let request = CompletionRequest {
        messages: vec![
            Message::system(GUARDIAN_SYSTEM_PROMPT),
            Message::user(format!(
                "Policy flag: {policy_reason}\n\
                 Tool: {tool_name}\n\
                 Arguments: {arguments}\n\n\
                 Should this tool call be approved?"
            )),
        ],
        tools: vec![],
    };

    let result = model.complete(request).await?;

    match result.response {
        ModelResponse::Text(text) => Ok(parse_guardian_response(&text)),
        ModelResponse::ToolCalls(_) => Err("guardian returned unexpected tool_calls".into()),
    }
}

/// LLM-backed approval handler (Codex Guardian-style).
pub struct GuardianReviewer<M: ModelProvider> {
    model: Arc<M>,
    config: GuardianConfig,
}

impl<M: ModelProvider> GuardianReviewer<M> {
    pub fn new(model: Arc<M>, config: GuardianConfig) -> Self {
        Self { model, config }
    }
}

#[async_trait::async_trait]
impl<M: ModelProvider + Send + Sync> ApprovalHandler for GuardianReviewer<M> {
    async fn review(
        &self,
        tool_name: &str,
        arguments: &serde_json::Value,
        policy_reason: &str,
    ) -> ApprovalResult {
        if !self.config.enabled {
            return ApprovalResult::approved("guardian-disabled", "guardian disabled, auto-approve");
        }

        info!(tool = %tool_name, "guardian: reviewing tool call");

        match review_with_model(&self.model, tool_name, arguments, policy_reason).await {
            Ok(GuardianDecision::Approve { reason }) => {
                info!(tool = %tool_name, %reason, "guardian: approve");
                ApprovalResult::approved("guardian-llm", reason)
            }
            Ok(GuardianDecision::Deny { reason }) => {
                warn!(tool = %tool_name, %reason, "guardian: deny");
                ApprovalResult::denied("guardian-llm", reason)
            }
            Err(e) => {
                warn!(tool = %tool_name, %e, "guardian: review failed, denying");
                ApprovalResult::denied("guardian-llm", format!("review error: {e}"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_approve() {
        let d = parse_guardian_response(
            "DECISION: APPROVE\nREASON: read-only ls command",
        );
        assert_eq!(
            d,
            GuardianDecision::Approve {
                reason: "read-only ls command".into()
            }
        );
    }

    #[test]
    fn parses_deny() {
        let d = parse_guardian_response(
            "DECISION: DENY\nREASON: destructive rm -rf",
        );
        assert_eq!(
            d,
            GuardianDecision::Deny {
                reason: "destructive rm -rf".into()
            }
        );
    }

    #[test]
    fn ambiguous_defaults_deny() {
        let d = parse_guardian_response("I'm not sure about this one");
        assert!(matches!(d, GuardianDecision::Deny { .. }));
    }
}
