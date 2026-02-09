use crate::ports::{Cache, StorageService};
use async_trait::async_trait;
use domain::entities::session::{Message, Session};
use shared::error::AppError;
use crate::services::cache_codec::{decode_cache, encode_cache};

/// Use case for session management
pub struct SessionUseCase {
    storage: StorageService,
    cache: Box<dyn Cache>,
}

impl SessionUseCase {
    pub fn new(storage: StorageService, cache: Box<dyn Cache>) -> Self {
        Self { storage, cache }
    }

    /// Create a new session
    pub async fn create_session(&self, session_id: String) -> Result<Session, AppError> {
        let session = Session::new(session_id.clone());

        // Store session
        self.storage.save_session(&session).await?;

        // Cache session
        let cache_key = format!("session:{}", session_id);
        let session_data = encode_cache(&session)?;
        self.cache.set(&cache_key, &session_data).await?;

        Ok(session)
    }

    /// Get session by ID
    pub async fn get_session(&self, session_id: &str) -> Result<Option<Session>, AppError> {
        // Check cache first
        let cache_key = format!("session:{}", session_id);
        if let Some(cached_data) = self.cache.get(&cache_key).await? {
            let session: Session = decode_cache(&cached_data)?;
            return Ok(Some(session));
        }

        // Fetch from storage
        let session = self.storage.find_session_by_id(session_id).await?;

        // Cache if found
        if let Some(ref sess) = session {
            let session_data = encode_cache(sess)?;
            self.cache.set(&cache_key, &session_data).await?;
        }

        Ok(session)
    }

    /// Add message to session
    pub async fn add_message(&self, session_id: &str, message: Message) -> Result<(), AppError> {
        // Get session
        let mut session = self
            .get_session(session_id)
            .await?
            .ok_or_else(|| AppError::not_found(format!("Session {} not found", session_id)))?;

        // Add message
        session.add_message(message);

        // Update storage
        self.storage.save_session(&session).await?;

        // Update cache
        let cache_key = format!("session:{}", session_id);
        let session_data = encode_cache(&session)?;
        self.cache.set(&cache_key, &session_data).await?;

        Ok(())
    }

    /// Get session messages
    pub async fn get_messages(&self, session_id: &str) -> Result<Vec<Message>, AppError> {
        let session = self
            .get_session(session_id)
            .await?
            .ok_or_else(|| AppError::not_found(format!("Session {} not found", session_id)))?;

        Ok(session.history().to_vec())
    }

    /// Get recent sessions
    pub async fn get_recent_sessions(&self, _limit: usize) -> Result<Vec<Session>, AppError> {
        // This would fetch from storage
        Ok(vec![])
    }

    /// Delete session
    pub async fn delete_session(&self, session_id: &str) -> Result<(), AppError> {
        // Delete from storage
        self.storage.delete_session(session_id).await?;

        // Delete from cache
        let cache_key = format!("session:{}", session_id);
        self.cache.delete(&cache_key).await?;

        Ok(())
    }

    /// Clear session history
    pub async fn clear_session_history(&self, session_id: &str) -> Result<(), AppError> {
        // Get session
        let mut session = self
            .get_session(session_id)
            .await?
            .ok_or_else(|| AppError::not_found(format!("Session {} not found", session_id)))?;

        // Clear history
        session.clear_history();

        // Update storage
        self.storage.save_session(&session).await?;

        // Update cache
        let cache_key = format!("session:{}", session_id);
        let session_data = encode_cache(&session)?;
        self.cache.set(&cache_key, &session_data).await?;

        Ok(())
    }

    /// Get session statistics
    pub async fn get_session_stats(&self, session_id: &str) -> Result<SessionStats, AppError> {
        let session = self
            .get_session(session_id)
            .await?
            .ok_or_else(|| AppError::not_found(format!("Session {} not found", session_id)))?;

        let message_count = session.history().len();
        let user_messages = session
            .history()
            .iter()
            .filter(|msg| matches!(msg.role(), domain::entities::session::MessageRole::User))
            .count();
        let assistant_messages = session
            .history()
            .iter()
            .filter(|msg| {
                matches!(
                    msg.role(),
                    domain::entities::session::MessageRole::Assistant
                )
            })
            .count();

        Ok(SessionStats::new(
            message_count,
            user_messages,
            assistant_messages,
            chrono::Utc::now(),
        ))
    }

    /// Search sessions by content
    pub async fn search_sessions(
        &self,
        _query: &str,
        _limit: usize,
    ) -> Result<Vec<SessionSearchResult>, AppError> {
        // This would search through sessions
        Ok(vec![])
    }
}

/// Session statistics
#[derive(Debug, Clone)]
pub struct SessionStats {
    pub total_messages: usize,
    pub user_messages: usize,
    pub assistant_messages: usize,
    pub last_activity: chrono::DateTime<chrono::Utc>,
}

impl SessionStats {
    pub fn new(
        total_messages: usize,
        user_messages: usize,
        assistant_messages: usize,
        last_activity: chrono::DateTime<chrono::Utc>,
    ) -> Self {
        Self {
            total_messages,
            user_messages,
            assistant_messages,
            last_activity,
        }
    }

    pub fn conversation_ratio(&self) -> f32 {
        if self.user_messages > 0 {
            self.assistant_messages as f32 / self.user_messages as f32
        } else {
            0.0
        }
    }
}

/// Session search result
#[derive(Debug, Clone)]
pub struct SessionSearchResult {
    pub session_id: String,
    pub snippet: String,
    pub message_count: usize,
    pub last_activity: chrono::DateTime<chrono::Utc>,
}

impl SessionSearchResult {
    pub fn new(
        session_id: String,
        snippet: String,
        message_count: usize,
        last_activity: chrono::DateTime<chrono::Utc>,
    ) -> Self {
        Self {
            session_id,
            snippet,
            message_count,
            last_activity,
        }
    }
}

#[async_trait]
pub trait AsyncSessionService: Send + Sync {
    async fn create_session(&self, session_id: String) -> Result<Session, AppError>;
    async fn get_session(&self, session_id: &str) -> Result<Option<Session>, AppError>;
    async fn add_message(&self, session_id: &str, message: Message) -> Result<(), AppError>;
    async fn get_messages(&self, session_id: &str) -> Result<Vec<Message>, AppError>;
}
