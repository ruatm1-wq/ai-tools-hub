/// Memory Engine — SQLite-backed memory store with facts, summaries, and search.
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

pub struct MemoryStore {
    db: Mutex<Connection>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Memory {
    pub id: i64,
    pub tool_id: String,
    pub session_id: Option<String>,
    pub memory_type: String, // "fact" | "summary" | "knowledge"
    pub content: String,
    pub keywords: String,    // comma-separated
    pub entities: String,    // comma-separated
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_id: String,
    pub tool_id: String,
    pub title: String,
    pub summary: String,
    pub created_at: String,
    pub updated_at: String,
}

impl MemoryStore {
    pub fn new() -> Self {
        let db_path = Self::db_path();
        let conn = Connection::open(&db_path).expect("Failed to open memory database");
        let store = MemoryStore { db: Mutex::new(conn) };
        store.init_schema();
        store
    }

    fn db_path() -> PathBuf {
        let base = std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        let dir = base.join("ai-tools-hub").join("memory");
        std::fs::create_dir_all(&dir).ok();
        dir.join("memory.db")
    }

    fn init_schema(&self) {
        let conn = self.db.lock().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS memories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tool_id TEXT NOT NULL,
                session_id TEXT,
                memory_type TEXT NOT NULL DEFAULT 'fact',
                content TEXT NOT NULL,
                keywords TEXT DEFAULT '',
                entities TEXT DEFAULT '',
                created_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
            );
            CREATE INDEX IF NOT EXISTS idx_memories_tool ON memories(tool_id);
            CREATE INDEX IF NOT EXISTS idx_memories_type ON memories(memory_type);
            CREATE TABLE IF NOT EXISTS session_summaries (
                session_id TEXT PRIMARY KEY,
                tool_id TEXT NOT NULL,
                title TEXT DEFAULT '',
                summary TEXT DEFAULT '',
                created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
            );
            CREATE INDEX IF NOT EXISTS idx_summaries_tool ON session_summaries(tool_id);"
        ).expect("Failed to initialize memory schema");
    }

    pub fn save(&self, tool_id: &str, session_id: Option<&str>,
                memory_type: &str, content: &str, keywords: &str, entities: &str) -> i64 {
        let conn = self.db.lock().unwrap();
        conn.execute(
            "INSERT INTO memories (tool_id, session_id, memory_type, content, keywords, entities)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![tool_id, session_id, memory_type, content, keywords, entities],
        ).expect("Failed to save memory");
        conn.last_insert_rowid()
    }

    pub fn search(&self, query: &str, tool_id: Option<&str>, limit: usize) -> Vec<Memory> {
        let conn = self.db.lock().unwrap();
        let pattern = format!("%{}%", query);
        let rows: Vec<Memory> = if let Some(tid) = tool_id {
            let mut stmt = conn.prepare(
                "SELECT id, tool_id, session_id, memory_type, content, keywords, entities, created_at
                 FROM memories WHERE tool_id = ?1 AND (content LIKE ?2 OR keywords LIKE ?2)
                 ORDER BY id DESC LIMIT ?3"
            ).expect("Failed to prepare search");
            stmt.query_map(params![tid, pattern, limit as i64], Self::row_to_memory)
                .expect("Failed to search")
                .filter_map(|r| r.ok())
                .collect()
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, tool_id, session_id, memory_type, content, keywords, entities, created_at
                 FROM memories WHERE content LIKE ?1 OR keywords LIKE ?1
                 ORDER BY id DESC LIMIT ?2"
            ).expect("Failed to prepare search");
            stmt.query_map(params![pattern, limit as i64], Self::row_to_memory)
                .expect("Failed to search")
                .filter_map(|r| r.ok())
                .collect()
        };
        rows
    }

    pub fn get_recent(&self, tool_id: &str, limit: usize) -> Vec<Memory> {
        let conn = self.db.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, tool_id, session_id, memory_type, content, keywords, entities, created_at
             FROM memories WHERE tool_id = ?1 ORDER BY id DESC LIMIT ?2"
        ).expect("Failed to prepare get_recent");
        stmt.query_map(params![tool_id, limit as i64], Self::row_to_memory)
            .expect("Failed to get recent")
            .filter_map(|r| r.ok())
            .collect()
    }

    pub fn delete(&self, id: i64) -> bool {
        let conn = self.db.lock().unwrap();
        conn.execute("DELETE FROM memories WHERE id = ?1", params![id])
            .map(|n| n > 0)
            .unwrap_or(false)
    }

    pub fn save_summary(&self, session_id: &str, tool_id: &str, title: &str, summary: &str) {
        let conn = self.db.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO session_summaries (session_id, tool_id, title, summary, updated_at)
             VALUES (?1, ?2, ?3, ?4, datetime('now','localtime'))",
            params![session_id, tool_id, title, summary],
        ).expect("Failed to save summary");
    }

    pub fn get_summary(&self, session_id: &str) -> Option<SessionSummary> {
        let conn = self.db.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT session_id, tool_id, title, summary, created_at, updated_at
             FROM session_summaries WHERE session_id = ?1"
        ).expect("Failed to prepare get_summary");
        stmt.query_row(params![session_id], |row| {
            Ok(SessionSummary {
                session_id: row.get(0)?,
                tool_id: row.get(1)?,
                title: row.get(2)?,
                summary: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        }).ok()
    }

    pub fn search_summaries(&self, query: &str, tool_id: Option<&str>, limit: usize) -> Vec<SessionSummary> {
        let conn = self.db.lock().unwrap();
        let pattern = format!("%{}%", query);
        if let Some(tid) = tool_id {
            let mut stmt = conn.prepare(
                "SELECT session_id, tool_id, title, summary, created_at, updated_at
                 FROM session_summaries WHERE tool_id = ?1 AND (title LIKE ?2 OR summary LIKE ?2)
                 ORDER BY updated_at DESC LIMIT ?3"
            ).expect("Failed to search summaries");
            stmt.query_map(params![tid, pattern, limit as i64], |row| {
                Ok(SessionSummary {
                    session_id: row.get(0)?, tool_id: row.get(1)?, title: row.get(2)?,
                    summary: row.get(3)?, created_at: row.get(4)?, updated_at: row.get(5)?,
                })
            }).expect("Failed to search summaries")
            .filter_map(|r| r.ok())
            .collect()
        } else {
            let mut stmt = conn.prepare(
                "SELECT session_id, tool_id, title, summary, created_at, updated_at
                 FROM session_summaries WHERE title LIKE ?1 OR summary LIKE ?1
                 ORDER BY updated_at DESC LIMIT ?2"
            ).expect("Failed to search summaries");
            stmt.query_map(params![pattern, limit as i64], |row| {
                Ok(SessionSummary {
                    session_id: row.get(0)?, tool_id: row.get(1)?, title: row.get(2)?,
                    summary: row.get(3)?, created_at: row.get(4)?, updated_at: row.get(5)?,
                })
            }).expect("Failed to search summaries")
            .filter_map(|r| r.ok())
            .collect()
        }
    }

    fn row_to_memory(row: &rusqlite::Row) -> rusqlite::Result<Memory> {
        Ok(Memory {
            id: row.get(0)?,
            tool_id: row.get(1)?,
            session_id: row.get(2)?,
            memory_type: row.get(3)?,
            content: row.get(4)?,
            keywords: row.get(5)?,
            entities: row.get(6)?,
            created_at: row.get(7)?,
        })
    }
}
