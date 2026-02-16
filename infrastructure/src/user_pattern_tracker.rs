//! User Pattern Tracker - learns user preferences and behavior patterns
//!
//! Tracks user-specific patterns including:
//! - Preferred commands for task types
//! - Direction patterns (redirects, clarifications)
//! - Confirmation preferences
//! - Common workflows

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, Result as SqliteResult};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPreference {
    pub id: i64,
    pub preference_key: String,
    pub preference_value: String,
    pub category: PreferenceCategory,
    pub confidence: f32,
    pub usage_count: i32,
    pub last_used: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreferenceCategory {
    CommandPreference,
    DirectionPattern,
    WorkflowPreference,
    ConfirmationStyle,
    ToolPreference,
}

impl PreferenceCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            PreferenceCategory::CommandPreference => "command_preference",
            PreferenceCategory::DirectionPattern => "direction_pattern",
            PreferenceCategory::WorkflowPreference => "workflow_preference",
            PreferenceCategory::ConfirmationStyle => "confirmation_style",
            PreferenceCategory::ToolPreference => "tool_preference",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "command_preference" => Some(PreferenceCategory::CommandPreference),
            "direction_pattern" => Some(PreferenceCategory::DirectionPattern),
            "workflow_preference" => Some(PreferenceCategory::WorkflowPreference),
            "confirmation_style" => Some(PreferenceCategory::ConfirmationStyle),
            "tool_preference" => Some(PreferenceCategory::ToolPreference),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectionPattern {
    pub pattern: String,
    pub pattern_type: DirectionType,
    pub count: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DirectionType {
    Redirect,
    Clarification,
    Question,
    Skip,
    Abort,
}

impl DirectionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            DirectionType::Redirect => "redirect",
            DirectionType::Clarification => "clarification",
            DirectionType::Question => "question",
            DirectionType::Skip => "skip",
            DirectionType::Abort => "abort",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "redirect" => Some(DirectionType::Redirect),
            "clarification" => Some(DirectionType::Clarification),
            "question" => Some(DirectionType::Question),
            "skip" => Some(DirectionType::Skip),
            "abort" => Some(DirectionType::Abort),
            _ => None,
        }
    }
}

pub struct UserPatternTracker {
    conn: Arc<Mutex<Connection>>,
}

impl UserPatternTracker {
    pub async fn new<P: AsRef<Path>>(db_path: P) -> SqliteResult<Self> {
        let db_path = db_path.as_ref().to_path_buf();
        
        let conn = match tokio::task::spawn_blocking(move || {
            Connection::open(&db_path)
        }).await {
            Ok(Ok(conn)) => conn,
            Ok(Err(e)) => return Err(e),
            Err(e) => return Err(rusqlite::Error::InvalidParameterName(e.to_string())),
        };

        let tracker = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        tracker.init_tables().await?;
        Ok(tracker)
    }

    async fn init_tables(&self) -> SqliteResult<()> {
        let conn = self.conn.lock().await;
        conn.execute_batch(
            "
            PRAGMA journal_mode=WAL;
            PRAGMA synchronous=NORMAL;
            
            CREATE TABLE IF NOT EXISTS user_preferences (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                preference_key TEXT NOT NULL,
                preference_value TEXT NOT NULL,
                category TEXT NOT NULL,
                confidence REAL DEFAULT 0.5,
                usage_count INTEGER DEFAULT 1,
                last_used TEXT NOT NULL,
                created_at TEXT NOT NULL,
                UNIQUE(preference_key, category)
            );
            
            CREATE TABLE IF NOT EXISTS direction_patterns (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                pattern TEXT NOT NULL,
                pattern_type TEXT NOT NULL,
                count INTEGER DEFAULT 1,
                last_seen TEXT NOT NULL,
                created_at TEXT NOT NULL,
                UNIQUE(pattern)
            );
            
            CREATE TABLE IF NOT EXISTS command_preferences (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_type TEXT NOT NULL,
                preferred_command TEXT NOT NULL,
                count INTEGER DEFAULT 1,
                success_rate REAL DEFAULT 0.5,
                last_used TEXT NOT NULL,
                created_at TEXT NOT NULL,
                UNIQUE(task_type, preferred_command)
            );
            
            CREATE INDEX IF NOT EXISTS idx_preferences_category ON user_preferences(category);
            CREATE INDEX IF NOT EXISTS idx_direction_patterns_type ON direction_patterns(pattern_type);
            CREATE INDEX IF NOT EXISTS idx_command_preferences_task ON command_preferences(task_type);
            ",
        )?;
        Ok(())
    }

    pub async fn record_command_preference(
        &self,
        task_type: &str,
        command: &str,
        success: bool,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock().await;
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO command_preferences (task_type, preferred_command, count, success_rate, last_used, created_at)
             VALUES (?1, ?2, 1, ?3, ?4, ?4)
             ON CONFLICT(task_type, preferred_command) DO UPDATE SET
                count = count + 1,
                success_rate = (success_rate * count + ?3) / (count + 1),
                last_used = ?4",
            params![task_type, command, if success { 1.0 } else { 0.0 }, now],
        )?;
        Ok(())
    }

    pub async fn get_command_preference(
        &self,
        task_type: &str,
    ) -> SqliteResult<Option<String>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT preferred_command FROM command_preferences
             WHERE task_type = ?1 AND count >= 2
             ORDER BY success_rate DESC, count DESC
             LIMIT 1",
        )?;
        
        let result = stmt.query_row(params![task_type], |row| {
            row.get::<_, String>(0)
        });
        
        match result {
            Ok(cmd) => Ok(Some(cmd)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub async fn record_direction(&self, direction: &str) -> SqliteResult<DirectionType> {
        let direction_type = Self::classify_direction(direction);
        let conn = self.conn.lock().await;
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO direction_patterns (pattern, pattern_type, count, last_seen, created_at)
             VALUES (?1, ?2, 1, ?3, ?3)
             ON CONFLICT(pattern) DO UPDATE SET
                count = count + 1,
                last_seen = ?3",
            params![direction, direction_type.as_str(), now],
        )?;
        
        Ok(direction_type)
    }

    fn classify_direction(input: &str) -> DirectionType {
        let lower = input.to_lowercase();
        
        if lower.contains("actually")
            || lower.contains("instead")
            || lower.contains("let's try")
            || lower.contains("try ")
            || lower.contains("run ")
        {
            return DirectionType::Redirect;
        }
        
        if lower.contains("what")
            || lower.contains("how")
            || lower.contains("why")
            || lower.contains("?")
        {
            return DirectionType::Question;
        }
        
        if lower.contains("no ")
            || lower.contains("not ")
            || lower.contains("but ")
            || lower.contains("however")
        {
            return DirectionType::Clarification;
        }
        
        if lower == "skip" || lower == "/skip" {
            return DirectionType::Skip;
        }
        
        if lower == "abort" || lower == "/abort" {
            return DirectionType::Abort;
        }
        
        DirectionType::Clarification
    }

    pub async fn get_dominant_direction(&self) -> SqliteResult<Option<DirectionType>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT pattern_type FROM direction_patterns
             WHERE count >= 3
             ORDER BY count DESC
             LIMIT 1",
        )?;
        
        let result = stmt.query_row([], |row| {
            row.get::<_, String>(0)
        });
        
        match result {
            Ok(s) => Ok(DirectionType::from_str(&s)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub async fn record_preference(
        &self,
        key: &str,
        value: &str,
        category: PreferenceCategory,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock().await;
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO user_preferences (preference_key, preference_value, category, confidence, usage_count, last_used, created_at)
             VALUES (?1, ?2, ?3, 0.6, 1, ?4, ?4)
             ON CONFLICT(preference_key, category) DO UPDATE SET
                usage_count = usage_count + 1,
                confidence = MIN(1.0, confidence + 0.05),
                last_used = ?4,
                preference_value = ?2",
            params![key, value, category.as_str(), now],
        )?;
        Ok(())
    }

    pub async fn get_preferences(
        &self,
        category: PreferenceCategory,
    ) -> SqliteResult<Vec<UserPreference>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, preference_key, preference_value, category, confidence, usage_count, last_used, created_at
             FROM user_preferences
             WHERE category = ?1 AND usage_count >= 2
             ORDER BY confidence DESC, usage_count DESC",
        )?;
        
        let prefs = stmt.query_map(params![category.as_str()], |row| {
            Ok(UserPreference {
                id: row.get(0)?,
                preference_key: row.get(1)?,
                preference_value: row.get(2)?,
                category: PreferenceCategory::from_str(&row.get::<_, String>(3)?)
                    .unwrap_or(PreferenceCategory::CommandPreference),
                confidence: row.get(4)?,
                usage_count: row.get(5)?,
                last_used: DateTime::parse_from_rfc3339(&row.get::<_, String>(6)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(7)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })?.collect::<Result<Vec<_>, _>>()?;
        
        Ok(prefs)
    }

    pub async fn cleanup_old_patterns(&self, days: i64) -> SqliteResult<usize> {
        let conn = self.conn.lock().await;
        let cutoff = (Utc::now() - chrono::Duration::days(days)).to_rfc3339();
        
        let deleted = conn.execute(
            "DELETE FROM direction_patterns WHERE last_seen < ?1 AND count < 3",
            params![cutoff],
        )?;
        
        Ok(deleted)
    }
}

impl Clone for UserPatternTracker {
    fn clone(&self) -> Self {
        Self {
            conn: Arc::clone(&self.conn),
        }
    }
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
        dir.join(format!("user_patterns_{}.db", nanos))
    }

    #[tokio::test]
    async fn test_tracker_creation() {
        let path = test_db_path();
        let _ = std::fs::remove_file(&path);
        
        let tracker = UserPatternTracker::new(&path).await.unwrap();
        
        assert!(path.exists());
        
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn test_command_preference() {
        let path = test_db_path();
        let _ = std::fs::remove_file(&path);
        
        let tracker = UserPatternTracker::new(&path).await.unwrap();
        
        tracker.record_command_preference("debug_nginx", "systemctl status nginx", true).await.unwrap();
        tracker.record_command_preference("debug_nginx", "systemctl status nginx", true).await.unwrap();
        
        let pref = tracker.get_command_preference("debug_nginx").await.unwrap();
        assert!(pref.is_some());
        assert_eq!(pref.unwrap(), "systemctl status nginx");
        
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn test_direction_classification() {
        let path = test_db_path();
        let _ = std::fs::remove_file(&path);
        
        let tracker = UserPatternTracker::new(&path).await.unwrap();
        
        let dtype = tracker.record_direction("actually let's try systemctl restart").await.unwrap();
        assert_eq!(dtype, DirectionType::Redirect);
        
        let dtype = tracker.record_direction("what does this do?").await.unwrap();
        assert_eq!(dtype, DirectionType::Question);
        
        let _ = std::fs::remove_file(&path);
    }
}
