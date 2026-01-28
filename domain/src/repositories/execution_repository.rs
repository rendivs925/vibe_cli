use async_trait::async_trait;
use chrono::{DateTime, Utc};
use shared::error::AppError;

#[derive(Debug, Clone)]
pub struct CommandExecution {
    id: String,
    command_line: String,
    executed_at: DateTime<Utc>,
    exit_code: Option<i32>,
    duration_ms: Option<u64>,
}

impl CommandExecution {
    pub fn new(
        id: String,
        command_line: String,
        executed_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            command_line,
            executed_at,
            exit_code: None,
            duration_ms: None,
        }
    }

    pub fn with_result(mut self, exit_code: i32, duration_ms: u64) -> Self {
        self.exit_code = Some(exit_code);
        self.duration_ms = Some(duration_ms);
        self
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn command_line(&self) -> &str {
        &self.command_line
    }

    pub fn executed_at(&self) -> &DateTime<Utc> {
        &self.executed_at
    }

    pub fn exit_code(&self) -> Option<i32> {
        self.exit_code
    }

    pub fn duration_ms(&self) -> Option<u64> {
        self.duration_ms
    }
}

#[async_trait]
pub trait ExecutionRepository: Send + Sync {
    async fn save(&self, execution: &CommandExecution) -> Result<(), AppError>;
    async fn get_all(&self) -> Result<Vec<CommandExecution>, AppError>;
    async fn get_by_id(&self, id: &str) -> Result<Option<CommandExecution>, AppError>;
    async fn get_by_command(&self, command_line: &str) -> Result<Vec<CommandExecution>, AppError>;
    async fn get_by_date_range(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<Vec<CommandExecution>, AppError>;
}