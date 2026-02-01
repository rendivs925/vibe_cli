use super::super::entities::session::{Message, Session};
use super::embedding_repository::RepositoryError;
use async_trait::async_trait;

/// Repository interface for session storage and retrieval
#[async_trait]
pub trait SessionRepository: Send + Sync {
    /// Store a new session
    async fn save(&self, session: &Session) -> Result<(), RepositoryError>;

    /// Retrieve session by ID
    async fn find_by_id(&self, id: &str) -> Result<Option<Session>, RepositoryError>;

    /// List all sessions
    async fn list_all(&self) -> Result<Vec<Session>, RepositoryError>;

    /// Get recent sessions
    async fn get_recent(&self, limit: usize) -> Result<Vec<Session>, RepositoryError>;

    /// Update session
    async fn update(&self, session: &Session) -> Result<(), RepositoryError>;

    /// Delete session by ID
    async fn delete(&self, id: &str) -> Result<(), RepositoryError>;

    /// Add message to session
    async fn add_message(&self, session_id: &str, message: &Message)
        -> Result<(), RepositoryError>;

    /// Get session messages
    async fn get_messages(&self, session_id: &str) -> Result<Vec<Message>, RepositoryError>;

    /// Clear session history
    async fn clear_history(&self, session_id: &str) -> Result<(), RepositoryError>;

    /// Count total sessions
    async fn count(&self) -> Result<usize, RepositoryError>;

    /// Check if session exists
    async fn exists(&self, id: &str) -> Result<bool, RepositoryError>;

    /// Get session statistics
    async fn get_stats(&self) -> Result<SessionStats, RepositoryError>;

    /// Find sessions by context key-value pair
    async fn find_by_context(
        &self,
        key: &str,
        value: &str,
    ) -> Result<Vec<Session>, RepositoryError>;

    /// Update session context
    async fn update_context(
        &self,
        session_id: &str,
        key: String,
        value: String,
    ) -> Result<(), RepositoryError>;

    /// Get sessions older than specified date
    async fn find_older_than(
        &self,
        date: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<Session>, RepositoryError>;
}

/// Statistics about sessions in the repository
#[derive(Debug, Clone)]
pub struct SessionStats {
    total_sessions: usize,
    total_messages: usize,
    average_messages_per_session: f64,
    active_sessions: usize,
    last_updated: chrono::DateTime<chrono::Utc>,
}

impl SessionStats {
    pub fn new(
        total_sessions: usize,
        total_messages: usize,
        active_sessions: usize,
        last_updated: chrono::DateTime<chrono::Utc>,
    ) -> Self {
        let average_messages_per_session = if total_sessions > 0 {
            total_messages as f64 / total_sessions as f64
        } else {
            0.0
        };

        Self {
            total_sessions,
            total_messages,
            average_messages_per_session,
            active_sessions,
            last_updated,
        }
    }

    pub fn total_sessions(&self) -> usize {
        self.total_sessions
    }

    pub fn total_messages(&self) -> usize {
        self.total_messages
    }

    pub fn average_messages_per_session(&self) -> f64 {
        self.average_messages_per_session
    }

    pub fn active_sessions(&self) -> usize {
        self.active_sessions
    }

    pub fn last_updated(&self) -> chrono::DateTime<chrono::Utc> {
        self.last_updated
    }
}
