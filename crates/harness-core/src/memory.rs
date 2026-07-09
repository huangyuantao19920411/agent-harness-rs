use std::sync::Arc;

use harness_memory::{
    parse_extracted_memories, store_extracted, ExtractedMemory, MemoryConfig, MemoryStore,
    MEMORY_EXTRACTION_PROMPT,
};
use tracing::info;

use crate::message::{Message, Role};
use crate::provider::{CompletionRequest, ModelProvider, ModelResponse};
use crate::Result;

/// Phase 1: LLM extracts structured memories from conversation transcript.
pub async fn extract_memories_with_model<M: ModelProvider>(
    model: &Arc<M>,
    messages: &[Message],
    max_items: usize,
) -> std::result::Result<Vec<ExtractedMemory>, String> {
    let transcript = format_messages(messages);

    let request = CompletionRequest {
        messages: vec![
            Message::system(MEMORY_EXTRACTION_PROMPT),
            Message::user(format!(
                "Extract up to {max_items} memories from:\n\n{transcript}"
            )),
        ],
        tools: vec![],
    };

    let result = model.complete(request).await?;

    let text = match result.response {
        ModelResponse::Text(t) => t,
        ModelResponse::ToolCalls(_) => {
            return Err("memory extractor returned tool_calls".into());
        }
    };

    parse_extracted_memories(&text, max_items).map_err(|e| e.to_string())
}

/// Full pipeline: extract → persist.
pub async fn extract_and_store<M: ModelProvider>(
    model: &Arc<M>,
    store: &MemoryStore,
    session_id: &str,
    messages: &[Message],
    config: &MemoryConfig,
    source_turn: Option<u32>,
) -> Result<usize> {
    if !config.enabled || !config.extract_on_complete {
        return Ok(0);
    }

    let extracted = extract_memories_with_model(model, messages, config.max_extract)
        .await
        .map_err(|e| crate::HarnessError::Memory(e))?;

    let count = store_extracted(store, session_id, &extracted, source_turn, config)
        .map_err(|e| crate::HarnessError::Memory(e.to_string()))?;

    info!(count, session_id, "memory pipeline complete");
    Ok(count)
}

fn format_messages(messages: &[Message]) -> String {
    messages
        .iter()
        .map(|m| match m.role {
            Role::System => format!("[system] {}", m.content),
            Role::User => format!("[user] {}", m.content),
            Role::Assistant => format!("[assistant] {}", m.content),
            Role::Tool => format!("[tool] {}", m.content),
        })
        .collect::<Vec<_>>()
        .join("\n")
}
