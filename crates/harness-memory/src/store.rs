use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use chrono::Utc;
use rusqlite::{params, Connection};
use tracing::info;
use uuid::Uuid;

use crate::config::MemoryConfig;
use crate::entry::{MemoryEntry, MemoryKind};
use crate::error::{MemoryError, Result};

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS memories (
    id          TEXT PRIMARY KEY,
    session_id  TEXT NOT NULL,
    kind        TEXT NOT NULL,
    content     TEXT NOT NULL,
    source_turn INTEGER,
    importance  REAL NOT NULL DEFAULT 0.5,
    created_at  TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_memories_session ON memories(session_id);
CREATE INDEX IF NOT EXISTS idx_memories_created ON memories(created_at DESC);
";

/// SQLite-backed episodic memory store.
#[derive(Clone)]
pub struct MemoryStore {
    conn: Arc<Mutex<Connection>>,
    path: PathBuf,
}

impl MemoryStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                MemoryError::Other(format!("create memory dir: {e}"))
            })?;
        }

        let conn = Connection::open(path.as_ref())?;
        conn.execute_batch(SCHEMA)?;

        info!(path = ?path.as_ref(), "memory store opened");

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            path: path.as_ref().to_path_buf(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn insert(
        &self,
        session_id: &str,
        kind: MemoryKind,
        content: &str,
        source_turn: Option<u32>,
        importance: f32,
    ) -> Result<MemoryEntry> {
        let entry = MemoryEntry {
            id: Uuid::new_v4().to_string(),
            session_id: session_id.to_string(),
            kind,
            content: content.to_string(),
            source_turn,
            created_at: Utc::now().to_rfc3339(),
            importance,
        };

        let conn = self.conn.lock().map_err(|e| {
            MemoryError::Other(format!("lock poisoned: {e}"))
        })?;

        conn.execute(
            "INSERT INTO memories (id, session_id, kind, content, source_turn, importance, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                entry.id,
                entry.session_id,
                entry.kind.as_str(),
                entry.content,
                entry.source_turn,
                entry.importance,
                entry.created_at,
            ],
        )?;

        Ok(entry)
    }

    /// Recall recent memories for a session (and optionally global).
    pub fn recall(&self, session_id: &str, config: &MemoryConfig) -> Result<Vec<MemoryEntry>> {
        let conn = self.conn.lock().map_err(|e| {
            MemoryError::Other(format!("lock poisoned: {e}"))
        })?;

        let sql = if config.global_recall {
            "SELECT id, session_id, kind, content, source_turn, importance, created_at
             FROM memories
             ORDER BY importance DESC, created_at DESC
             LIMIT ?1"
        } else {
            "SELECT id, session_id, kind, content, source_turn, importance, created_at
             FROM memories
             WHERE session_id = ?1
             ORDER BY importance DESC, created_at DESC
             LIMIT ?2"
        };

        let mut stmt = conn.prepare(sql)?;

        let rows = if config.global_recall {
            stmt.query_map(params![config.max_recall as i64], row_to_entry)?
        } else {
            stmt.query_map(
                params![session_id, config.max_recall as i64],
                row_to_entry,
            )?
        };

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(MemoryError::from)
    }

    /// Keyword search across memory content (simple LIKE, no vector index yet).
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<MemoryEntry>> {
        let conn = self.conn.lock().map_err(|e| {
            MemoryError::Other(format!("lock poisoned: {e}"))
        })?;

        let pattern = format!("%{query}%");
        let mut stmt = conn.prepare(
            "SELECT id, session_id, kind, content, source_turn, importance, created_at
             FROM memories
             WHERE content LIKE ?1
             ORDER BY importance DESC, created_at DESC
             LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![pattern, limit as i64], row_to_entry)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(MemoryError::from)
    }

    pub fn count(&self) -> Result<usize> {
        let conn = self.conn.lock().map_err(|e| {
            MemoryError::Other(format!("lock poisoned: {e}"))
        })?;
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM memories", [], |r| r.get(0))?;
        Ok(n as usize)
    }
}

fn row_to_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryEntry> {
    Ok(MemoryEntry {
        id: row.get(0)?,
        session_id: row.get(1)?,
        kind: MemoryKind::parse(&row.get::<_, String>(2)?),
        content: row.get(3)?,
        source_turn: row.get::<_, Option<i64>>(4)?.map(|n| n as u32),
        importance: row.get(5)?,
        created_at: row.get(6)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn insert_and_recall() {
        let tmp = NamedTempFile::new().unwrap();
        let store = MemoryStore::open(tmp.path()).unwrap();
        store
            .insert("sess-1", MemoryKind::Fact, "User prefers Rust", None, 0.8)
            .unwrap();

        let config = MemoryConfig {
            enabled: true,
            global_recall: false,
            max_recall: 10,
            ..Default::default()
        };
        let recalled = store.recall("sess-1", &config).unwrap();
        assert_eq!(recalled.len(), 1);
        assert!(recalled[0].content.contains("Rust"));
    }
}
