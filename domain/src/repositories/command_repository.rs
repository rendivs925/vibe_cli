use super::super::entities::command::Command;
use super::embedding_repository::RepositoryError;
use async_trait::async_trait;

/// Repository interface for command storage and retrieval
#[async_trait]
pub trait CommandRepository: Send + Sync {
    /// Store a new command
    async fn save(&self, command: &Command) -> Result<(), RepositoryError>;

    /// Retrieve command by ID
    async fn find_by_id(&self, id: &str) -> Result<Option<Command>, RepositoryError>;

    /// List all commands
    async fn list_all(&self) -> Result<Vec<Command>, RepositoryError>;

    /// Get recent commands
    async fn get_recent(&self, limit: usize) -> Result<Vec<Command>, RepositoryError>;

    /// Search commands by description
    async fn search_by_description(&self, query: &str) -> Result<Vec<Command>, RepositoryError>;

    /// Search commands by command line content
    async fn search_by_command(&self, query: &str) -> Result<Vec<Command>, RepositoryError>;

    /// Get commands by confidence threshold
    async fn get_by_confidence(&self, min_confidence: f32)
        -> Result<Vec<Command>, RepositoryError>;

    /// Get safe commands only
    async fn get_safe_commands(&self) -> Result<Vec<Command>, RepositoryError>;

    /// Get unsafe commands only
    async fn get_unsafe_commands(&self) -> Result<Vec<Command>, RepositoryError>;

    /// Update command
    async fn update(&self, command: &Command) -> Result<(), RepositoryError>;

    /// Delete command by ID
    async fn delete(&self, id: &str) -> Result<(), RepositoryError>;

    /// Count total commands
    async fn count(&self) -> Result<usize, RepositoryError>;

    /// Count safe commands
    async fn count_safe(&self) -> Result<usize, RepositoryError>;

    /// Count unsafe commands
    async fn count_unsafe(&self) -> Result<usize, RepositoryError>;

    /// Check if command exists
    async fn exists(&self, id: &str) -> Result<bool, RepositoryError>;

    /// Get command statistics
    async fn get_stats(&self) -> Result<CommandStats, RepositoryError>;

    /// Find similar commands by description
    async fn find_similar(
        &self,
        command: &Command,
        threshold: f32,
    ) -> Result<Vec<Command>, RepositoryError>;

    /// Get commands by date range
    async fn get_by_date_range(
        &self,
        start: chrono::DateTime<chrono::Utc>,
        end: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<Command>, RepositoryError>;
}

/// Statistics about commands in the repository
#[derive(Debug, Clone)]
pub struct CommandStats {
    total_commands: usize,
    safe_commands: usize,
    unsafe_commands: usize,
    average_confidence: f32,
    commands_by_type: std::collections::HashMap<String, usize>,
    last_updated: chrono::DateTime<chrono::Utc>,
}

impl CommandStats {
    pub fn new(
        total_commands: usize,
        safe_commands: usize,
        unsafe_commands: usize,
        commands_by_type: std::collections::HashMap<String, usize>,
        last_updated: chrono::DateTime<chrono::Utc>,
    ) -> Self {
        let average_confidence = if total_commands > 0 {
            // This would be calculated from actual command confidences in a real implementation
            0.75
        } else {
            0.0
        };

        Self {
            total_commands,
            safe_commands,
            unsafe_commands,
            average_confidence,
            commands_by_type,
            last_updated,
        }
    }

    pub fn total_commands(&self) -> usize {
        self.total_commands
    }

    pub fn safe_commands(&self) -> usize {
        self.safe_commands
    }

    pub fn unsafe_commands(&self) -> usize {
        self.unsafe_commands
    }

    pub fn average_confidence(&self) -> f32 {
        self.average_confidence
    }

    pub fn commands_by_type(&self) -> &std::collections::HashMap<String, usize> {
        &self.commands_by_type
    }

    pub fn last_updated(&self) -> chrono::DateTime<chrono::Utc> {
        self.last_updated
    }

    pub fn safety_rate(&self) -> f32 {
        if self.total_commands > 0 {
            self.safe_commands as f32 / self.total_commands as f32
        } else {
            0.0
        }
    }
}
