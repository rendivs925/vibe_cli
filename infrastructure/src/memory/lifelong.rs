use rusqlite::{params, Connection, OptionalExtension};
use std::error::Error;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug, Clone)]
pub struct LifelongEntry {
    pub id: i64,
    pub content: String,
    pub timestamp: String,
    pub score: f32,
}

#[derive(Debug, Clone)]
pub struct PatternEntry {
    pub id: i64,
    pub pattern: String,
    pub success_count: i32,
    pub failure_count: i32,
    pub confidence: f32,
}

pub struct LifelongMemoryStore {
    conn: Mutex<Connection>,
}

impl LifelongMemoryStore {
    pub fn new(path: PathBuf) -> Result<Self, Box<dyn Error>> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_tables()?;
        Ok(store)
    }

    fn init_tables(&self) -> Result<(), Box<dyn Error>> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS lifelong_knowledge (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content TEXT NOT NULL,
                embedding BLOB,
                timestamp TEXT NOT NULL
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS lifelong_knowledge_fts USING fts5(
                content,
                content='lifelong_knowledge',
                content_rowid='id'
            );

            CREATE TRIGGER IF NOT EXISTS lifelong_ai AFTER INSERT ON lifelong_knowledge BEGIN
                INSERT INTO lifelong_knowledge_fts(rowid, content) VALUES (new.id, new.content);
            END;

            CREATE TRIGGER IF NOT EXISTS lifelong_ad AFTER DELETE ON lifelong_knowledge BEGIN
                INSERT INTO lifelong_knowledge_fts(lifelong_knowledge_fts, rowid, content)
                VALUES('delete', old.id, old.content);
            END;

            CREATE TRIGGER IF NOT EXISTS lifelong_au AFTER UPDATE ON lifelong_knowledge BEGIN
                INSERT INTO lifelong_knowledge_fts(lifelong_knowledge_fts, rowid, content)
                VALUES('delete', old.id, old.content);
                INSERT INTO lifelong_knowledge_fts(rowid, content) VALUES (new.id, new.content);
            END;

            CREATE TABLE IF NOT EXISTS entities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                type TEXT NOT NULL,
                properties TEXT
            );

            CREATE TABLE IF NOT EXISTS relationships (
                from_id INTEGER NOT NULL,
                to_id INTEGER NOT NULL,
                relation_type TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS learned_patterns (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                pattern TEXT NOT NULL,
                success_count INTEGER DEFAULT 0,
                failure_count INTEGER DEFAULT 0,
                confidence REAL DEFAULT 0.0,
                created_at TEXT NOT NULL
            );
            ",
        )?;
        Ok(())
    }

    pub fn remember(&self, content: &str) -> Result<i64, Box<dyn Error>> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let timestamp = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO lifelong_knowledge (content, timestamp) VALUES (?1, ?2)",
            params![content, timestamp],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<LifelongEntry>, Box<dyn Error>> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn.prepare(
            "SELECT k.id, k.content, k.timestamp, bm25(lifelong_knowledge_fts) as score
             FROM lifelong_knowledge_fts
             JOIN lifelong_knowledge k ON k.id = lifelong_knowledge_fts.rowid
             WHERE lifelong_knowledge_fts MATCH ?1
             ORDER BY score
             LIMIT ?2",
        )?;

        let rows = stmt
            .query_map(params![query, limit as i64], |row| {
                let score: f64 = row.get(3)?;
                Ok(LifelongEntry {
                    id: row.get(0)?,
                    content: row.get(1)?,
                    timestamp: row.get(2)?,
                    score: score as f32,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    pub fn forget(&self, query_or_id: &str) -> Result<usize, Box<dyn Error>> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        if let Ok(id) = query_or_id.parse::<i64>() {
            let count = conn.execute("DELETE FROM lifelong_knowledge WHERE id = ?1", [id])?;
            return Ok(count);
        }
        let like = format!("%{}%", query_or_id);
        let count = conn.execute(
            "DELETE FROM lifelong_knowledge WHERE content LIKE ?1",
            [like],
        )?;
        Ok(count)
    }

    pub fn add_pattern(
        &self,
        pattern: &str,
        success_count: i32,
        failure_count: i32,
        confidence: f32,
    ) -> Result<i64, Box<dyn Error>> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let created_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO learned_patterns (pattern, success_count, failure_count, confidence, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![pattern, success_count, failure_count, confidence, created_at],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn search_patterns(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<PatternEntry>, Box<dyn Error>> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn.prepare(
            "SELECT id, pattern, success_count, failure_count, confidence
             FROM learned_patterns
             WHERE pattern LIKE ?1
             ORDER BY confidence DESC
             LIMIT ?2",
        )?;
        let like = format!("%{}%", query);
        let rows = stmt
            .query_map(params![like, limit as i64], |row| {
                let confidence: f64 = row.get(4)?;
                Ok(PatternEntry {
                    id: row.get(0)?,
                    pattern: row.get(1)?,
                    success_count: row.get(2)?,
                    failure_count: row.get(3)?,
                    confidence: confidence as f32,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn get_pattern(&self, id: i64) -> Result<Option<PatternEntry>, Box<dyn Error>> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn.prepare(
            "SELECT id, pattern, success_count, failure_count, confidence
             FROM learned_patterns WHERE id = ?1",
        )?;
        let entry = stmt
            .query_row([id], |row| {
                let confidence: f64 = row.get(4)?;
                Ok(PatternEntry {
                    id: row.get(0)?,
                    pattern: row.get(1)?,
                    success_count: row.get(2)?,
                    failure_count: row.get(3)?,
                    confidence: confidence as f32,
                })
            })
            .optional()?;
        Ok(entry)
    }
}
