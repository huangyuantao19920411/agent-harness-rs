use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use crate::TraceEvent;

/// Append-only JSONL trace writer (Codex-style durable session log).
pub struct TraceWriter {
    file: File,
}

impl TraceWriter {
    pub fn create(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        Ok(Self { file })
    }

    pub fn write_event(&mut self, event: &TraceEvent) -> std::io::Result<()> {
        let line = serde_json::to_string(event).map_err(std::io::Error::other)?;
        writeln!(self.file, "{line}")?;
        self.file.flush()?;
        Ok(())
    }
}

/// Load trace events from a JSONL file for replay / evaluation.
pub fn load_trace(path: impl AsRef<Path>) -> std::io::Result<Vec<TraceEvent>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut events = Vec::new();

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let event: TraceEvent = serde_json::from_str(&line).map_err(std::io::Error::other)?;
        events.push(event);
    }

    Ok(events)
}

/// Summarize a trace for replay / debugging (human-readable).
pub fn replay_summary(events: &[TraceEvent]) -> String {
    let mut lines = Vec::new();
    for event in events {
        match event {
            TraceEvent::ToolCall {
                iteration,
                name,
                arguments,
            } => {
                lines.push(format!(
                    "[turn {iteration}] CALL {name}({arguments})"
                ));
            }
            TraceEvent::ToolResult {
                iteration,
                name,
                result,
            } => {
                let preview: String = result.chars().take(120).collect();
                lines.push(format!(
                    "[turn {iteration}] RESULT {name} → {preview}{}",
                    if result.len() > 120 { "..." } else { "" }
                ));
            }
            TraceEvent::FinalAnswer {
                iteration,
                content,
            } => {
                lines.push(format!("[turn {iteration}] ANSWER: {content}"));
            }
            TraceEvent::ContextCompacted {
                iteration,
                messages_before,
                messages_after,
                tokens_before,
                tokens_after,
                summary_preview,
            } => {
                lines.push(format!(
                    "[turn {iteration}] COMPACT {messages_before}→{messages_after} msgs, \
                     {tokens_before}→{tokens_after} tokens: {summary_preview}..."
                ));
            }
            TraceEvent::ToolApprovalReview {
                iteration,
                name,
                approved,
                reviewer,
                reason,
            } => {
                let verdict = if *approved { "APPROVE" } else { "DENY" };
                lines.push(format!(
                    "[turn {iteration}] GUARDIAN {verdict} {name} by {reviewer}: {reason}"
                ));
            }
            TraceEvent::MemoryRecalled {
                session_id,
                count,
                preview,
            } => {
                lines.push(format!(
                    "[memory] RECALL session={session_id} count={count}: {preview}..."
                ));
            }
            TraceEvent::MemoryPersisted { session_id, count } => {
                lines.push(format!(
                    "[memory] PERSIST session={session_id} count={count}"
                ));
            }
            TraceEvent::SkillLoaded { name, path } => {
                lines.push(format!("[skill] LOADED {name} ({path})"));
            }
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn roundtrip_jsonl() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path();

        let mut writer = TraceWriter::create(path).unwrap();
        writer
            .write_event(&TraceEvent::ToolCall {
                iteration: 1,
                name: "list_dir".into(),
                arguments: serde_json::json!({"path": "."}),
            })
            .unwrap();

        let events = load_trace(path).unwrap();
        assert_eq!(events.len(), 1);
        let summary = replay_summary(&events);
        assert!(summary.contains("list_dir"));
    }
}
