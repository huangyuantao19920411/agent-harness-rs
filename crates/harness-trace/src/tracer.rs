use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::persist::{load_trace, replay_summary, TraceWriter};
use crate::TraceEvent;

#[derive(Clone, Default)]
pub struct Tracer {
    events: Arc<RwLock<Vec<TraceEvent>>>,
    persist_path: Arc<RwLock<Option<PathBuf>>>,
}

impl Tracer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable JSONL persistence (Codex-style durable trace).
    pub async fn enable_persistence(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        *self.persist_path.write().await = Some(path.as_ref().to_path_buf());
        Ok(())
    }

    pub async fn record(&self, event: TraceEvent) {
        if let Some(path) = self.persist_path.read().await.clone() {
            if let Ok(mut writer) = TraceWriter::create(&path) {
                let _ = writer.write_event(&event);
            }
        }
        self.events.write().await.push(event);
    }

    pub async fn events(&self) -> Vec<TraceEvent> {
        self.events.read().await.clone()
    }

    pub async fn clear(&self) {
        self.events.write().await.clear();
    }

    /// Export in-memory events to JSONL.
    pub async fn export_jsonl(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let events = self.events().await;
        let mut writer = TraceWriter::create(path)?;
        for event in &events {
            writer.write_event(event)?;
        }
        Ok(())
    }

    /// Human-readable replay summary of current session.
    pub async fn replay_summary(&self) -> String {
        replay_summary(&self.events().await)
    }

    /// Load and summarize a persisted trace file.
    pub fn replay_file(path: impl AsRef<Path>) -> std::io::Result<String> {
        let events = load_trace(path)?;
        Ok(replay_summary(&events))
    }
}
