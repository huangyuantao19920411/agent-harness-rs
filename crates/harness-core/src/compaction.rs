use std::sync::Arc;

use tracing::{info, warn};

use crate::compaction_config::CompactionConfig;
use crate::context::{estimate_message_tokens, ContextManager};
use crate::message::{Message, Role};
use crate::provider::{CompletionRequest, ModelProvider, ModelResponse};
use crate::Result;

/// System prompt for the compaction LLM call (no tools, separate from agent turn).
pub const COMPACTION_SYSTEM_PROMPT: &str = "\
You are a conversation summarizer for a software agent harness. \
Summarize the transcript below into a concise state snapshot. Preserve:
- The user's original goal and constraints
- Tool calls executed and their key outcomes
- File paths, errors, and technical decisions
- Current progress and remaining work

Write in third person past tense. Be dense and factual. Do not add advice or filler.";

/// Result of a compaction operation.
#[derive(Debug, Clone)]
pub struct CompactionResult {
    pub summary: String,
    pub messages_before: usize,
    pub messages_after: usize,
    pub tokens_before: usize,
    pub tokens_after: usize,
}

/// Returns true when LLM compaction should run before the next model call.
pub fn should_compact(ctx: &ContextManager, max_tokens: usize, config: &CompactionConfig) -> bool {
    if !config.enabled {
        return false;
    }
    ctx.estimated_tokens() >= config.trigger_tokens(max_tokens)
}

/// Split messages into (system, to_compact, recent_tail).
pub fn split_for_compaction(
    messages: &[Message],
    keep_recent: usize,
) -> Option<(Option<Message>, Vec<Message>, Vec<Message>)> {
    if messages.len() <= keep_recent + 2 {
        return None;
    }

    let system = messages.first().filter(|m| m.role == Role::System).cloned();
    let start = if system.is_some() { 1 } else { 0 };
    let split_at = messages.len().saturating_sub(keep_recent);

    if split_at <= start {
        return None;
    }

    let to_compact = messages[start..split_at].to_vec();
    let recent = messages[split_at..].to_vec();

    if to_compact.is_empty() {
        return None;
    }

    Some((system, to_compact, recent))
}

/// Format messages into a plain-text transcript for the compaction model.
pub fn format_transcript(messages: &[Message]) -> String {
    messages
        .iter()
        .map(format_message_line)
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_message_line(msg: &Message) -> String {
    match msg.role {
        Role::System => format!("[system] {}", msg.content),
        Role::User => format!("[user] {}", msg.content),
        Role::Assistant => {
            if let Some(calls) = &msg.tool_calls {
                let calls_str: Vec<String> = calls
                    .iter()
                    .map(|c| format!("{}({})", c.name, c.arguments))
                    .collect();
                format!("[assistant/tool_calls] {}", calls_str.join(", "))
            } else {
                format!("[assistant] {}", msg.content)
            }
        }
        Role::Tool => format!("[tool:{}] {}", msg.tool_call_id.as_deref().unwrap_or("?"), msg.content),
    }
}

/// Call the model to summarize `to_compact` messages (Codex compact endpoint pattern).
pub async fn compact_with_model<M: ModelProvider>(
    model: &Arc<M>,
    to_compact: &[Message],
) -> std::result::Result<String, String> {
    let transcript = format_transcript(to_compact);

    let request = CompletionRequest {
        messages: vec![
            Message::system(COMPACTION_SYSTEM_PROMPT),
            Message::user(format!("Transcript to summarize:\n\n{transcript}")),
        ],
        tools: vec![],
    };

    let result = model.complete(request).await?;

    match result.response {
        ModelResponse::Text(summary) if !summary.trim().is_empty() => Ok(summary),
        ModelResponse::Text(_) => Err("compaction model returned empty summary".into()),
        ModelResponse::ToolCalls(_) => {
            Err("compaction model unexpectedly returned tool_calls".into())
        }
    }
}

/// Run LLM compaction on a ContextManager, rebuilding message list.
pub async fn compact_context<M: ModelProvider>(
    ctx: &mut ContextManager,
    model: &Arc<M>,
    max_tokens: usize,
    config: &CompactionConfig,
) -> Result<Option<CompactionResult>> {
    if !should_compact(ctx, max_tokens, config) {
        return Ok(None);
    }

    let messages = ctx.messages().to_vec();
    let tokens_before = ctx.estimated_tokens();
    let messages_before = messages.len();

    let Some((system, to_compact, recent)) = split_for_compaction(&messages, config.keep_recent_messages)
    else {
        return Ok(None);
    };

    info!(
        messages_before,
        compacting = to_compact.len(),
        keeping = recent.len(),
        "LLM context compaction triggered"
    );

    let summary = match compact_with_model(model, &to_compact).await {
        Ok(s) => s,
        Err(e) => {
            warn!(%e, "LLM compaction failed");
            if config.fallback_heuristic {
                ctx.compress_heuristic();
                return Ok(Some(CompactionResult {
                    summary: format!("[heuristic fallback: {e}]"),
                    messages_before,
                    messages_after: ctx.messages().len(),
                    tokens_before,
                    tokens_after: ctx.estimated_tokens(),
                }));
            }
            return Err(crate::HarnessError::Context(format!(
                "compaction failed: {e}"
            )));
        }
    };

    let mut rebuilt = Vec::new();
    if let Some(sys) = system {
        rebuilt.push(sys);
    }
    rebuilt.push(Message::system(format!(
        "[Compacted context — {messages_before} messages summarized]\n\n{summary}"
    )));
    rebuilt.extend(recent);

    let tokens_after: usize = rebuilt.iter().map(estimate_message_tokens).sum();
    let messages_after = rebuilt.len();

    ctx.replace_messages(rebuilt);

    info!(
        messages_before,
        messages_after,
        tokens_before,
        tokens_after,
        "LLM context compaction complete"
    );

    Ok(Some(CompactionResult {
        summary,
        messages_before,
        messages_after,
        tokens_before,
        tokens_after,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_keeps_recent_tail() {
        let msgs = vec![
            Message::system("sys"),
            Message::user("u1"),
            Message::assistant("a1"),
            Message::user("u2"),
            Message::assistant("a2"),
            Message::user("u3"),
        ];
        let (sys, compact, recent) = split_for_compaction(&msgs, 2).unwrap();
        assert!(sys.is_some());
        assert_eq!(compact.len(), 3);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].content, "a2");
        assert_eq!(recent[1].content, "u3");
    }

    #[test]
    fn should_not_compact_when_disabled() {
        let mut ctx = ContextManager::new(4).with_token_budget(100);
        ctx.reset_with(Message::system("s"), Message::user("u"));
        for _ in 0..20 {
            ctx.push(Message::assistant("x".repeat(50)));
        }
        assert!(!should_compact(&ctx, 100, &CompactionConfig::disabled()));
    }

    #[test]
    fn format_transcript_includes_roles() {
        let t = format_transcript(&[Message::user("hello"), Message::assistant("hi")]);
        assert!(t.contains("[user] hello"));
        assert!(t.contains("[assistant] hi"));
    }
}
