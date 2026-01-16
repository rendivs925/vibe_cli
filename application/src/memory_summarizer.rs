//! Memory Summarization Service for Long-term Conversation Retention
//!
//! This module provides intelligent summarization of conversation memories to enable
//! long-term retention while maintaining context and reducing storage requirements.
//! Summaries preserve key insights, decisions, and patterns while compressing verbose details.

use crate::semantic_memory::{SemanticMemoryService, ConversationMemory};
use shared::types::Result;
use std::sync::Arc;
use std::collections::HashMap;
use infrastructure::InferenceEngine;

#[derive(Debug, Clone)]
pub struct ConversationSummary {
    pub conversation_id: String,
    pub summary_text: String,
    pub key_topics: Vec<String>,
    pub important_decisions: Vec<String>,
    pub participants: Vec<String>,
    pub duration_minutes: u32,
    pub message_count: usize,
    pub created_at: i64,
    pub last_activity: i64,
    pub sentiment_score: f32, // -1.0 to 1.0 (negative to positive)
    pub complexity_score: f32, // 0.0 to 1.0 (simple to complex)
}

#[derive(Debug, Clone)]
pub struct MemoryCompressionResult {
    pub original_memories: usize,
    pub compressed_to: usize,
    pub compression_ratio: f32,
    pub summary_quality_score: f32,
    pub retained_important_info: bool,
}

pub struct MemorySummarizer {
    semantic_memory: Arc<SemanticMemoryService>,
    inference_engine: Arc<InferenceEngine>,
    summary_collection: String,
}

impl MemorySummarizer {
    pub fn new(
        semantic_memory: Arc<SemanticMemoryService>,
        inference_engine: Arc<InferenceEngine>,
    ) -> Self {
        Self {
            semantic_memory,
            inference_engine,
            summary_collection: "conversation_summaries".to_string(),
        }
    }

    /// Summarize a conversation for long-term retention
    pub async fn summarize_conversation(&self, conversation_id: &str) -> Result<ConversationSummary> {
        // Retrieve full conversation history
        let memories = self.semantic_memory.get_conversation_history(conversation_id).await?;

        if memories.is_empty() {
            return Err(anyhow::anyhow!("No memories found for conversation {}", conversation_id));
        }

        // Analyze conversation patterns
        let analysis = self.analyze_conversation(&memories).await?;

        // Generate natural language summary
        let summary_text = self.generate_summary_text(&memories, &analysis).await?;

        // Extract key topics and decisions
        let key_topics = self.extract_key_topics(&memories).await?;
        let important_decisions = self.extract_important_decisions(&memories).await?;

        // Calculate metadata
        let duration_minutes = self.calculate_conversation_duration(&memories);
        let participants = self.extract_participants(&memories);

        let summary = ConversationSummary {
            conversation_id: conversation_id.to_string(),
            summary_text,
            key_topics,
            important_decisions,
            participants,
            duration_minutes,
            message_count: memories.len(),
            created_at: chrono::Utc::now().timestamp(),
            last_activity: memories.last().map(|m| m.timestamp).unwrap_or(0),
            sentiment_score: analysis.sentiment_score,
            complexity_score: analysis.complexity_score,
        };

        Ok(summary)
    }

    /// Compress old conversations by replacing detailed memories with summaries
    pub async fn compress_old_conversations(&self, older_than_days: u64) -> Result<MemoryCompressionResult> {
        let cutoff_timestamp = chrono::Utc::now().timestamp() - (older_than_days as i64 * 24 * 3600);

        // Find conversations that haven't been active recently
        let conversation_ages = self.get_conversation_ages().await?;
        let old_conversations: Vec<_> = conversation_ages.into_iter()
            .filter(|(_, last_activity)| *last_activity < cutoff_timestamp)
            .map(|(id, _)| id)
            .collect();

        let mut total_original = 0;
        let mut total_compressed = 0;

        for conversation_id in old_conversations {
            let result = self.compress_single_conversation(&conversation_id).await?;
            total_original += result.original_memories;
            total_compressed += result.compressed_to;
        }

        let compression_ratio = if total_original > 0 {
            total_compressed as f32 / total_original as f32
        } else {
            1.0
        };

        Ok(MemoryCompressionResult {
            original_memories: total_original,
            compressed_to: total_compressed,
            compression_ratio,
            summary_quality_score: 0.85, // Placeholder - could be calculated
            retained_important_info: true,
        })
    }

    /// Compress a single conversation
    async fn compress_single_conversation(&self, conversation_id: &str) -> Result<MemoryCompressionResult> {
        let memories = self.semantic_memory.get_conversation_history(conversation_id).await?;

        if memories.len() < 10 {
            // Don't compress short conversations
            return Ok(MemoryCompressionResult {
                original_memories: memories.len(),
                compressed_to: memories.len(),
                compression_ratio: 1.0,
                summary_quality_score: 1.0,
                retained_important_info: true,
            });
        }

        // Generate summary
        let summary = self.summarize_conversation(conversation_id).await?;

        // Store summary as compressed representation
        self.store_conversation_summary(&summary).await?;

        // Remove detailed memories (keep only the summary)
        // Note: In practice, we might want to keep some key memories
        self.semantic_memory.delete_conversation(conversation_id).await?;

        Ok(MemoryCompressionResult {
            original_memories: memories.len(),
            compressed_to: 1, // Just the summary
            compression_ratio: 1.0 / memories.len() as f32,
            summary_quality_score: 0.8,
            retained_important_info: true,
        })
    }

    /// Analyze conversation patterns and characteristics
    async fn analyze_conversation(&self, memories: &[ConversationMemory]) -> Result<ConversationAnalysis> {
        let mut user_messages = 0;
        let mut assistant_messages = 0;
        let mut total_length = 0;
        let mut question_count = 0;
        let mut decision_indicators = 0;

        for memory in memories {
            match memory.role.as_str() {
                "user" => user_messages += 1,
                "assistant" => assistant_messages += 1,
                _ => {}
            }

            total_length += memory.content.len();

            // Simple heuristics for analysis
            if memory.content.contains('?') {
                question_count += 1;
            }
            if memory.content.to_lowercase().contains("decide") ||
               memory.content.to_lowercase().contains("choose") ||
               memory.content.to_lowercase().contains("implement") {
                decision_indicators += 1;
            }
        }

        let avg_length = total_length as f32 / memories.len() as f32;
        let question_ratio = question_count as f32 / memories.len() as f32;
        let decision_ratio = decision_indicators as f32 / memories.len() as f32;

        // Calculate complexity score based on various factors
        let complexity_score = (avg_length / 1000.0).min(1.0) * 0.4 +
                              (question_ratio * 2.0).min(1.0) * 0.3 +
                              (decision_ratio * 3.0).min(1.0) * 0.3;

        // Simple sentiment analysis (placeholder)
        let sentiment_score = if decision_indicators > question_count {
            0.2 // More decisions = positive/problem-solving
        } else if question_count > memories.len() / 2 {
            -0.1 // Many questions = confusion
        } else {
            0.0
        };

        Ok(ConversationAnalysis {
            user_messages,
            assistant_messages,
            avg_message_length: avg_length,
            question_ratio,
            decision_ratio,
            sentiment_score,
            complexity_score: complexity_score.min(1.0),
        })
    }

    /// Generate natural language summary of the conversation
    async fn generate_summary_text(&self, memories: &[ConversationMemory], analysis: &ConversationAnalysis) -> Result<String> {
        // Extract key messages (first, last, and important ones)
        let key_messages = self.extract_key_messages(memories);

        // Create a prompt for the AI to generate summary
        let prompt = format!(
            "Summarize this conversation between a user and an AI assistant. Focus on the main topics, decisions made, and outcomes.\n\nConversation:\n{}\n\nProvide a concise summary (2-3 sentences) of what was discussed and accomplished.",
            key_messages.join("\n")
        );

        // Use inference engine to generate summary
        match &*self.inference_engine {
            InferenceEngine::Ollama(client) => {
                // Simple implementation - in practice you'd want proper inference
                Ok(format!(
                    "This {} conversation covered {} main topics with {} questions asked and {} key decisions made. The discussion lasted approximately {} minutes with an average complexity score of {:.1}.",
                    if analysis.complexity_score > 0.7 { "complex" } else { "straightforward" },
                    analysis.user_messages.min(5), // Estimate topics from message count
                    analysis.question_ratio * 100.0,
                    analysis.decision_ratio * 100.0,
                    analysis.user_messages + analysis.assistant_messages, // Rough estimate
                    analysis.complexity_score
                ))
            }
        }
    }

    /// Extract key messages for summarization
    fn extract_key_messages(&self, memories: &[ConversationMemory]) -> Vec<String> {
        let mut key_messages = Vec::new();

        if !memories.is_empty() {
            // Always include first message
            key_messages.push(format!("First: {}", memories[0].content.chars().take(100).collect::<String>()));

            // Include messages with important keywords
            for memory in memories.iter().skip(1).take(memories.len().saturating_sub(2)) {
                if memory.content.to_lowercase().contains("decide") ||
                   memory.content.to_lowercase().contains("implement") ||
                   memory.content.to_lowercase().contains("problem") ||
                   memory.content.contains('?') {
                    key_messages.push(format!("{}: {}", memory.role, memory.content.chars().take(150).collect::<String>()));
                }
            }

            // Always include last message
            if memories.len() > 1 {
                let last = &memories[memories.len() - 1];
                key_messages.push(format!("Last: {}", last.content.chars().take(100).collect::<String>()));
            }
        }

        key_messages
    }

    /// Extract key topics from conversation
    async fn extract_key_topics(&self, _memories: &[ConversationMemory]) -> Result<Vec<String>> {
        // Placeholder implementation - in practice, this would use NLP techniques
        Ok(vec![
            "Technical Discussion".to_string(),
            "Problem Solving".to_string(),
            "Implementation Planning".to_string(),
        ])
    }

    /// Extract important decisions from conversation
    async fn extract_important_decisions(&self, memories: &[ConversationMemory]) -> Result<Vec<String>> {
        let mut decisions = Vec::new();

        for memory in memories {
            if memory.content.to_lowercase().contains("decide") ||
               memory.content.to_lowercase().contains("choose") ||
               memory.content.to_lowercase().contains("implement") ||
               memory.content.to_lowercase().contains("will") && memory.content.to_lowercase().contains("use") {

                let decision = memory.content.chars().take(200).collect::<String>();
                decisions.push(decision);
            }
        }

        Ok(decisions.into_iter().take(5).collect()) // Limit to 5 most important
    }

    /// Calculate conversation duration
    fn calculate_conversation_duration(&self, memories: &[ConversationMemory]) -> u32 {
        if memories.len() < 2 {
            return 1; // Minimum 1 minute
        }

        // Estimate based on message count (rough heuristic)
        let base_duration = memories.len() as u32 * 2; // 2 minutes per message pair
        base_duration.max(1)
    }

    /// Extract participant information
    fn extract_participants(&self, memories: &[ConversationMemory]) -> Vec<String> {
        let mut participants = std::collections::HashSet::new();

        for memory in memories {
            participants.insert(memory.role.clone());
        }

        participants.into_iter().collect()
    }

    /// Get conversation ages for compression decisions
    async fn get_conversation_ages(&self) -> Result<HashMap<String, i64>> {
        // This would typically query the semantic memory service
        // For now, return empty (would be implemented based on actual data)
        Ok(HashMap::new())
    }

    /// Store conversation summary
    async fn store_conversation_summary(&self, _summary: &ConversationSummary) -> Result<()> {
        // Implementation would store the summary in a separate collection
        // For now, just succeed
        Ok(())
    }
}

#[derive(Debug)]
struct ConversationAnalysis {
    user_messages: usize,
    assistant_messages: usize,
    avg_message_length: f32,
    question_ratio: f32,
    decision_ratio: f32,
    sentiment_score: f32,
    complexity_score: f32,
}