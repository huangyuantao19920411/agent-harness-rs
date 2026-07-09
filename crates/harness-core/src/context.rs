use crate::message::{Message, Role};

/// Manages conversation context with sliding-window and token-budget compression
/// (Codex-style compaction when approaching context limits).
pub struct ContextManager {
    max_messages: usize,
    max_tokens: usize,
    max_tool_result_chars: usize,
    auto_heuristic: bool,
    messages: Vec<Message>,
}

impl ContextManager {
    pub fn new(max_messages: usize) -> Self {
        Self {
            max_messages,
            max_tokens: max_messages * 512,
            max_tool_result_chars: 4000,
            auto_heuristic: true,
            messages: Vec::new(),
        }
    }

    /// When false, heuristic compression only runs via explicit `compress_heuristic()` (LLM mode).
    pub fn with_auto_heuristic(mut self, enabled: bool) -> Self {
        self.auto_heuristic = enabled;
        self
    }

    pub fn with_token_budget(mut self, max_tokens: usize) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    pub fn with_tool_result_limit(mut self, max_chars: usize) -> Self {
        self.max_tool_result_chars = max_chars;
        self
    }

    pub fn reset_with(&mut self, system: Message, user: Message) {
        self.messages.clear();
        self.messages.push(system);
        self.messages.push(user);
    }

    pub fn push(&mut self, message: Message) {
        let message = self.truncate_tool_result(message);
        self.messages.push(message);
        if self.auto_heuristic {
            self.compress_heuristic();
        }
    }

    /// Replace entire message list (used after LLM compaction).
    pub fn replace_messages(&mut self, messages: Vec<Message>) {
        self.messages = messages;
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    pub fn estimated_tokens(&self) -> usize {
        self.messages
            .iter()
            .map(estimate_message_tokens)
            .sum()
    }

    fn truncate_tool_result(&self, mut message: Message) -> Message {
        if message.role == Role::Tool && message.content.len() > self.max_tool_result_chars {
            let truncated = message.content.len() - self.max_tool_result_chars;
            message.content.truncate(self.max_tool_result_chars);
            message.content.push_str(&format!(
                "\n\n[... truncated {truncated} chars — full output in trace ...]"
            ));
        }
        message
    }

    /// Heuristic sliding-window fallback when LLM compaction is unavailable.
    pub fn compress_heuristic(&mut self) {
        let over_messages = self.messages.len() > self.max_messages;
        let over_tokens = self.estimated_tokens() > self.max_tokens;

        if !over_messages && !over_tokens {
            return;
        }

        let system = self.messages.first().cloned();
        let keep = self
            .max_messages
            .saturating_sub(2)
            .max(2)
            .min(self.messages.len().saturating_sub(1));

        if self.messages.len() <= keep + 1 {
            // Token over budget but too few messages to split — drop oldest non-system
            if self.messages.len() > 2 {
                let sys = self.messages.first().cloned();
                self.messages.remove(1);
                if let Some(s) = sys {
                    self.messages[0] = s;
                }
            }
            return;
        }

        let dropped = self.messages.len().saturating_sub(keep + 1);
        let tail: Vec<Message> = self.messages.drain(self.messages.len().saturating_sub(keep)..).collect();
        self.messages.clear();

        if let Some(sys) = system {
            self.messages.push(sys);
        }

        if dropped > 0 {
            self.messages.push(Message::system(format!(
                "[Context compacted (heuristic): {dropped} earlier messages dropped. \
                 Continue from recent tool results below.]"
            )));
        }

        self.messages.extend(tail);
    }
}

/// Rough token estimate: ~4 chars per token (Codex uses similar heuristics pre-compaction).
pub fn estimate_message_tokens(msg: &Message) -> usize {
    let base = msg.content.len() / 4 + 4;
    let tool_calls = msg
        .tool_calls
        .as_ref()
        .map(|calls| {
            calls
                .iter()
                .map(|c| c.name.len() + c.arguments.len())
                .sum::<usize>()
                / 4
        })
        .unwrap_or(0);
    base + tool_calls
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::Role;

    #[test]
    fn sliding_window_keeps_system_message() {
        let mut ctx = ContextManager::new(4);
        ctx.reset_with(Message::system("sys"), Message::user("hi"));
        ctx.push(Message::assistant("hello"));
        ctx.push(Message::user("more"));
        ctx.push(Message::assistant("ok"));

        assert_eq!(ctx.messages().first().unwrap().role, Role::System);
        assert!(ctx.messages().len() <= 5);
    }

    #[test]
    fn truncates_large_tool_results() {
        let ctx = ContextManager::new(20).with_tool_result_limit(100);
        let long = "x".repeat(200);
        let truncated = ctx.truncate_tool_result(Message::tool(long, "id"));
        assert!(truncated.content.len() < 200);
        assert!(truncated.content.contains("truncated"));
    }

    #[test]
    fn compaction_inserts_summary() {
        let mut ctx = ContextManager::new(3).with_token_budget(50);
        ctx.reset_with(Message::system("sys"), Message::user("hi"));
        for i in 0..10 {
            ctx.push(Message::assistant(format!("response {i} with enough text to exceed budget")));
        }
        let has_compact = ctx
            .messages()
            .iter()
            .any(|m| m.content.contains("Context compacted"));
        assert!(has_compact);
    }
}
