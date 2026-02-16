use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPreference {
    pub key: String,
    pub value: String,
    pub source: String,
    pub confidence: f32,
    pub updated_at: String,
}

pub struct UserPreferenceStore {
    conn: Connection,
}

impl UserPreferenceStore {
    pub fn new(db_path: &PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        let conn = Connection::open(db_path)?;
        let store = Self { conn };
        store.init_tables()?;
        Ok(store)
    }

    fn init_tables(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS user_preferences (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                source TEXT NOT NULL,
                confidence REAL NOT NULL DEFAULT 1.0,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;

        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS user_feedback (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                query TEXT NOT NULL,
                command TEXT NOT NULL,
                feedback_type TEXT NOT NULL,
                comment TEXT,
                created_at TEXT NOT NULL
            )",
            [],
        )?;

        Ok(())
    }

    pub fn set_preference(
        &self,
        key: &str,
        value: &str,
        source: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.conn.execute(
            "INSERT OR REPLACE INTO user_preferences (key, value, source, confidence, updated_at)
             VALUES (?1, ?2, ?3, 1.0, ?4)",
            params![key, value, source, chrono::Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn get_preference(
        &self,
        key: &str,
    ) -> Result<Option<UserPreference>, Box<dyn std::error::Error>> {
        let mut stmt = self.conn.prepare(
            "SELECT key, value, source, confidence, updated_at FROM user_preferences WHERE key = ?1"
        )?;

        let pref = stmt
            .query_row([key], |row| {
                Ok(UserPreference {
                    key: row.get(0)?,
                    value: row.get(1)?,
                    source: row.get(2)?,
                    confidence: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })
            .optional()?;

        Ok(pref)
    }

    pub fn get_all_preferences(&self) -> Result<Vec<UserPreference>, Box<dyn std::error::Error>> {
        let mut stmt = self
            .conn
            .prepare("SELECT key, value, source, confidence, updated_at FROM user_preferences")?;

        let prefs = stmt
            .query_map([], |row| {
                Ok(UserPreference {
                    key: row.get(0)?,
                    value: row.get(1)?,
                    source: row.get(2)?,
                    confidence: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(prefs)
    }

    pub fn record_feedback(
        &self,
        session_id: &str,
        query: &str,
        command: &str,
        feedback_type: &str,
        comment: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.conn.execute(
            "INSERT INTO user_feedback (session_id, query, command, feedback_type, comment, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                session_id,
                query,
                command,
                feedback_type,
                comment,
                chrono::Utc::now().to_rfc3339()
            ],
        )?;

        self.update_preference_from_feedback(query, command, feedback_type)?;
        Ok(())
    }

    fn update_preference_from_feedback(
        &self,
        query: &str,
        command: &str,
        feedback_type: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match feedback_type {
            "approved" => {
                self.increment_command_success(command)?;
            }
            "rejected" | "skipped" => {
                self.decrement_command_preference(command)?;
            }
            "modified" => {
                self.record_command_modification(command)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn increment_command_success(&self, command: &str) -> Result<(), Box<dyn std::error::Error>> {
        let tool = extract_tool_from_command(command);
        if !tool.is_empty() {
            let key = format!("preferred_tool:{}", tool);
            let current = self.get_preference(&key)?;
            let new_confidence = current
                .map(|p| (p.confidence + 0.1).min(1.0))
                .unwrap_or(0.8);

            self.conn.execute(
                "INSERT OR REPLACE INTO user_preferences (key, value, source, confidence, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![key, tool, "feedback", new_confidence, chrono::Utc::now().to_rfc3339()],
            )?;
        }
        Ok(())
    }

    fn decrement_command_preference(
        &self,
        command: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let tool = extract_tool_from_command(command);
        if !tool.is_empty() {
            let key = format!("preferred_tool:{}", tool);
            let current = self.get_preference(&key)?;
            let new_confidence = current
                .map(|p| (p.confidence - 0.1).max(0.1))
                .unwrap_or(0.5);

            self.conn.execute(
                "INSERT OR REPLACE INTO user_preferences (key, value, source, confidence, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![key, tool, "feedback", new_confidence, chrono::Utc::now().to_rfc3339()],
            )?;
        }
        Ok(())
    }

    fn record_command_modification(&self, command: &str) -> Result<(), Box<dyn std::error::Error>> {
        let tool = extract_tool_from_command(command);
        if !tool.is_empty() {
            let key = format!("modified_tool:{}", tool);
            let count: i32 = self
                .conn
                .query_row(
                    "SELECT COUNT(*) FROM user_preferences WHERE key = ?1",
                    [&key],
                    |row| row.get(0),
                )
                .unwrap_or(0);

            self.conn.execute(
                "INSERT OR REPLACE INTO user_preferences (key, value, source, confidence, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![key, format!("{}", count + 1), "feedback", 0.8, chrono::Utc::now().to_rfc3339()],
            )?;
        }
        Ok(())
    }

    pub fn get_preferred_tools(&self) -> Result<HashMap<String, f32>, Box<dyn std::error::Error>> {
        let mut stmt = self.conn.prepare(
            "SELECT key, confidence FROM user_preferences WHERE key LIKE 'preferred_tool:%'",
        )?;

        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, f32>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut prefs = HashMap::new();
        for (key, confidence) in rows {
            if let Some(tool) = key.strip_prefix("preferred_tool:") {
                prefs.insert(tool.to_string(), confidence);
            }
        }

        Ok(prefs)
    }
}

fn extract_tool_from_command(command: &str) -> String {
    let parts: Vec<&str> = command.split_whitespace().collect();
    if let Some(first) = parts.first() {
        let tool = first.to_lowercase();
        if tool == "shell" {
            return parts.get(1).map(|s| s.to_lowercase()).unwrap_or_default();
        }
        return tool;
    }
    String::new()
}
