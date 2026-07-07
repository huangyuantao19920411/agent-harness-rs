use crate::message::Message;

/// Manages the conversation context window with basic sliding-window compression.
pub struct ContextManager {
    max_messages: usize,
    messages: Vec<Message>,
}

impl ContextManager {
    pub fn new(max_messages: usize) -> Self {
        Self {
            max_messages,
            messages: Vec::new(),
        }
    }

    pub fn reset_with(&mut self, system: Message, user: Message) {
        self.messages.clear();
        self.messages.push(system);
        self.messages.push(user);
    }

    pub fn push(&mut self, message: Message) {
        self.messages.push(message);
        self.compress_if_needed();
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    fn compress_if_needed(&mut self) {
        if self.messages.len() <= self.max_messages {
            return;
        }

        let system = self.messages.first().cloned();
        let keep = self.max_messages.saturating_sub(1);
        let tail: Vec<Message> = self.messages.drain(self.messages.len() - keep..).collect();
        self.messages.clear();
        if let Some(sys) = system {
            self.messages.push(sys);
        }
        self.messages.extend(tail);
    }
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
        assert!(ctx.messages().len() <= 4);
    }
}
