//! Semantic Index - vector-based semantic search for sessions and commands
//!
//! Provides true semantic search using embeddings across sessions,
//! enabling retrieval of similar past experiences via vector similarity.

use anyhow::Context;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, Result as SqliteResult};
use shared::types::Result;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task;

/// A semantic search result for sessions
#[derive(Debug, Clone)]
pub struct SessionSearchResult {
    pub session_id: String,
    pub goal: String,
    pub similarity: f32,
    pub created_at: DateTime<Utc>,
    pub summary: Option<String>,
}

/// A semantic search result for commands
#[derive(Debug, Clone)]
pub struct CommandSearchResult {
    pub command_id: String,
    pub session_id: String,
    pub command: String,
    pub output_text: Option<String>,
    pub similarity: f32,
    pub executed_at: DateTime<Utc>,
}

/// Semantic index for cross-session search using embeddings
pub struct SemanticIndex {
    conn: Arc<Mutex<Connection>>,
    embedding_dimension: usize,
}

impl SemanticIndex {
    /// Initialize the semantic index at the given database path
    pub async fn new(db_path: impl AsRef<Path>) -> Result<Self> {
        let db_path = db_path.as_ref().to_path_buf();
        let conn = task::spawn_blocking(move || -> Result<Connection> {
            if let Some(parent) = db_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let conn = Connection::open(&db_path)?;
            Self::setup_db(&conn)?;
            Ok(conn)
        })
        .await??;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            embedding_dimension: 384, // Default for all-MiniLM-L6-v2
        })
    }

    /// Initialize with custom embedding dimension
    pub async fn with_dimension(
        db_path: impl AsRef<Path>,
        dimension: usize,
    ) -> Result<Self> {
        let mut index = Self::new(db_path).await?;
        index.embedding_dimension = dimension;
        Ok(index)
    }

    fn setup_db(conn: &Connection) -> SqliteResult<()> {
        conn.execute_batch(
            "
            PRAGMA journal_mode=WAL;
            PRAGMA synchronous=NORMAL;
            PRAGMA cache_size=-64000;
            PRAGMA temp_store=MEMORY;
            
            -- Session embeddings for semantic search
            CREATE TABLE IF NOT EXISTS semantic_sessions (
                session_id TEXT PRIMARY KEY,
                goal TEXT NOT NULL,
                summary TEXT,
                embedding BLOB NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                tags TEXT,
                success_rate REAL DEFAULT 0.0
            );
            
            -- Command embeddings for semantic search
            CREATE TABLE IF NOT EXISTS semantic_commands (
                command_id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                command TEXT NOT NULL,
                output_text TEXT,
                embedding BLOB NOT NULL,
                executed_at TEXT NOT NULL,
                exit_code INTEGER,
                success INTEGER DEFAULT 0,
                FOREIGN KEY (session_id) REFERENCES semantic_sessions(session_id)
            );
            
            -- Experience patterns with embeddings
            CREATE TABLE IF NOT EXISTS semantic_patterns (
                pattern_id TEXT PRIMARY KEY,
                pattern_text TEXT NOT NULL,
                pattern_type TEXT NOT NULL,
                embedding BLOB NOT NULL,
                success_count INTEGER DEFAULT 0,
                failure_count INTEGER DEFAULT 0,
                last_accessed TEXT NOT NULL,
                confidence REAL DEFAULT 0.0
            );
            
            -- Indexes for faster lookup
            CREATE INDEX IF NOT EXISTS idx_sem_sessions_created ON semantic_sessions(created_at);
            CREATE INDEX IF NOT EXISTS idx_sem_commands_session ON semantic_commands(session_id);
            CREATE INDEX IF NOT EXISTS idx_sem_patterns_type ON semantic_patterns(pattern_type);
            
            -- Full-text search for hybrid retrieval
            CREATE VIRTUAL TABLE IF NOT EXISTS semantic_sessions_fts USING fts5(
                session_id UNINDEXED,
                goal,
                summary,
                content='semantic_sessions',
                content_rowid='rowid'
            );
            
            -- Triggers to keep FTS index in sync
            CREATE TRIGGER IF NOT EXISTS semantic_sessions_ai AFTER INSERT ON semantic_sessions BEGIN
                INSERT INTO semantic_sessions_fts(session_id, goal, summary)
                VALUES (new.session_id, new.goal, new.summary);
            END;
            
            CREATE TRIGGER IF NOT EXISTS semantic_sessions_ad AFTER DELETE ON semantic_sessions BEGIN
                INSERT INTO semantic_sessions_fts(semantic_sessions_fts, rowid, session_id, goal, summary)
                VALUES ('delete', old.rowid, old.session_id, old.goal, old.summary);
            END;
            
            CREATE TRIGGER IF NOT EXISTS semantic_sessions_au AFTER UPDATE ON semantic_sessions BEGIN
                INSERT INTO semantic_sessions_fts(semantic_sessions_fts, rowid, session_id, goal, summary)
                VALUES ('delete', old.rowid, old.session_id, old.goal, old.summary);
                INSERT INTO semantic_sessions_fts(session_id, goal, summary)
                VALUES (new.session_id, new.goal, new.summary);
            END;
            
            -- Metadata for tracking
            CREATE TABLE IF NOT EXISTS semantic_index_meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            
            INSERT OR IGNORE INTO semantic_index_meta (key, value, updated_at)
            VALUES ('version', '1.0.0', datetime('now'));
            
            INSERT OR IGNORE INTO semantic_index_meta (key, value, updated_at)
            VALUES ('embedding_dimension', '384', datetime('now'));
            
            INSERT OR IGNORE INTO semantic_index_meta (key, value, updated_at)
            VALUES ('last_compaction', datetime('now'), datetime('now'));
            
            -- Stats for monitoring
            CREATE TABLE IF NOT EXISTS semantic_search_stats (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                query_pattern TEXT NOT NULL,
                results_count INTEGER NOT NULL,
                avg_similarity REAL NOT NULL,
                searched_at TEXT NOT NULL
            );
        ",
        )?;
        Ok(())
    }

    /// Index a session with its embedding
    pub async fn index_session(
        &self,
        session_id: &str,
        goal: &str,
        summary: Option<&str>,
        embedding: &[f32],
        tags: Option<Vec<String>>,
        success_rate: f32,
    ) -> Result<()> {
        let session_id = session_id.to_string();
        let goal = goal.to_string();
        let summary = summary.map(|s| s.to_string());
        let embedding = embedding.to_vec();
        let tags_str = tags.map(|t| t.join(","));
        let created_at = Utc::now();
        let updated_at = Utc::now();
        let conn = Arc::clone(&self.conn);

        task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            let embedding_bytes = bincode::serialize(&embedding)?;
            let tags = tags_str.unwrap_or_default();

            conn.execute(
                "INSERT OR REPLACE INTO semantic_sessions 
                 (session_id, goal, summary, embedding, created_at, updated_at, tags, success_rate)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    session_id,
                    goal,
                    summary,
                    embedding_bytes,
                    created_at.to_rfc3339(),
                    updated_at.to_rfc3339(),
                    tags,
                    success_rate,
                ],
            )?;

            Ok(())
        })
        .await?
    }

    /// Index a command with its embedding
    pub async fn index_command(
        &self,
        command_id: &str,
        session_id: &str,
        command: &str,
        output_text: Option<&str>,
        embedding: &[f32],
        exit_code: i32,
        success: bool,
    ) -> Result<()> {
        let command_id = command_id.to_string();
        let session_id = session_id.to_string();
        let command = command.to_string();
        let output_text = output_text.map(|s| s.to_string());
        let embedding = embedding.to_vec();
        let executed_at = Utc::now();
        let conn = Arc::clone(&self.conn);

        task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            let embedding_bytes = bincode::serialize(&embedding)?;

            conn.execute(
                "INSERT OR REPLACE INTO semantic_commands 
                 (command_id, session_id, command, output_text, embedding, executed_at, exit_code, success)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    command_id,
                    session_id,
                    command,
                    output_text,
                    embedding_bytes,
                    executed_at.to_rfc3339(),
                    exit_code,
                    if success { 1 } else { 0 },
                ],
            )?;

            Ok(())
        })
        .await?
    }

    /// Index an experience pattern with its embedding
    pub async fn index_pattern(
        &self,
        pattern_id: &str,
        pattern_text: &str,
        pattern_type: &str,
        embedding: &[f32],
        success_count: i32,
        failure_count: i32,
        confidence: f32,
    ) -> Result<()> {
        let pattern_id = pattern_id.to_string();
        let pattern_text = pattern_text.to_string();
        let pattern_type = pattern_type.to_string();
        let embedding = embedding.to_vec();
        let last_accessed = Utc::now();
        let conn = Arc::clone(&self.conn);

        task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            let embedding_bytes = bincode::serialize(&embedding)?;

            conn.execute(
                "INSERT OR REPLACE INTO semantic_patterns 
                 (pattern_id, pattern_text, pattern_type, embedding, success_count, failure_count, last_accessed, confidence)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    pattern_id,
                    pattern_text,
                    pattern_type,
                    embedding_bytes,
                    success_count,
                    failure_count,
                    last_accessed.to_rfc3339(),
                    confidence,
                ],
            )?;

            Ok(())
        })
        .await?
    }

    /// Search for similar sessions using vector similarity
    pub async fn search_sessions(
        &self,
        query_embedding: &[f32],
        limit: usize,
        min_similarity: f32,
    ) -> Result<Vec<SessionSearchResult>> {
        let query_embedding = query_embedding.to_vec();
        let conn = Arc::clone(&self.conn);

        let results = task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            let mut stmt = conn.prepare(
                "SELECT session_id, goal, summary, embedding, created_at, success_rate 
                 FROM semantic_sessions 
                 ORDER BY created_at DESC
                 LIMIT ?1",
            )?;

            let rows = stmt
                .query_map([limit as i64 * 10], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Vec<u8>>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, f32>(5)?,
                    ))
                })?;

            let mut results = Vec::new();
            for row in rows {
                let (session_id, goal, summary, embedding_bytes, created_at_str, _success_rate) = row?;
                let session_embedding: Vec<f32> = match bincode::deserialize(&embedding_bytes) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                let similarity = cosine_similarity(&query_embedding, &session_embedding);
                if similarity >= min_similarity {
                    let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|_| chrono::Utc::now());

                    results.push(SessionSearchResult {
                        session_id,
                        goal,
                        similarity,
                        created_at,
                        summary,
                    });
                }
            }

            // Sort by similarity descending and take limit
            results.sort_by(|a, b| {
                b.similarity
                    .partial_cmp(&a.similarity)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            results.truncate(limit);

            anyhow::Ok(results)
        })
        .await??;

        Ok(results)
    }

    /// Search for similar commands using vector similarity
    pub async fn search_commands(
        &self,
        query_embedding: &[f32],
        limit: usize,
        min_similarity: f32,
        only_successful: bool,
    ) -> Result<Vec<CommandSearchResult>> {
        let query_embedding = query_embedding.to_vec();
        let conn = Arc::clone(&self.conn);

        let results = task::spawn_blocking(move || {
            let conn = conn.blocking_lock();

            let sql = if only_successful {
                "SELECT command_id, session_id, command, output_text, embedding, executed_at, exit_code 
                 FROM semantic_commands 
                 WHERE success = 1
                 ORDER BY executed_at DESC
                 LIMIT ?1"
            } else {
                "SELECT command_id, session_id, command, output_text, embedding, executed_at, exit_code 
                 FROM semantic_commands 
                 ORDER BY executed_at DESC
                 LIMIT ?1"
            };

            let mut stmt = conn.prepare(sql)?;

            let rows = stmt
                .query_map([limit as i64 * 10], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Vec<u8>>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, i32>(6)?,
                    ))
                })?;

            let mut results = Vec::new();
            for row in rows {
                let (
                    command_id,
                    session_id,
                    command,
                    output_text,
                    embedding_bytes,
                    executed_at_str,
                    _exit_code,
                ) = row?;

                let command_embedding: Vec<f32> = match bincode::deserialize(&embedding_bytes) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                let similarity = cosine_similarity(&query_embedding, &command_embedding);
                if similarity >= min_similarity {
                    let executed_at = chrono::DateTime::parse_from_rfc3339(&executed_at_str)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|_| chrono::Utc::now());

                    results.push(CommandSearchResult {
                        command_id,
                        session_id,
                        command,
                        output_text,
                        similarity,
                        executed_at,
                    });
                }
            }

            // Sort by similarity descending and take limit
            results.sort_by(|a, b| {
                b.similarity
                    .partial_cmp(&a.similarity)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            results.truncate(limit);

            anyhow::Ok(results)
        })
        .await??;

        Ok(results)
    }

    /// Search for similar patterns using vector similarity
    pub async fn search_patterns(
        &self,
        query_embedding: &[f32],
        limit: usize,
        min_similarity: f32,
        pattern_type: Option<&str>,
    ) -> Result<Vec<(String, String, f32, f32)>> {
        // Returns: (pattern_id, pattern_text, similarity, confidence)
        let query_embedding = query_embedding.to_vec();
        let pattern_type = pattern_type.map(|s| s.to_string());
        let conn = Arc::clone(&self.conn);

        let results = task::spawn_blocking(move || {
            let conn = conn.blocking_lock();

            let sql = if pattern_type.is_some() {
                "SELECT pattern_id, pattern_text, embedding, confidence 
                 FROM semantic_patterns 
                 WHERE pattern_type = ?1
                 ORDER BY last_accessed DESC
                 LIMIT ?2"
            } else {
                "SELECT pattern_id, pattern_text, embedding, confidence 
                 FROM semantic_patterns 
                 ORDER BY last_accessed DESC
                 LIMIT ?1"
            };

            let mut results = Vec::new();
            
            if let Some(ref pt) = pattern_type {
                let mut stmt = conn.prepare(sql)?;
                let rows = stmt
                    .query_map([pt, &(limit as i64 * 10).to_string()], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, Vec<u8>>(2)?,
                            row.get::<_, f32>(3)?,
                        ))
                    })?;
                
                for row in rows {
                    let (pattern_id, pattern_text, embedding_bytes, confidence) = row?;
                    let pattern_embedding: Vec<f32> = match bincode::deserialize(&embedding_bytes) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };

                    let similarity = cosine_similarity(&query_embedding, &pattern_embedding);
                    if similarity >= min_similarity {
                        results.push((pattern_id, pattern_text, similarity, confidence));
                    }
                }
            } else {
                let mut stmt = conn.prepare(sql)?;
                let rows = stmt
                    .query_map([limit as i64 * 10], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, Vec<u8>>(2)?,
                            row.get::<_, f32>(3)?,
                        ))
                    })?;
                
                for row in rows {
                    let (pattern_id, pattern_text, embedding_bytes, confidence) = row?;
                    let pattern_embedding: Vec<f32> = match bincode::deserialize(&embedding_bytes) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };

                    let similarity = cosine_similarity(&query_embedding, &pattern_embedding);
                    if similarity >= min_similarity {
                        results.push((pattern_id, pattern_text, similarity, confidence));
                    }
                }
            }

            // Sort by similarity descending and take limit
            results.sort_by(|a, b| {
                b.2.partial_cmp(&a.2)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            results.truncate(limit);

            anyhow::Ok(results)
        })
        .await??;

        Ok(results)
    }

    /// Hybrid search combining FTS and vector similarity
    pub async fn hybrid_search_sessions(
        &self,
        query_text: &str,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<SessionSearchResult>> {
        // First, do FTS search
        let fts_results = self.fts_search_sessions(query_text, limit * 2).await?;

        // Then, do vector search
        let vector_results = self
            .search_sessions(query_embedding, limit * 2, 0.3)
            .await?;

        // Combine and deduplicate
        let mut combined: HashMap<String, SessionSearchResult> = HashMap::new();

        for result in fts_results {
            combined.insert(result.session_id.clone(), result);
        }

        for result in vector_results {
            if let Some(existing) = combined.get_mut(&result.session_id) {
                // Boost score if found in both
                existing.similarity = (existing.similarity + result.similarity) / 2.0 + 0.1;
            } else {
                combined.insert(result.session_id.clone(), result);
            }
        }

        // Sort and return top results
        let mut results: Vec<SessionSearchResult> = combined.into_values().collect();
        results.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);

        Ok(results)
    }

    /// Full-text search for sessions
    async fn fts_search_sessions(
        &self,
        query_text: &str,
        limit: usize,
    ) -> Result<Vec<SessionSearchResult>> {
        let query_text = query_text.to_string();
        let conn = Arc::clone(&self.conn);

        let results = task::spawn_blocking(move || {
            let conn = conn.blocking_lock();

            // Use FTS5 to find matching sessions
            let mut stmt = conn.prepare(
                "SELECT s.session_id, s.goal, s.summary, s.created_at 
                 FROM semantic_sessions_fts fts
                 JOIN semantic_sessions s ON fts.session_id = s.session_id
                 WHERE semantic_sessions_fts MATCH ?1
                 ORDER BY rank
                 LIMIT ?2",
            )?;

            let rows = stmt
                .query_map([&query_text, &(limit as i64).to_string()], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                })?;

            let mut results = Vec::new();
            for row in rows {
                let (session_id, goal, summary, created_at_str) = row?;
                let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now());

                results.push(SessionSearchResult {
                    session_id,
                    goal,
                    similarity: 0.8, // FTS match baseline
                    created_at,
                    summary,
                });
            }

            anyhow::Ok(results)
        })
        .await??;

        Ok(results)
    }

    /// Get statistics about the index
    pub async fn get_stats(&self) -> Result<(usize, usize, usize)> {
        let conn = Arc::clone(&self.conn);

        let (session_count, command_count, pattern_count) = task::spawn_blocking(move || {
            let conn = conn.blocking_lock();

            let session_count: i64 = conn
                .query_row("SELECT COUNT(*) FROM semantic_sessions", [], |row| row.get(0))?;

            let command_count: i64 = conn
                .query_row("SELECT COUNT(*) FROM semantic_commands", [], |row| row.get(0))?;

            let pattern_count: i64 = conn
                .query_row("SELECT COUNT(*) FROM semantic_patterns", [], |row| row.get(0))?;

            anyhow::Ok((
                session_count as usize,
                command_count as usize,
                pattern_count as usize,
            ))
        })
        .await??;

        Ok((session_count, command_count, pattern_count))
    }

    /// Delete old sessions (retention policy)
    pub async fn delete_old_sessions(&self, older_than: DateTime<Utc>) -> Result<usize> {
        let older_than_str = older_than.to_rfc3339();
        let conn = Arc::clone(&self.conn);

        let deleted = task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            conn.execute(
                "DELETE FROM semantic_sessions WHERE updated_at < ?1",
                [&older_than_str],
            )?;
            let deleted = conn.changes() as usize;
            anyhow::Ok(deleted)
        })
        .await??;

        Ok(deleted)
    }

    /// Compact the database
    pub async fn compact(&self) -> Result<()> {
        let conn = Arc::clone(&self.conn);

        task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            conn.execute("VACUUM", [])?;

            // Update last compaction time
            conn.execute(
                "INSERT OR REPLACE INTO semantic_index_meta (key, value, updated_at) 
                 VALUES ('last_compaction', datetime('now'), datetime('now'))",
                [],
            )?;

            anyhow::Ok(())
        })
        .await??;

        Ok(())
    }

    /// Get the embedding dimension
    pub fn embedding_dimension(&self) -> usize {
        self.embedding_dimension
    }
}

/// Calculate cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();

    let magnitude_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let magnitude_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if magnitude_a == 0.0 || magnitude_b == 0.0 {
        return 0.0;
    }

    dot_product / (magnitude_a * magnitude_b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_db_path() -> std::path::PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let dir = std::path::PathBuf::from(home).join(".config/vibe_cli/test_dbs");
        let _ = std::fs::create_dir_all(&dir);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        dir.join(format!("semantic_index_{}.db", nanos))
    }

    fn create_test_embedding(dim: usize) -> Vec<f32> {
        (0..dim).map(|i| (i as f32) / (dim as f32)).collect()
    }

    #[tokio::test]
    async fn test_index_and_search_session() {
        let db_path = test_db_path();
        let _ = std::fs::remove_file(&db_path);

        let index = SemanticIndex::new(&db_path).await.unwrap();

        // Index a session
        let session_id = "test-session-1";
        let goal = "Debug nginx performance";
        let embedding = create_test_embedding(384);

        index
            .index_session(session_id, goal, None, &embedding, None, 0.95)
            .await
            .unwrap();

        // Search for it
        let results = index
            .search_sessions(&embedding, 5, 0.5)
            .await
            .unwrap();

        assert!(!results.is_empty());
        assert_eq!(results[0].session_id, session_id);
        assert_eq!(results[0].goal, goal);
        assert!(results[0].similarity > 0.99);

        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_search_returns_similar_sessions() {
        let db_path = test_db_path();
        let _ = std::fs::remove_file(&db_path);

        let index = SemanticIndex::new(&db_path).await.unwrap();

        // Index sessions with different embeddings
        let embedding1: Vec<f32> = (0..384).map(|i| (i as f32) * 0.1).collect();
        let embedding2: Vec<f32> = (0..384).map(|i| (i as f32) * 0.1 + 0.01).collect();
        let embedding3: Vec<f32> = (0..384).map(|i| 100.0 - (i as f32) * 0.1).collect();

        index
            .index_session("session-1", "Debug nginx", None, &embedding1, None, 0.9)
            .await
            .unwrap();
        index
            .index_session("session-2", "Debug apache", None, &embedding2, None, 0.85)
            .await
            .unwrap();
        index
            .index_session("session-3", "Install packages", None, &embedding3, None, 1.0)
            .await
            .unwrap();

        // Search with embedding1 - should find session-1 and session-2
        let results = index.search_sessions(&embedding1, 5, 0.5).await.unwrap();

        assert!(results.len() >= 1);
        // Session-1 should be most similar
        assert_eq!(results[0].session_id, "session-1");

        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_stats() {
        let db_path = test_db_path();
        let _ = std::fs::remove_file(&db_path);

        let index = SemanticIndex::new(&db_path).await.unwrap();

        let (sessions, commands, patterns) = index.get_stats().await.unwrap();
        assert_eq!(sessions, 0);
        assert_eq!(commands, 0);
        assert_eq!(patterns, 0);

        let _ = std::fs::remove_file(&db_path);
    }
}
