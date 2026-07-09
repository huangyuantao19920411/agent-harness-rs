use tracing::{info, warn};

use crate::config::MemoryConfig;
use crate::entry::{ExtractedMemory, MemoryKind};
use crate::error::Result;
use crate::store::MemoryStore;

/// System prompt for memory extraction (phase 1 of Codex memory pipeline).
pub const MEMORY_EXTRACTION_PROMPT: &str = "\
You are a memory extractor for a software agent. \
Review the session transcript and extract durable facts worth remembering across future sessions.

Output ONLY a JSON array (no markdown fences), max 5 items:
[{\"kind\":\"fact|preference|task|error\",\"content\":\"...\",\"importance\":0.0-1.0}]

Extract:
- User preferences and constraints
- Project paths, tech stack, conventions
- Completed tasks and unresolved issues
- Errors encountered and their fixes

Skip transient tool output and greetings.";

/// Format recalled memories for injection into the agent system prompt.
pub fn format_memories_for_prompt(memories: &[crate::entry::MemoryEntry]) -> String {
    if memories.is_empty() {
        return String::new();
    }

    let lines: Vec<String> = memories
        .iter()
        .map(|m| format!("- [{}] {}", m.kind.as_str(), m.content))
        .collect();

    format!(
        "Relevant memories from prior sessions:\n{}\n\nUse these as context but verify if unsure.",
        lines.join("\n")
    )
}

/// Parse LLM JSON output into extracted memories.
pub fn parse_extracted_memories(text: &str, max_items: usize) -> Result<Vec<ExtractedMemory>> {
    let trimmed = text.trim();

    if let Ok(items) = serde_json::from_str::<Vec<ExtractedMemory>>(trimmed) {
        return Ok(items.into_iter().take(max_items).collect());
    }

    if let Some(start) = trimmed.find('[') {
        if let Some(end) = trimmed.rfind(']') {
            let slice = &trimmed[start..=end];
            if let Ok(items) = serde_json::from_str::<Vec<ExtractedMemory>>(slice) {
                return Ok(items.into_iter().take(max_items).collect());
            }
        }
    }

    warn!("failed to parse memory extraction JSON, using fallback");
    Ok(vec![ExtractedMemory {
        kind: MemoryKind::Fact,
        content: trimmed.chars().take(500).collect(),
        importance: 0.4,
    }])
}

/// Phase 2: persist extracted memories to SQLite.
pub fn persist_extracted(
    store: &MemoryStore,
    session_id: &str,
    items: &[ExtractedMemory],
    source_turn: Option<u32>,
) -> Result<Vec<crate::entry::MemoryEntry>> {
    let mut saved = Vec::new();
    for item in items {
        let entry = store.insert(
            session_id,
            item.kind,
            &item.content,
            source_turn,
            item.importance.clamp(0.0, 1.0),
        )?;
        saved.push(entry);
    }
    Ok(saved)
}

/// Recall memories for session start.
pub fn recall_for_session(
    store: &MemoryStore,
    session_id: &str,
    config: &MemoryConfig,
) -> Result<Vec<crate::entry::MemoryEntry>> {
    if !config.enabled {
        return Ok(vec![]);
    }
    store.recall(session_id, config)
}

/// Persist pre-parsed extracted memories (phase 2 only).
pub fn store_extracted(
    store: &MemoryStore,
    session_id: &str,
    items: &[ExtractedMemory],
    source_turn: Option<u32>,
    config: &MemoryConfig,
) -> Result<usize> {
    if !config.extract_on_complete || items.is_empty() {
        return Ok(0);
    }
    let saved = persist_extracted(store, session_id, items, source_turn)?;
    info!(count = saved.len(), session_id, "memories persisted");
    Ok(saved.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::MemoryEntry;

    #[test]
    fn parses_json_array() {
        let text = r#"[{"kind":"fact","content":"Uses Rust","importance":0.9}]"#;
        let items = parse_extracted_memories(text, 5).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].content, "Uses Rust");
    }

    #[test]
    fn format_memories_block() {
        let mem = MemoryEntry {
            id: "1".into(),
            session_id: "s".into(),
            kind: MemoryKind::Preference,
            content: "prefers async Rust".into(),
            source_turn: None,
            created_at: "now".into(),
            importance: 0.7,
        };
        let block = format_memories_for_prompt(&[mem]);
        assert!(block.contains("prior sessions"));
        assert!(block.contains("prefers async Rust"));
    }
}
