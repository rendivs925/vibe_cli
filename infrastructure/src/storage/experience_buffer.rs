//! Experience Buffer - stores failures and learns from mistakes
//!
//! SQLite-backed storage for past interactions, tracking failed attempts
//! and user corrections to prevent repeating the same mistakes.

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, Result as SqliteResult};
use std::path::Path;

/// Entry in the experience buffer
#[derive(Debug, Clone)]
pub struct ExperienceEntry {
    pub id: i64,
    pub timestamp: DateTime<Utc>,
    pub session_id: String,
    pub query: String,
    pub attempted_command: String,
    pub failure_type: FailureType,
    pub error_message: Option<String>,
    pub user_correction: Option<String>,
    pub success: bool,
}

/// Types of failures that can occur
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FailureType {
    SafetyViolation,
    SyntaxError,
    ExecutionFailed,
    InvalidFlag,
    PermissionDenied,
    CommandNotFound,
    Timeout,
    UserCancelled,
    Other,
}

impl FailureType {
    pub fn as_str(&self) -> &'static str {
        match self {
            FailureType::SafetyViolation => "safety_violation",
            FailureType::SyntaxError => "syntax_error",
            FailureType::ExecutionFailed => "execution_failed",
            FailureType::InvalidFlag => "invalid_flag",
            FailureType::PermissionDenied => "permission_denied",
            FailureType::CommandNotFound => "command_not_found",
            FailureType::Timeout => "timeout",
            FailureType::UserCancelled => "user_cancelled",
            FailureType::Other => "other",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "safety_violation" => FailureType::SafetyViolation,
            "syntax_error" => FailureType::SyntaxError,
            "execution_failed" => FailureType::ExecutionFailed,
            "invalid_flag" => FailureType::InvalidFlag,
            "permission_denied" => FailureType::PermissionDenied,
            "command_not_found" => FailureType::CommandNotFound,
            "timeout" => FailureType::Timeout,
            "user_cancelled" => FailureType::UserCancelled,
            _ => FailureType::Other,
        }
    }
}

/// Query pattern with statistics
#[derive(Debug, Clone)]
pub struct QueryPattern {
    pub id: i64,
    pub pattern: String,
    pub normalized_query: String,
    pub success_count: i32,
    pub failure_count: i32,
    pub best_approach: Option<String>,
    pub last_accessed: DateTime<Utc>,
}

/// Main experience buffer storage
pub struct ExperienceBuffer {
    conn: Connection,
}

impl ExperienceBuffer {
    /// Initialize storage at given path
    pub fn new<P: AsRef<Path>>(db_path: P) -> SqliteResult<Self> {
        let conn = Connection::open(db_path)?;
        let storage = Self { conn };
        storage.init_tables()?;
        Ok(storage)
    }

    /// Initialize database tables
    fn init_tables(&self) -> SqliteResult<()> {
        // Experience entries table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS experience_entries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                session_id TEXT NOT NULL,
                query TEXT NOT NULL,
                attempted_command TEXT NOT NULL,
                failure_type TEXT NOT NULL,
                error_message TEXT,
                user_correction TEXT,
                success INTEGER NOT NULL DEFAULT 0
            )",
            [],
        )?;

        // Query patterns table for aggregated stats
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS query_patterns (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                pattern TEXT NOT NULL UNIQUE,
                normalized_query TEXT NOT NULL,
                success_count INTEGER NOT NULL DEFAULT 0,
                failure_count INTEGER NOT NULL DEFAULT 0,
                best_approach TEXT,
                last_accessed TEXT NOT NULL
            )",
            [],
        )?;

        // Indexes
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_exp_session ON experience_entries(session_id)",
            [],
        )?;
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_exp_query ON experience_entries(query)",
            [],
        )?;
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_exp_failure ON experience_entries(failure_type)",
            [],
        )?;
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_patterns_query ON query_patterns(normalized_query)",
            [],
        )?;

        Ok(())
    }

    /// Log a failed attempt
    pub fn log_failure(
        &self,
        session_id: &str,
        query: &str,
        attempted_command: &str,
        failure_type: FailureType,
        error_message: Option<&str>,
    ) -> SqliteResult<i64> {
        let timestamp = Utc::now().to_rfc3339();

        self.conn.execute(
            "INSERT INTO experience_entries 
             (timestamp, session_id, query, attempted_command, 
              failure_type, error_message, success)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0)",
            params![
                timestamp,
                session_id,
                query,
                attempted_command,
                failure_type.as_str(),
                error_message,
            ],
        )?;

        let id = self.conn.last_insert_rowid();

        // Update pattern statistics
        self.update_pattern_stats(query, false, None)?;

        Ok(id)
    }

    /// Log a successful execution with optional user correction
    pub fn log_success(
        &self,
        session_id: &str,
        query: &str,
        command: &str,
        user_correction: Option<&str>,
    ) -> SqliteResult<i64> {
        let timestamp = Utc::now().to_rfc3339();

        self.conn.execute(
            "INSERT INTO experience_entries 
             (timestamp, session_id, query, attempted_command, 
              failure_type, success, user_correction)
             VALUES (?1, ?2, ?3, ?4, 'none', 1, ?5)",
            params![timestamp, session_id, query, command, user_correction,],
        )?;

        let id = self.conn.last_insert_rowid();

        // Update pattern statistics
        self.update_pattern_stats(query, true, user_correction)?;

        Ok(id)
    }

    /// Update pattern statistics
    fn update_pattern_stats(
        &self,
        query: &str,
        success: bool,
        best_approach: Option<&str>,
    ) -> SqliteResult<()> {
        let normalized = Self::normalize_query(query);
        let pattern = Self::extract_pattern(query);
        let now = Utc::now().to_rfc3339();

        let success_inc: i32 = if success { 1 } else { 0 };
        let failure_inc: i32 = if success { 0 } else { 1 };

        // Try to update existing pattern
        let updated = self.conn.execute(
            "UPDATE query_patterns SET
             success_count = success_count + ?1,
             failure_count = failure_count + ?2,
             best_approach = COALESCE(?3, best_approach),
             last_accessed = ?4
             WHERE normalized_query = ?5",
            params![success_inc, failure_inc, best_approach, now, normalized],
        )?;

        // If no rows updated, insert new pattern
        if updated == 0 {
            self.conn.execute(
                "INSERT INTO query_patterns 
                 (pattern, normalized_query, success_count, failure_count, best_approach, last_accessed)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![pattern, normalized, success_inc, failure_inc, best_approach, now],
            )?;
        }

        Ok(())
    }

    /// Normalize a query for pattern matching
    fn normalize_query(query: &str) -> String {
        query
            .to_lowercase()
            .split_whitespace()
            .filter(|w| w.len() > 2) // Filter out short words
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Extract key pattern from query
    fn extract_pattern(query: &str) -> String {
        let lowercase = query.to_lowercase();
        let words: Vec<&str> = lowercase
            .split_whitespace()
            .filter(|w| {
                // Keep action words and important nouns
                matches!(
                    *w,
                    "list"
                        | "show"
                        | "delete"
                        | "create"
                        | "check"
                        | "start"
                        | "stop"
                        | "restart"
                        | "find"
                        | "search"
                        | "clean"
                        | "process"
                        | "file"
                        | "service"
                        | "log"
                        | "disk"
                        | "memory"
                        | "cpu"
                        | "user"
                        | "package"
                ) || w.len() > 3
            })
            .collect();

        words.join(" ")
    }

    /// Find similar past queries that failed
    pub fn find_similar_failures(
        &self,
        query: &str,
        limit: i64,
    ) -> SqliteResult<Vec<ExperienceEntry>> {
        let normalized = Self::normalize_query(query);
        let pattern = format!("%{}", normalized.replace(' ', "%"));

        let mut stmt = self.conn.prepare(
            "SELECT id, timestamp, session_id, query, attempted_command,
                    failure_type, error_message, user_correction, success
             FROM experience_entries
             WHERE (query LIKE ?1 OR ?2 != '') AND success = 0
             ORDER BY timestamp DESC
             LIMIT ?3",
        )?;

        let entries = stmt.query_map(params![pattern, normalized, limit], |row| {
            Ok(ExperienceEntry {
                id: row.get(0)?,
                timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(1)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                session_id: row.get(2)?,
                query: row.get(3)?,
                attempted_command: row.get(4)?,
                failure_type: FailureType::from_str(&row.get::<_, String>(5)?),
                error_message: row.get(6)?,
                user_correction: row.get(7)?,
                success: row.get::<_, i32>(8)? != 0,
            })
        })?;

        entries.collect()
    }

    /// List recent failures for induction analysis
    pub fn list_failures(&self, limit: i64) -> SqliteResult<Vec<ExperienceEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, timestamp, session_id, query, attempted_command,
                    failure_type, error_message, user_correction, success
             FROM experience_entries
             WHERE success = 0
             ORDER BY timestamp DESC
             LIMIT ?1",
        )?;

        let entries = stmt.query_map(params![limit], |row| {
            Ok(ExperienceEntry {
                id: row.get(0)?,
                timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(1)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                session_id: row.get(2)?,
                query: row.get(3)?,
                attempted_command: row.get(4)?,
                failure_type: FailureType::from_str(&row.get::<_, String>(5)?),
                error_message: row.get(6)?,
                user_correction: row.get(7)?,
                success: row.get::<_, i32>(8)? != 0,
            })
        })?;

        entries.collect()
    }

    /// Get lessons learned for a query type
    pub fn get_lessons_learned(&self, query: &str) -> SqliteResult<Vec<String>> {
        let normalized = Self::normalize_query(query);

        let mut stmt = self.conn.prepare(
            "SELECT user_correction, error_message, attempted_command
             FROM experience_entries
             WHERE normalized_query = ?1 AND success = 0 AND user_correction IS NOT NULL
             ORDER BY timestamp DESC
             LIMIT 5",
        )?;

        let lessons: Result<Vec<String>, _> = stmt
            .query_map([&normalized], |row| {
                let correction: Option<String> = row.get(0)?;
                let error: Option<String> = row.get(1)?;
                let attempted: String = row.get(2)?;

                let lesson = if let Some(corr) = correction {
                    format!("Instead of '{}', use '{}'", attempted, corr)
                } else if let Some(err) = error {
                    format!("'{}' failed with: {}", attempted, err)
                } else {
                    format!("'{}' previously failed", attempted)
                };

                Ok(lesson)
            })?
            .collect();

        lessons
    }

    /// Get best approach for a query pattern
    pub fn get_best_approach(&self, query: &str) -> SqliteResult<Option<String>> {
        let normalized = Self::normalize_query(query);

        let result = self.conn.query_row(
            "SELECT best_approach FROM query_patterns
             WHERE normalized_query = ?1 AND best_approach IS NOT NULL
             ORDER BY success_count DESC, failure_count ASC
             LIMIT 1",
            [&normalized],
            |row| row.get::<_, Option<String>>(0),
        );

        match result {
            Ok(approach) => Ok(approach),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get "Do Not Repeat" context for a query
    pub fn get_do_not_repeat_context(&self, query: &str) -> SqliteResult<Option<String>> {
        let failures = self.find_similar_failures(query, 3)?;

        if failures.is_empty() {
            return Ok(None);
        }

        let mut context = String::from("PREVIOUS FAILURES FOR SIMILAR QUERIES:\n");

        for (i, entry) in failures.iter().enumerate() {
            context.push_str(&format!("\n{}. Query: '{}'\n", i + 1, entry.query));
            context.push_str(&format!("   Attempted: '{}'\n", entry.attempted_command));
            if let Some(ref error) = entry.error_message {
                context.push_str(&format!("   Error: {}\n", error));
            }
            if let Some(ref correction) = entry.user_correction {
                context.push_str(&format!("   Correction: Use '{}'\n", correction));
            }
        }

        context.push_str("\nDO NOT REPEAT THESE APPROACHES.\n");

        Ok(Some(context))
    }

    /// Get success rate for a query pattern
    pub fn get_success_rate(&self, query: &str) -> SqliteResult<f32> {
        let normalized = Self::normalize_query(query);

        let result = self.conn.query_row(
            "SELECT success_count, failure_count FROM query_patterns
             WHERE normalized_query = ?1",
            [&normalized],
            |row| {
                let success: i32 = row.get(0)?;
                let failure: i32 = row.get(1)?;
                Ok((success, failure))
            },
        );

        match result {
            Ok((success, failure)) => {
                let total = success + failure;
                if total > 0 {
                    Ok(success as f32 / total as f32)
                } else {
                    Ok(0.5) // Unknown - neutral
                }
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(0.5),
            Err(e) => Err(e),
        }
    }

    /// Get statistics
    pub fn get_stats(&self) -> SqliteResult<(usize, usize, f32)> {
        let total_entries: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM experience_entries", [], |row| {
                    row.get(0)
                })?;

        let total_patterns: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM query_patterns", [], |row| row.get(0))?;

        let success_rate: f64 = self.conn.query_row(
            "SELECT 
                CASE WHEN COUNT(*) > 0 
                THEN CAST(SUM(CASE WHEN success = 1 THEN 1 ELSE 0 END) AS REAL) / COUNT(*)
                ELSE 0.5 
                END
             FROM experience_entries",
            [],
            |row| row.get(0),
        )?;

        Ok((
            total_entries as usize,
            total_patterns as usize,
            success_rate as f32,
        ))
    }

    /// Clear old entries (keep last N)
    pub fn clear_old_entries(&self, keep_count: i64) -> SqliteResult<usize> {
        self.conn.execute(
            "DELETE FROM experience_entries
             WHERE id NOT IN (
                 SELECT id FROM experience_entries
                 ORDER BY timestamp DESC
                 LIMIT ?1
             )",
            [keep_count],
        )
    }

    /// Clear all data
    pub fn clear_all(&self) -> SqliteResult<()> {
        self.conn.execute("DELETE FROM experience_entries", [])?;
        self.conn.execute("DELETE FROM query_patterns", [])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_db_path(prefix: &str) -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let dir = PathBuf::from(home).join(".config/vibe_cli/test_dbs");
        let _ = std::fs::create_dir_all(&dir);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        dir.join(format!("{}_{}.db", prefix, nanos))
    }

    #[test]
    fn test_init_tables() {
        let db_path = test_db_path("exp_init");
        let _ = std::fs::remove_file(&db_path);
        let buffer = ExperienceBuffer::new(db_path.clone()).unwrap();
        let (entries, patterns, _) = buffer.get_stats().unwrap();
        assert_eq!(entries, 0);
        assert_eq!(patterns, 0);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn test_log_failure() {
        let db_path = test_db_path("exp_failure");
        let _ = std::fs::remove_file(&db_path);
        let buffer = ExperienceBuffer::new(db_path.clone()).unwrap();

        buffer
            .log_failure(
                "test-session",
                "list processes",
                "ps aux",
                FailureType::ExecutionFailed,
                Some("permission denied"),
            )
            .unwrap();

        let (entries, _, _) = buffer.get_stats().unwrap();
        assert_eq!(entries, 1);

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn test_log_success() {
        let db_path = test_db_path("exp_success");
        let _ = std::fs::remove_file(&db_path);
        let buffer = ExperienceBuffer::new(db_path.clone()).unwrap();

        buffer
            .log_success(
                "test-session",
                "list processes",
                "ps aux",
                Some("ACTION(list) & TARGET(process)"),
            )
            .unwrap();

        let (entries, _, rate) = buffer.get_stats().unwrap();
        assert_eq!(entries, 1);
        assert_eq!(rate, 1.0);

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn test_find_similar_failures() {
        let db_path = test_db_path("exp_similar");
        let _ = std::fs::remove_file(&db_path);
        let buffer = ExperienceBuffer::new(db_path.clone()).unwrap();

        buffer
            .log_failure(
                "test-session",
                "list running processes",
                "invalid command",
                FailureType::CommandNotFound,
                None,
            )
            .unwrap();

        let similar = buffer
            .find_similar_failures("list all processes", 5)
            .unwrap();
        assert!(!similar.is_empty());

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn test_normalize_query() {
        assert_eq!(
            ExperienceBuffer::normalize_query("List All Processes"),
            "list all processes"
        );
        assert_eq!(
            ExperienceBuffer::normalize_query("  Clean  old   logs  "),
            "clean old logs"
        );
    }

    #[test]
    fn test_get_success_rate() {
        let db_path = test_db_path("exp_rate");
        let _ = std::fs::remove_file(&db_path);
        let buffer = ExperienceBuffer::new(db_path.clone()).unwrap();

        // Log 2 successes and 1 failure
        buffer
            .log_success("s1", "test query", "cmd1", None)
            .unwrap();
        buffer
            .log_success("s2", "test query", "cmd2", None)
            .unwrap();
        buffer
            .log_failure("s3", "test query", "cmd3", FailureType::Other, None)
            .unwrap();

        let rate = buffer.get_success_rate("test query").unwrap();
        assert!((rate - 0.666).abs() < 0.01);

        let _ = std::fs::remove_file(&db_path);
    }
}
