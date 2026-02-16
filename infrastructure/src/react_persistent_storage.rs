use async_trait::async_trait;
use rusqlite::{params, Connection, OptionalExtension};
use std::sync::Mutex;

use domain::entities::react::{ProposedCommand, ReactSession, ReactStatus, ReactStep};
use domain::repositories::react_repository::{ReactCommandRepository, ReactRepository};
use std::error::Error;

pub struct SqliteReactStorage {
    conn: Mutex<Connection>,
}

impl SqliteReactStorage {
    pub fn new(db_path: &str) -> Result<Self, Box<dyn Error>> {
        let conn = Connection::open(db_path)?;
        let storage = Self {
            conn: Mutex::new(conn),
        };
        storage.init_tables()?;
        Ok(storage)
    }

    fn init_tables(&self) -> Result<(), Box<dyn Error>> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS react_sessions (
                id TEXT PRIMARY KEY,
                query TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                context_json TEXT,
                compacted_summary TEXT,
                neurosymbolic_enabled INTEGER NOT NULL DEFAULT 0
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS react_steps (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                step_type TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL,
                status TEXT NOT NULL,
                reasoning TEXT,
                observations_json TEXT,
                commands_json TEXT,
                FOREIGN KEY (session_id) REFERENCES react_sessions(id)
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS session_embeddings (
                session_id TEXT PRIMARY KEY,
                embedding BLOB NOT NULL,
                indexed_at TEXT NOT NULL,
                FOREIGN KEY (session_id) REFERENCES react_sessions(id)
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_steps_session ON react_steps(session_id)",
            [],
        )?;

        Ok(())
    }

    fn do_save_session(&self, session: &ReactSession) -> Result<(), Box<dyn Error>> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let context_json = serde_json::to_string(&session.context).unwrap_or_default();
        
        conn.execute(
            "INSERT OR REPLACE INTO react_sessions 
             (id, query, status, created_at, updated_at, context_json, compacted_summary, neurosymbolic_enabled)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                session.id,
                session.query,
                format!("{:?}", session.status),
                session.created_at.to_rfc3339(),
                session.updated_at.to_rfc3339(),
                context_json,
                session.compacted_summary,
                session.neurosymbolic_enabled as i32,
            ],
        )?;
        Ok(())
    }

    fn do_get_session(&self, session_id: &str) -> Result<Option<ReactSession>, Box<dyn Error>> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn.prepare(
            "SELECT id, query, status, created_at, updated_at, context_json, compacted_summary, neurosymbolic_enabled
             FROM react_sessions WHERE id = ?1"
        )?;

        let session = stmt
            .query_row([session_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, i32>(7)?,
                ))
            })
            .optional()?;

        drop(stmt);
        drop(conn);

        if let Some((id, query, status, created_at, updated_at, context_json, compacted_summary, neurosymbolic_enabled)) = session {
            let context: std::collections::HashMap<String, String> = 
                serde_json::from_str(&context_json).unwrap_or_default();
            
            let status = match status.as_str() {
                "Running" => ReactStatus::Running,
                "Completed" => ReactStatus::Completed,
                "Failed" => ReactStatus::Failed,
                "Aborted" => ReactStatus::Aborted,
                _ => ReactStatus::Running,
            };

            let created = chrono::DateTime::parse_from_rfc3339(&created_at)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now());
            let updated = chrono::DateTime::parse_from_rfc3339(&updated_at)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now());

            Ok(Some(ReactSession {
                id,
                query,
                created_at: created,
                updated_at: updated,
                status,
                steps: Vec::new(),
                context,
                memory: domain::entities::react_memory::SessionMemory::default(),
                intent: None,
                compacted_summary,
                neurosymbolic_enabled: neurosymbolic_enabled != 0,
            }))
        } else {
            Ok(None)
        }
    }

    fn do_delete_session(&self, session_id: &str) -> Result<(), Box<dyn Error>> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM session_embeddings WHERE session_id = ?1", [session_id])?;
        conn.execute("DELETE FROM react_steps WHERE session_id = ?1", [session_id])?;
        conn.execute("DELETE FROM react_sessions WHERE id = ?1", [session_id])?;
        Ok(())
    }

    fn do_save_step(&self, step: &ReactStep) -> Result<(), Box<dyn Error>> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let observations_json = serde_json::to_string(&step.observations).unwrap_or_default();
        let commands_json = serde_json::to_string(&step.commands).unwrap_or_default();

        conn.execute(
            "INSERT OR REPLACE INTO react_steps 
             (id, session_id, step_type, content, created_at, status, reasoning, observations_json, commands_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                step.id,
                step.session_id,
                format!("{:?}", step.step_type),
                step.content,
                step.created_at.to_rfc3339(),
                format!("{:?}", step.status),
                step.reasoning,
                observations_json,
                commands_json,
            ],
        )?;
        Ok(())
    }

    fn do_get_steps(&self, session_id: &str) -> Result<Vec<ReactStep>, Box<dyn Error>> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn.prepare(
            "SELECT id, session_id, step_type, content, created_at, status, reasoning, observations_json, commands_json
             FROM react_steps WHERE session_id = ?1 ORDER BY created_at"
        )?;

        let step_rows: Vec<(String, String, String, String, String, String, Option<String>, String, String)> = stmt
            .query_map([session_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        drop(stmt);
        drop(conn);

        let mut result = Vec::new();
        for (id, session_id, step_type, content, created_at, status, reasoning, observations_json, commands_json) in step_rows {
            let step_type = match step_type.as_str() {
                "Thought" => domain::entities::react::ReactStepType::Thought,
                "Action" => domain::entities::react::ReactStepType::Action,
                "Observation" => domain::entities::react::ReactStepType::Observation,
                "Verify" => domain::entities::react::ReactStepType::Verify,
                "Complete" => domain::entities::react::ReactStepType::Complete,
                _ => domain::entities::react::ReactStepType::Thought,
            };

            let status = match status.as_str() {
                "Pending" => domain::entities::react::ReactStepStatus::Pending,
                "InProgress" => domain::entities::react::ReactStepStatus::InProgress,
                "Completed" => domain::entities::react::ReactStepStatus::Completed,
                "Failed" => domain::entities::react::ReactStepStatus::Failed,
                "Skipped" => domain::entities::react::ReactStepStatus::Skipped,
                _ => domain::entities::react::ReactStepStatus::Pending,
            };

            let created = chrono::DateTime::parse_from_rfc3339(&created_at)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now());

            let observations: Vec<String> = serde_json::from_str(&observations_json).unwrap_or_default();
            let commands: Vec<ProposedCommand> = serde_json::from_str(&commands_json).unwrap_or_default();

            result.push(ReactStep {
                id,
                session_id,
                step_type,
                content,
                created_at: created,
                status,
                reasoning,
                observations,
                commands,
            });
        }

        Ok(result)
    }

    fn do_get_recent_sessions(&self, limit: usize) -> Result<Vec<ReactSession>, Box<dyn Error>> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn.prepare(
            "SELECT id FROM react_sessions ORDER BY updated_at DESC LIMIT ?1"
        )?;

        let ids: Vec<String> = stmt
            .query_map([limit as i64], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<String>, _>>()?;

        drop(stmt);
        drop(conn);

        let mut sessions = Vec::new();
        for id in ids {
            if let Some(session) = self.do_get_session(&id)? {
                sessions.push(session);
            }
        }

        Ok(sessions)
    }

    fn do_get_sessions_by_status(&self, status: &str) -> Result<Vec<ReactSession>, Box<dyn Error>> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn.prepare(
            "SELECT id FROM react_sessions WHERE status = ?1"
        )?;

        let ids: Vec<String> = stmt
            .query_map([status], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<String>, _>>()?;

        drop(stmt);
        drop(conn);

        let mut sessions = Vec::new();
        for id in ids {
            if let Some(session) = self.do_get_session(&id)? {
                sessions.push(session);
            }
        }

        Ok(sessions)
    }
}

#[async_trait]
impl ReactRepository for SqliteReactStorage {
    async fn save_session(&self, session: &ReactSession) -> Result<(), Box<dyn Error>> {
        self.do_save_session(session)
    }

    async fn get_session(&self, session_id: &str) -> Result<Option<ReactSession>, Box<dyn Error>> {
        self.do_get_session(session_id)
    }

    async fn update_session(&self, session: &ReactSession) -> Result<(), Box<dyn Error>> {
        self.do_save_session(session)
    }

    async fn delete_session(&self, session_id: &str) -> Result<(), Box<dyn Error>> {
        self.do_delete_session(session_id)
    }

    async fn save_step(&self, step: &ReactStep) -> Result<(), Box<dyn Error>> {
        self.do_save_step(step)
    }

    async fn get_steps(&self, session_id: &str) -> Result<Vec<ReactStep>, Box<dyn Error>> {
        self.do_get_steps(session_id)
    }

    async fn update_step(&self, step: &ReactStep) -> Result<(), Box<dyn Error>> {
        self.do_save_step(step)
    }

    async fn get_recent_sessions(&self, limit: usize) -> Result<Vec<ReactSession>, Box<dyn Error>> {
        self.do_get_recent_sessions(limit)
    }

    async fn get_sessions_by_status(&self, status: &str) -> Result<Vec<ReactSession>, Box<dyn Error>> {
        self.do_get_sessions_by_status(status)
    }
}

#[async_trait]
impl ReactCommandRepository for SqliteReactStorage {
    async fn save_command(&self, _command: &ProposedCommand) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    async fn update_command(&self, _command: &ProposedCommand) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    async fn get_commands_by_step(&self, _step_id: &str) -> Result<Vec<ProposedCommand>, Box<dyn Error>> {
        Ok(Vec::new())
    }

    async fn get_pending_commands(&self, _step_id: &str) -> Result<Vec<ProposedCommand>, Box<dyn Error>> {
        Ok(Vec::new())
    }
}

impl SqliteReactStorage {
    pub async fn save_session_embedding(&self, session_id: &str, embedding: &[f32]) -> Result<(), Box<dyn Error>> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let vector_bytes = bincode::serialize(embedding).map_err(|e| e.to_string())?;
        
        conn.execute(
            "INSERT OR REPLACE INTO session_embeddings (session_id, embedding, indexed_at) VALUES (?1, ?2, ?3)",
            params![
                session_id,
                vector_bytes,
                chrono::Utc::now().to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub async fn get_session_embedding(&self, session_id: &str) -> Result<Option<Vec<f32>>, Box<dyn Error>> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn.prepare("SELECT embedding FROM session_embeddings WHERE session_id = ?1")?;
        
        let embedding = stmt
            .query_row([session_id], |row| row.get::<_, Vec<u8>>(0))
            .optional()?;

        drop(stmt);
        drop(conn);

        if let Some(bytes) = embedding {
            let vector: Vec<f32> = bincode::deserialize(&bytes).map_err(|e| e.to_string())?;
            Ok(Some(vector))
        } else {
            Ok(None)
        }
    }

    pub async fn get_all_embeddings(&self) -> Result<Vec<(String, Vec<f32>)>, Box<dyn Error>> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn.prepare("SELECT session_id, embedding FROM session_embeddings")?;
        
        let rows: Vec<(String, Vec<u8>)> = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        drop(stmt);
        drop(conn);

        let mut results = Vec::new();
        for (session_id, bytes) in rows {
            let vector: Vec<f32> = bincode::deserialize(&bytes).map_err(|e| e.to_string())?;
            results.push((session_id, vector));
        }

        Ok(results)
    }
}
