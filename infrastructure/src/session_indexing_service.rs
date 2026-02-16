//! Session Indexing Service - manages semantic indexing of sessions and commands
//!
//! Provides high-level API for indexing session data with embeddings,
//! enabling semantic search across all sessions and commands.

use crate::ollama_client::OllamaClient;
use crate::semantic_index::{CommandSearchResult, SemanticIndex, SessionSearchResult};
use chrono::{Duration, Utc};
use shared::types::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Configuration for the session indexing service
#[derive(Debug, Clone)]
pub struct IndexingConfig {
    /// Path to the semantic index database
    pub db_path: PathBuf,
    /// Embedding model to use
    pub embedding_model: String,
    /// Minimum similarity threshold for search
    pub min_similarity: f32,
    /// Default number of results to return
    pub default_limit: usize,
    /// Retention period for sessions (default: 30 days)
    pub retention_days: i64,
    /// Enable hybrid search (FTS + vector)
    pub hybrid_search: bool,
}

impl Default for IndexingConfig {
    fn default() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        Self {
            db_path: PathBuf::from(home).join(".config/vibe_cli/semantic_index.db"),
            embedding_model: "nomic-embed-text".to_string(),
            min_similarity: 0.6,
            default_limit: 5,
            retention_days: 30,
            hybrid_search: true,
        }
    }
}

/// Service for semantic indexing and retrieval of sessions
pub struct SessionIndexingService {
    index: Arc<Mutex<SemanticIndex>>,
    client: OllamaClient,
    config: IndexingConfig,
}

impl SessionIndexingService {
    /// Create a new indexing service with default config
    pub async fn new() -> Result<Self> {
        Self::with_config(IndexingConfig::default()).await
    }

    /// Create with custom configuration
    pub async fn with_config(config: IndexingConfig) -> Result<Self> {
        let index = SemanticIndex::new(&config.db_path).await?;

        Ok(Self {
            index: Arc::new(Mutex::new(index)),
            client: OllamaClient::new()?,
            config,
        })
    }

    /// Index a complete session
    pub async fn index_session(
        &self,
        session_id: &str,
        goal: &str,
        summary: Option<&str>,
        tags: Option<Vec<String>>,
        success_rate: f32,
    ) -> Result<()> {
        // Generate embedding for the goal
        let embedding = self.generate_embedding(goal).await?;

        let index = self.index.lock().await;
        index
            .index_session(session_id, goal, summary, &embedding, tags, success_rate)
            .await?;

        Ok(())
    }

    /// Index a command execution
    pub async fn index_command(
        &self,
        command_id: &str,
        session_id: &str,
        command: &str,
        output_text: Option<&str>,
        exit_code: i32,
    ) -> Result<()> {
        // Generate embedding for the command
        let embedding = self.generate_embedding(command).await?;
        let success = exit_code == 0;

        let index = self.index.lock().await;
        index
            .index_command(
                command_id,
                session_id,
                command,
                output_text,
                &embedding,
                exit_code,
                success,
            )
            .await?;

        Ok(())
    }

    /// Index an experience pattern
    pub async fn index_pattern(
        &self,
        pattern_id: &str,
        pattern_text: &str,
        pattern_type: &str,
        success_count: i32,
        failure_count: i32,
        confidence: f32,
    ) -> Result<()> {
        // Generate embedding for the pattern
        let embedding = self.generate_embedding(pattern_text).await?;

        let index = self.index.lock().await;
        index
            .index_pattern(
                pattern_id,
                pattern_text,
                pattern_type,
                &embedding,
                success_count,
                failure_count,
                confidence,
            )
            .await?;

        Ok(())
    }

    /// Search for similar sessions
    pub async fn find_similar_sessions(
        &self,
        query: &str,
        limit: Option<usize>,
    ) -> Result<Vec<SessionSearchResult>> {
        let limit = limit.unwrap_or(self.config.default_limit);

        if self.config.hybrid_search {
            // Use hybrid search
            let embedding = self.generate_embedding(query).await?;
            let index = self.index.lock().await;
            let results = index
                .hybrid_search_sessions(query, &embedding, limit)
                .await?;
            Ok(results)
        } else {
            // Use vector-only search
            let embedding = self.generate_embedding(query).await?;
            let index = self.index.lock().await;
            let results = index
                .search_sessions(&embedding, limit, self.config.min_similarity)
                .await?;
            Ok(results)
        }
    }

    /// Search for similar commands
    pub async fn find_similar_commands(
        &self,
        query: &str,
        limit: Option<usize>,
        only_successful: bool,
    ) -> Result<Vec<CommandSearchResult>> {
        let limit = limit.unwrap_or(self.config.default_limit);
        let embedding = self.generate_embedding(query).await?;

        let index = self.index.lock().await;
        let results = index
            .search_commands(&embedding, limit, self.config.min_similarity, only_successful)
            .await?;

        Ok(results)
    }

    /// Search for experience patterns
    pub async fn find_patterns(
        &self,
        query: &str,
        limit: Option<usize>,
        pattern_type: Option<&str>,
    ) -> Result<Vec<(String, String, f32, f32)>> {
        // Returns: (pattern_id, pattern_text, similarity, confidence)
        let limit = limit.unwrap_or(self.config.default_limit);
        let embedding = self.generate_embedding(query).await?;

        let index = self.index.lock().await;
        let results = index
            .search_patterns(&embedding, limit, self.config.min_similarity, pattern_type)
            .await?;

        Ok(results)
    }

    /// Get similar past sessions with formatted context
    pub async fn get_similar_sessions_context(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Option<String>> {
        let similar = self.find_similar_sessions(query, Some(limit)).await?;

        if similar.is_empty() {
            return Ok(None);
        }

        let mut context_parts = vec![
            "=== SIMILAR PAST SESSIONS ===".to_string(),
        ];

        for (i, session) in similar.iter().enumerate() {
            let similarity_pct = (session.similarity * 100.0) as i32;
            context_parts.push(format!(
                "{}. Goal: {} ({}% similar)",
                i + 1,
                session.goal,
                similarity_pct
            ));

            if let Some(ref summary) = session.summary {
                context_parts.push(format!("   Summary: {}", summary));
            }

            context_parts.push(format!(
                "   Date: {}",
                session.created_at.format("%Y-%m-%d %H:%M")
            ));
        }

        context_parts.push("=== END SIMILAR SESSIONS ===".to_string());

        Ok(Some(context_parts.join("\n")))
    }

    /// Get relevant command patterns from past sessions
    pub async fn get_command_patterns_context(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Option<String>> {
        let patterns = self.find_patterns(query, Some(limit), None).await?;

        if patterns.is_empty() {
            return Ok(None);
        }

        let mut context_parts = vec![
            "=== LEARNED PATTERNS ===".to_string(),
        ];

        for (i, (pattern_id, pattern_text, similarity, confidence)) in patterns.iter().enumerate() {
            let similarity_pct = (*similarity * 100.0) as i32;
            let confidence_pct = (*confidence * 100.0) as i32;
            context_parts.push(format!(
                "{}. {} ({}% match, {}% confidence) - ID: {}",
                i + 1,
                pattern_text,
                similarity_pct,
                confidence_pct,
                pattern_id
            ));
        }

        context_parts.push("=== END PATTERNS ===".to_string());

        Ok(Some(context_parts.join("\n")))
    }

    /// Run cleanup - delete old sessions
    pub async fn cleanup_old_sessions(&self) -> Result<usize> {
        let cutoff = Utc::now() - Duration::days(self.config.retention_days);
        let index = self.index.lock().await;
        let deleted = index.delete_old_sessions(cutoff).await?;
        Ok(deleted)
    }

    /// Compact the index database
    pub async fn compact(&self) -> Result<()> {
        let index = self.index.lock().await;
        index.compact().await?;
        Ok(())
    }

    /// Get index statistics
    pub async fn get_stats(&self) -> Result<(usize, usize, usize)> {
        let index = self.index.lock().await;
        let stats = index.get_stats().await?;
        Ok(stats)
    }

    /// Generate embedding using Ollama
    async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>> {
        self.client.generate_embedding(text).await
    }

    /// Update service configuration
    pub fn set_config(mut self, config: IndexingConfig) -> Self {
        self.config = config;
        self
    }

    /// Get current configuration
    pub fn config(&self) -> &IndexingConfig {
        &self.config
    }
}

impl Clone for SessionIndexingService {
    fn clone(&self) -> Self {
        Self {
            index: Arc::clone(&self.index),
            client: OllamaClient::new().expect("Failed to create Ollama client"),
            config: self.config.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_config() -> IndexingConfig {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let dir = PathBuf::from(home).join(".config/vibe_cli/test_dbs");
        let _ = std::fs::create_dir_all(&dir);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        IndexingConfig {
            db_path: dir.join(format!("indexing_service_{}.db", nanos)),
            embedding_model: "nomic-embed-text".to_string(),
            min_similarity: 0.3,
            default_limit: 5,
            retention_days: 30,
            hybrid_search: false, // Disable for testing
        }
    }

    #[tokio::test]
    async fn test_service_creation() {
        let config = test_config();
        let _ = std::fs::remove_file(&config.db_path);

        // This test may fail if Ollama is not running, so we'll skip the actual indexing
        // and just verify the service can be created
        let result = SessionIndexingService::with_config(config.clone()).await;

        // Service should be created successfully (if Ollama is available)
        if result.is_ok() {
            let service = result.unwrap();
            let stats = service.get_stats().await.unwrap();
            assert_eq!(stats.0, 0); // 0 sessions
            assert_eq!(stats.1, 0); // 0 commands
            assert_eq!(stats.2, 0); // 0 patterns
        }

        let _ = std::fs::remove_file(&config.db_path);
    }

    #[test]
    fn test_default_config() {
        let config = IndexingConfig::default();
        assert_eq!(config.embedding_model, "nomic-embed-text");
        assert!(config.min_similarity > 0.0);
        assert!(config.default_limit > 0);
        assert!(config.retention_days > 0);
        assert!(config.hybrid_search);
    }

    #[test]
    fn test_config_clone() {
        let config = IndexingConfig::default();
        let cloned = config.clone();
        assert_eq!(config.embedding_model, cloned.embedding_model);
        assert_eq!(config.min_similarity, cloned.min_similarity);
    }
}
