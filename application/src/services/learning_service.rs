//! Learning Service - integrates experience buffer with neurosymbolic reasoning
//!
//! Provides RAG-style retrieval of past experiences to prevent repeating mistakes
//! and learns from user corrections.

use infrastructure::storage::experience_buffer::{ExperienceBuffer, FailureType};
use shared::types::Result;
use std::path::PathBuf;

/// Service for learning from experience
pub struct LearningService {
    buffer: ExperienceBuffer,
    session_id: String,
    enabled: bool,
}

impl LearningService {
    /// Create new learning service
    pub fn new() -> Result<Self> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let db_path = PathBuf::from(home).join(".config/vibe_cli/experience.db");

        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let buffer = ExperienceBuffer::new(&db_path)?;
        let session_id = format!(
            "session_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        );

        Ok(Self {
            buffer,
            session_id,
            enabled: true,
        })
    }

    /// Create with custom database path
    pub fn with_path(db_path: PathBuf) -> Result<Self> {
        let buffer = ExperienceBuffer::new(&db_path)?;
        let session_id = format!(
            "session_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        );

        Ok(Self {
            buffer,
            session_id,
            enabled: true,
        })
    }

    /// Check if learning is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enable/disable learning
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Record a successful command execution
    pub fn record_success(
        &self,
        query: &str,
        command: &str,
        user_correction: Option<&str>,
    ) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        self.buffer
            .log_success(&self.session_id, query, command, user_correction)?;

        Ok(())
    }

    /// Record a failed command execution
    pub fn record_failure(
        &self,
        query: &str,
        attempted_command: &str,
        failure_type: FailureType,
        error_message: Option<&str>,
    ) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        self.buffer.log_failure(
            &self.session_id,
            query,
            attempted_command,
            failure_type,
            error_message,
        )?;

        Ok(())
    }

    /// Get context to inject into LLM prompt
    pub fn get_context_for_query(&self, query: &str) -> Result<Option<String>> {
        if !self.enabled {
            return Ok(None);
        }

        // Get Do Not Repeat context
        let dnr_context = self.buffer.get_do_not_repeat_context(query)?;

        // Get best approach if available
        let best_approach = self.buffer.get_best_approach(query)?;

        // Get success rate
        let success_rate = self.buffer.get_success_rate(query)?;

        // Build context string
        let mut context_parts = vec![];

        if let Some(dnr) = dnr_context {
            context_parts.push(dnr);
        }

        if let Some(approach) = best_approach {
            context_parts.push(format!("RECOMMENDED APPROACH: {}", approach));
        }

        if success_rate < 0.5 {
            context_parts.push(format!(
                "WARNING: Similar queries have low success rate ({:.0}%)",
                success_rate * 100.0
            ));
        }

        if context_parts.is_empty() {
            Ok(None)
        } else {
            Ok(Some(context_parts.join("\n\n")))
        }
    }

    /// Get formatted learning context for prompt injection
    pub fn format_learning_context(&self, query: &str) -> Result<String> {
        match self.get_context_for_query(query)? {
            Some(context) => Ok(format!(
                "=== LEARNING CONTEXT (Do Not Repeat Past Mistakes) ===\n{}\n=== END LEARNING CONTEXT ===\n",
                context
            )),
            None => Ok(String::new()),
        }
    }

    /// Check if we have experience with similar queries
    pub fn has_relevant_experience(&self, query: &str) -> Result<bool> {
        let failures = self.buffer.find_similar_failures(query, 1)?;
        Ok(!failures.is_empty())
    }

    /// Get lessons learned for a query
    pub fn get_lessons(&self, query: &str) -> Result<Vec<String>> {
        Ok(self.buffer.get_lessons_learned(query)?)
    }

    /// Get previously failed commands for similar queries
    pub fn get_failed_commands(&self, query: &str, limit: usize) -> Result<Vec<String>> {
        if !self.enabled {
            return Ok(Vec::new());
        }
        let limit_i64 = i64::try_from(limit).unwrap_or(i64::MAX);
        let failures = self.buffer.find_similar_failures(query, limit_i64)?;
        let mut commands: Vec<String> = failures
            .into_iter()
            .map(|entry| entry.attempted_command)
            .collect();
        commands.sort();
        commands.dedup();
        Ok(commands)
    }

    /// Get success rate for query pattern
    pub fn get_success_rate(&self, query: &str) -> Result<f32> {
        Ok(self.buffer.get_success_rate(query)?)
    }

    /// Get service statistics
    pub fn get_stats(&self) -> Result<(usize, usize, f32)> {
        Ok(self.buffer.get_stats()?)
    }

    /// Get the session ID
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Clear all learning data
    pub fn clear_all(&self) -> Result<()> {
        self.buffer.clear_all()?;
        Ok(())
    }

    /// Create augmented prompt with learning context
    pub fn augment_prompt(&self, original_prompt: &str, query: &str) -> Result<String> {
        let context = self.format_learning_context(query)?;

        if context.is_empty() {
            Ok(original_prompt.to_string())
        } else {
            Ok(format!("{}\n\n{}", context, original_prompt))
        }
    }
}

impl Default for LearningService {
    fn default() -> Self {
        Self::new().expect("Failed to initialize learning service")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_db_path() -> PathBuf {
        PathBuf::from("/tmp/test_learning_service.db")
    }

    #[test]
    fn test_record_and_retrieve() {
        let _ = std::fs::remove_file(test_db_path());
        let service = LearningService::with_path(test_db_path()).unwrap();

        // Record a failure
        service
            .record_failure(
                "list processes",
                "invalid command",
                FailureType::CommandNotFound,
                Some("command not found"),
            )
            .unwrap();

        // Check that we have relevant experience
        assert!(service
            .has_relevant_experience("list all processes")
            .unwrap());

        // Get context
        let context = service.get_context_for_query("show processes").unwrap();
        assert!(context.is_some());

        let _ = std::fs::remove_file(test_db_path());
    }

    #[test]
    fn test_augment_prompt() {
        let _ = std::fs::remove_file(test_db_path());
        let service = LearningService::with_path(test_db_path()).unwrap();

        service
            .record_failure(
                "list processes",
                "bad command",
                FailureType::SyntaxError,
                Some("syntax error"),
            )
            .unwrap();

        let original = "Generate a command to list processes";
        let augmented = service.augment_prompt(original, "list processes").unwrap();

        assert!(augmented.contains("LEARNING CONTEXT"));
        assert!(augmented.contains("PREVIOUS FAILURES"));

        let _ = std::fs::remove_file(test_db_path());
    }

    #[test]
    fn test_success_rate() {
        let _ = std::fs::remove_file(test_db_path());
        let service = LearningService::with_path(test_db_path()).unwrap();

        // Record successes and failures
        service
            .record_success("test query", "cmd1", None)
            .unwrap();
        service
            .record_success("test query", "cmd2", None)
            .unwrap();
        service
            .record_failure("test query", "cmd3", FailureType::Other, None)
            .unwrap();

        let rate = service.get_success_rate("test query").unwrap();
        assert!((rate - 0.666).abs() < 0.01);

        let _ = std::fs::remove_file(test_db_path());
    }
}
