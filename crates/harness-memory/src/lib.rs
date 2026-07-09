//! Episodic memory pipeline with SQLite persistence (Codex-inspired).
//!
//! Two-phase pipeline:
//! 1. **Extract** — LLM extracts durable facts (see `harness-core::memory`)
//! 2. **Persist** — store in SQLite for cross-session recall

mod config;
mod entry;
mod error;
mod pipeline;
mod store;

pub use config::MemoryConfig;
pub use entry::{ExtractedMemory, MemoryEntry, MemoryKind};
pub use error::{MemoryError, Result};
pub use pipeline::{
    format_memories_for_prompt, parse_extracted_memories, persist_extracted, recall_for_session,
    store_extracted, MEMORY_EXTRACTION_PROMPT,
};
pub use store::MemoryStore;
