use crate::entities::command::Command;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use shared::error::AppError;

#[derive(Debug, Clone)]
pub struct CommandExecution {
    id: String,
    command_line: String,
    executed_at: Option<DateTime<Utc>>,
    exit_code: Option<i32>,
    duration_ms: Option<u64>,
    executed: bool,
}

impl CommandExecution {
    pub fn new(id: String, command_line: String, executed_at: DateTime<Utc>) -> Self {
        Self {
            id,
            command_line,
            executed_at: Some(executed_at),
            exit_code: None,
            duration_ms: None,
            executed: true,
        }
    }

    pub fn from_command(command: &Command) -> Self {
        Self {
            id: command.id().to_string(),
            command_line: command.command_line().to_string(),
            executed_at: None,
            exit_code: None,
            duration_ms: None,
            executed: false,
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

    pub fn executed_at(&self) -> &Option<DateTime<Utc>> {
        &self.executed_at
    }

    pub fn exit_code(&self) -> Option<i32> {
        self.exit_code
    }

    pub fn duration_ms(&self) -> Option<u64> {
        self.duration_ms
    }

    pub fn is_executed(&self) -> bool {
        self.executed
    }
}

#[async_trait]
pub trait ExecutionRepository: Send + Sync {
    async fn save(&self, execution: &CommandExecution) -> Result<(), AppError>;
    async fn get_all(&self) -> Result<Vec<CommandExecution>, AppError>;
    async fn get_by_id(&self, id: &str) -> Result<Option<CommandExecution>, AppError>;
    async fn get_by_command(&self, command_line: &str) -> Result<Vec<CommandExecution>, AppError>;
    async fn get_by_date_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<CommandExecution>, AppError>;
}
