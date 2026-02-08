//! Safety violation logging infrastructure
//!
//! SQLite-backed storage for safety violations and learning data

use domain::safety::SafetyViolation;
use rusqlite::{params, Connection, Result as SqliteResult};
use std::path::Path;

/// Storage for safety violations
pub struct SafetyViolationStorage {
    conn: Connection,
}

impl SafetyViolationStorage {
    /// Initialize storage at given path
    pub fn new<P: AsRef<Path>>(db_path: P) -> SqliteResult<Self> {
        let conn = Connection::open(db_path)?;
        let storage = Self { conn };
        storage.init_tables()?;
        Ok(storage)
    }

    /// Initialize database tables
    fn init_tables(&self) -> SqliteResult<()> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS safety_violations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                session_id TEXT,
                command TEXT NOT NULL,
                query TEXT,
                rule_id TEXT NOT NULL,
                rule_name TEXT NOT NULL,
                violation_type TEXT NOT NULL,
                description TEXT NOT NULL,
                blocked INTEGER NOT NULL,
                matched_pattern TEXT NOT NULL,
                suggestion TEXT,
                user_confirmed INTEGER,
                user_cancelled INTEGER
            )",
            [],
        )?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_violations_timestamp ON safety_violations(timestamp)",
            [],
        )?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_violations_rule_id ON safety_violations(rule_id)",
            [],
        )?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_violations_session ON safety_violations(session_id)",
            [],
        )?;

        // Create statistics table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS safety_stats (
                rule_id TEXT PRIMARY KEY,
                rule_name TEXT NOT NULL,
                match_count INTEGER NOT NULL DEFAULT 0,
                block_count INTEGER NOT NULL DEFAULT 0,
                warning_count INTEGER NOT NULL DEFAULT 0,
                last_triggered TEXT
            )",
            [],
        )?;

        Ok(())
    }

    /// Log a safety violation
    pub fn log_violation(
        &self,
        session_id: Option<&str>,
        command: &str,
        query: Option<&str>,
        violation: &SafetyViolation,
        user_confirmed: Option<bool>,
    ) -> SqliteResult<()> {
        self.conn.execute(
            "INSERT INTO safety_violations 
             (session_id, command, query, rule_id, rule_name, violation_type, 
              description, blocked, matched_pattern, suggestion, user_confirmed, user_cancelled)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                session_id,
                command,
                query,
                violation.rule_id,
                violation.rule_name,
                format!("{:?}", violation.violation_type),
                violation.description,
                violation.blocked,
                violation.matched_pattern,
                violation.suggestion,
                user_confirmed.map(|c| c as i32),
                user_confirmed.map(|c| (!c) as i32),
            ],
        )?;

        // Update statistics
        self.update_stats(violation)?;

        Ok(())
    }

    /// Update rule statistics
    fn update_stats(&self, violation: &SafetyViolation) -> SqliteResult<()> {
        let now = chrono::Local::now().to_rfc3339();

        self.conn.execute(
            "INSERT INTO safety_stats (rule_id, rule_name, match_count, block_count, warning_count, last_triggered)
             VALUES (?1, ?2, 1, ?3, ?4, ?5)
             ON CONFLICT(rule_id) DO UPDATE SET
             match_count = match_count + 1,
             block_count = block_count + ?3,
             warning_count = warning_count + ?4,
             last_triggered = ?5",
            params![
                violation.rule_id,
                violation.rule_name,
                if violation.blocked { 1 } else { 0 },
                if violation.blocked { 0 } else { 1 },
                now,
            ],
        )?;

        Ok(())
    }

    /// Get recent violations
    pub fn get_recent_violations(&self, limit: i64) -> SqliteResult<Vec<ViolationRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT timestamp, session_id, command, query, rule_id, rule_name, 
                    violation_type, description, blocked, matched_pattern, suggestion
             FROM safety_violations
             ORDER BY timestamp DESC
             LIMIT ?1",
        )?;

        let records = stmt.query_map([limit], |row| {
            Ok(ViolationRecord {
                timestamp: row.get(0)?,
                session_id: row.get(1)?,
                command: row.get(2)?,
                query: row.get(3)?,
                rule_id: row.get(4)?,
                rule_name: row.get(5)?,
                violation_type: row.get(6)?,
                description: row.get(7)?,
                blocked: row.get(8)?,
                matched_pattern: row.get(9)?,
                suggestion: row.get(10)?,
            })
        })?;

        records.collect()
    }

    /// Get statistics for all rules
    pub fn get_all_stats(&self) -> SqliteResult<Vec<RuleStats>> {
        let mut stmt = self.conn.prepare(
            "SELECT rule_id, rule_name, match_count, block_count, warning_count, last_triggered
             FROM safety_stats
             ORDER BY match_count DESC",
        )?;

        let stats = stmt.query_map([], |row| {
            Ok(RuleStats {
                rule_id: row.get(0)?,
                rule_name: row.get(1)?,
                match_count: row.get(2)?,
                block_count: row.get(3)?,
                warning_count: row.get(4)?,
                last_triggered: row.get(5)?,
            })
        })?;

        stats.collect()
    }

    /// Get violations for a specific session
    pub fn get_session_violations(&self, session_id: &str) -> SqliteResult<Vec<ViolationRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT timestamp, session_id, command, query, rule_id, rule_name, 
                    violation_type, description, blocked, matched_pattern, suggestion
             FROM safety_violations
             WHERE session_id = ?1
             ORDER BY timestamp DESC",
        )?;

        let records = stmt.query_map([session_id], |row| {
            Ok(ViolationRecord {
                timestamp: row.get(0)?,
                session_id: row.get(1)?,
                command: row.get(2)?,
                query: row.get(3)?,
                rule_id: row.get(4)?,
                rule_name: row.get(5)?,
                violation_type: row.get(6)?,
                description: row.get(7)?,
                blocked: row.get(8)?,
                matched_pattern: row.get(9)?,
                suggestion: row.get(10)?,
            })
        })?;

        records.collect()
    }

    /// Get violation count
    pub fn get_violation_count(&self) -> SqliteResult<i64> {
        let count: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM safety_violations", [], |row| {
                    row.get(0)
                })?;
        Ok(count)
    }

    /// Clear old violations (keep last N)
    pub fn clear_old_violations(&self, keep_count: i64) -> SqliteResult<usize> {
        self.conn.execute(
            "DELETE FROM safety_violations
             WHERE id NOT IN (
                 SELECT id FROM safety_violations
                 ORDER BY timestamp DESC
                 LIMIT ?1
             )",
            [keep_count],
        )
    }

    /// Clear all violations
    pub fn clear_all_violations(&self) -> SqliteResult<()> {
        self.conn.execute("DELETE FROM safety_violations", [])?;
        self.conn.execute("DELETE FROM safety_stats", [])?;
        Ok(())
    }
}

/// A recorded violation from the database
#[derive(Debug, Clone)]
pub struct ViolationRecord {
    pub timestamp: String,
    pub session_id: Option<String>,
    pub command: String,
    pub query: Option<String>,
    pub rule_id: String,
    pub rule_name: String,
    pub violation_type: String,
    pub description: String,
    pub blocked: bool,
    pub matched_pattern: String,
    pub suggestion: Option<String>,
}

/// Statistics for a safety rule
#[derive(Debug, Clone)]
pub struct RuleStats {
    pub rule_id: String,
    pub rule_name: String,
    pub match_count: i64,
    pub block_count: i64,
    pub warning_count: i64,
    pub last_triggered: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_db_path() -> PathBuf {
        PathBuf::from("/tmp/test_safety_violations.db")
    }

    #[test]
    fn test_init_tables() {
        let _ = std::fs::remove_file(test_db_path());
        let storage = SafetyViolationStorage::new(test_db_path()).unwrap();
        let count = storage.get_violation_count().unwrap();
        assert_eq!(count, 0);
        let _ = std::fs::remove_file(test_db_path());
    }

    #[test]
    fn test_log_violation() {
        let _ = std::fs::remove_file(test_db_path());
        let storage = SafetyViolationStorage::new(test_db_path()).unwrap();

        let violation = SafetyViolation::new(
            "TEST-001",
            "Test Rule",
            ViolationType::Other,
            "Test description",
            true,
            "test command",
            Some("Test suggestion"),
        );

        storage
            .log_violation(
                Some("test-session"),
                "test command",
                Some("test query"),
                &violation,
                None,
            )
            .unwrap();

        let count = storage.get_violation_count().unwrap();
        assert_eq!(count, 1);

        let violations = storage.get_recent_violations(10).unwrap();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].rule_id, "TEST-001");

        let _ = std::fs::remove_file(test_db_path());
    }
}
