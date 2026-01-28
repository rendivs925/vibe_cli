use async_trait::async_trait;
use domain::repositories::execution_repository::{ExecutionRepository, CommandExecution};
use rusqlite::{params, Connection, Result as SqlResult};
use shared::error::AppError;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task;

pub struct ExecutionStorage {
    conn: Arc<Mutex<Connection>>,
}

impl ExecutionStorage {
    pub async fn new(db_path: impl AsRef<Path>) -> Result<Self, AppError> {
        let db_path = db_path.as_ref().to_path_buf();
        let conn = task::spawn_blocking(move || -> Result<Connection, AppError> {
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
        })
    }

    fn setup_db(conn: &Connection) -> SqlResult<()> {
        conn.execute_batch(
            "
            PRAGMA journal_mode=WAL;
            PRAGMA synchronous=NORMAL;
            PRAGMA cache_size=-64000;
            PRAGMA temp_store=MEMORY;
            CREATE TABLE IF NOT EXISTS command_executions (
                id TEXT PRIMARY KEY,
                command_line TEXT NOT NULL,
                executed_at TEXT NOT NULL,
                exit_code INTEGER,
                duration_ms INTEGER
            );
            CREATE INDEX IF NOT EXISTS idx_executions_command_line ON command_executions(command_line);
            CREATE INDEX IF NOT EXISTS idx_executions_executed_at ON command_executions(executed_at);
            ",
        )?;
        Ok(())
    }
}

#[async_trait]
impl ExecutionRepository for ExecutionStorage {
    async fn save(&self, execution: &CommandExecution) -> Result<(), AppError> {
        let conn = Arc::clone(&self.conn);
        let execution = execution.clone();
        
        task::spawn_blocking(move || -> Result<(), AppError> {
            let conn = conn.blocking_lock();
            conn.execute(
                "INSERT OR REPLACE INTO command_executions (id, command_line, executed_at, exit_code, duration_ms) VALUES (?, ?, ?, ?, ?)",
                params![
                    execution.id(),
                    execution.command_line(),
                    execution.executed_at().to_rfc3339(),
                    execution.exit_code(),
                    execution.duration_ms()
                ],
            )?;
            Ok(())
        }).await?
    }

    async fn get_all(&self) -> Result<Vec<CommandExecution>, AppError> {
        let conn = Arc::clone(&self.conn);
        
        task::spawn_blocking(move || -> Result<Vec<CommandExecution>, AppError> {
            let conn = conn.blocking_lock();
            let mut stmt = conn.prepare("SELECT id, command_line, executed_at, exit_code, duration_ms FROM command_executions ORDER BY executed_at DESC")?;
            let mut rows = stmt.query([])?;
            let mut executions = Vec::new();
            
            while let Some(row) = rows.next()? {
                let id: String = row.get(0)?;
                let command_line: String = row.get(1)?;
                let executed_at_str: String = row.get(2)?;
                let exit_code: Option<i32> = row.get(3)?;
                let duration_ms: Option<u64> = row.get(4)?;
                
                let executed_at = chrono::DateTime::parse_from_rfc3339(&executed_at_str)
                    .map_err(|e| AppError::database(format!("Invalid datetime format: {}", e)))?
                    .with_timezone(&chrono::Utc);
                
                let mut execution = CommandExecution::new(id, command_line, executed_at);
                if let (Some(exit_code), Some(duration_ms)) = (exit_code, duration_ms) {
                    execution = execution.with_result(exit_code, duration_ms);
                }
                
                executions.push(execution);
            }
            
            Ok(executions)
        }).await?
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<CommandExecution>, AppError> {
        let conn = Arc::clone(&self.conn);
        let id = id.to_string();
        
        task::spawn_blocking(move || -> Result<Option<CommandExecution>, AppError> {
            let conn = conn.blocking_lock();
            let mut stmt = conn.prepare("SELECT id, command_line, executed_at, exit_code, duration_ms FROM command_executions WHERE id = ?")?;
            let mut rows = stmt.query([id])?;
            
            if let Some(row) = rows.next()? {
                let id: String = row.get(0)?;
                let command_line: String = row.get(1)?;
                let executed_at_str: String = row.get(2)?;
                let exit_code: Option<i32> = row.get(3)?;
                let duration_ms: Option<u64> = row.get(4)?;
                
                let executed_at = chrono::DateTime::parse_from_rfc3339(&executed_at_str)
                    .map_err(|e| AppError::database(format!("Invalid datetime format: {}", e)))?
                    .with_timezone(&chrono::Utc);
                
                let mut execution = CommandExecution::new(id, command_line, executed_at);
                if let (Some(exit_code), Some(duration_ms)) = (exit_code, duration_ms) {
                    execution = execution.with_result(exit_code, duration_ms);
                }
                
                Ok(Some(execution))
            } else {
                Ok(None)
            }
        }).await?
    }

    async fn get_by_command(&self, command_line: &str) -> Result<Vec<CommandExecution>, AppError> {
        let conn = Arc::clone(&self.conn);
        let command_line = command_line.to_string();
        
        task::spawn_blocking(move || -> Result<Vec<CommandExecution>, AppError> {
            let conn = conn.blocking_lock();
            let mut stmt = conn.prepare("SELECT id, command_line, executed_at, exit_code, duration_ms FROM command_executions WHERE command_line = ? ORDER BY executed_at DESC")?;
            let mut rows = stmt.query([command_line])?;
            let mut executions = Vec::new();
            
            while let Some(row) = rows.next()? {
                let id: String = row.get(0)?;
                let command_line: String = row.get(1)?;
                let executed_at_str: String = row.get(2)?;
                let exit_code: Option<i32> = row.get(3)?;
                let duration_ms: Option<u64> = row.get(4)?;
                
                let executed_at = chrono::DateTime::parse_from_rfc3339(&executed_at_str)
                    .map_err(|e| AppError::database(format!("Invalid datetime format: {}", e)))?
                    .with_timezone(&chrono::Utc);
                
                let mut execution = CommandExecution::new(id, command_line, executed_at);
                if let (Some(exit_code), Some(duration_ms)) = (exit_code, duration_ms) {
                    execution = execution.with_result(exit_code, duration_ms);
                }
                
                executions.push(execution);
            }
            
            Ok(executions)
        }).await?
    }

    async fn get_by_date_range(&self, start: chrono::DateTime<chrono::Utc>, end: chrono::DateTime<chrono::Utc>) -> Result<Vec<CommandExecution>, AppError> {
        let conn = Arc::clone(&self.conn);
        let start_str = start.to_rfc3339();
        let end_str = end.to_rfc3339();
        
        task::spawn_blocking(move || -> Result<Vec<CommandExecution>, AppError> {
            let conn = conn.blocking_lock();
            let mut stmt = conn.prepare("SELECT id, command_line, executed_at, exit_code, duration_ms FROM command_executions WHERE executed_at BETWEEN ? AND ? ORDER BY executed_at DESC")?;
            let mut rows = stmt.query([start_str, end_str])?;
            let mut executions = Vec::new();
            
            while let Some(row) = rows.next()? {
                let id: String = row.get(0)?;
                let command_line: String = row.get(1)?;
                let executed_at_str: String = row.get(2)?;
                let exit_code: Option<i32> = row.get(3)?;
                let duration_ms: Option<u64> = row.get(4)?;
                
                let executed_at = chrono::DateTime::parse_from_rfc3339(&executed_at_str)
                    .map_err(|e| AppError::database(format!("Invalid datetime format: {}", e)))?
                    .with_timezone(&chrono::Utc);
                
                let mut execution = CommandExecution::new(id, command_line, executed_at);
                if let (Some(exit_code), Some(duration_ms)) = (exit_code, duration_ms) {
                    execution = execution.with_result(exit_code, duration_ms);
                }
                
                executions.push(execution);
            }
            
            Ok(executions)
        }).await?
    }
}