//! Semantic Memory Service for storing and retrieving conversation context
//!
//! This service enables agents to have persistent memory by storing conversation
//! history as semantic embeddings in Qdrant, allowing retrieval of relevant past
//! interactions based on semantic similarity rather than exact keyword matching.

use domain::models::{AgentContext, ConversationMessage};
use infrastructure::{
    embedder::{Embedder, EmbeddingInput},
    qdrant_storage::QdrantStorage,
};
use serde::{Deserialize, Serialize};
use shared::types::Result;
use std::sync::Arc;

/// Represents a stored conversation memory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMemory {
    pub conversation_id: String,
    pub message_index: usize,
    pub role: String,
    pub content: String,
    pub timestamp: i64,
    pub tool_calls: Option<Vec<domain::models::ToolCall>>,
    pub tool_call_id: Option<String>,
}

/// Service for managing semantic conversation memory
pub struct SemanticMemoryService {
    qdrant: Arc<QdrantStorage>,
    embedder: Arc<Embedder>,
    collection_name: String,
}

impl SemanticMemoryService {
    /// Create a new semantic memory service
    pub async fn new(qdrant_url: &str, embedder: Arc<Embedder>) -> Result<Self> {
        let qdrant = Arc::new(
            QdrantStorage::new(
                Some(qdrant_url.to_string()),
                "conversation_memory".to_string(),
                768,
            )
            .await?,
        );

        Ok(Self {
            qdrant,
            embedder,
            collection_name: "conversation_memory".to_string(),
        })
    }

    /// Helper method to generate embedding for text
    async fn embed_text(&self, text: &str) -> Result<Vec<f32>> {
        let input = EmbeddingInput {
            id: "temp".to_string(),
            path: "temp".to_string(),
            text: text.to_string(),
        };

        let embeddings = self.embedder.as_ref().generate_embeddings(&[input]).await?;
        Ok(embeddings
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("No embedding generated"))?
            .vector)
    }

    /// Store a conversation message in semantic memory
    pub async fn store_message(
        &self,
        conversation_id: &str,
        message_index: usize,
        message: &ConversationMessage,
    ) -> Result<()> {
        // Generate embedding for the message content
        let embedding = self.embed_text(&message.content).await?;

        // Create memory entry
        let memory = ConversationMemory {
            conversation_id: conversation_id.to_string(),
            message_index,
            role: message.role.clone(),
            content: message.content.clone(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
            tool_calls: message.tool_calls.clone(),
            tool_call_id: message.tool_call_id.clone(),
        };

        // Store in Qdrant with metadata
        let memory_json = serde_json::to_string(&memory)?;
        let id = format!("{}_{}", conversation_id, message_index);

        self.qdrant
            .insert_embeddings(vec![domain::models::Embedding {
                id,
                vector: embedding,
                text: memory_json,
                path: format!("conversation/{}/{}", conversation_id, message_index),
            }])
            .await?;

        Ok(())
    }

    /// Store an entire conversation context
    pub async fn store_conversation(
        &self,
        context: &AgentContext,
        conversation_id: &str,
    ) -> Result<()> {
        for (index, message) in context.conversation_history.iter().enumerate() {
            self.store_message(conversation_id, index, message).await?;
        }
        Ok(())
    }

    /// Retrieve relevant conversation memories based on semantic similarity
    pub async fn retrieve_relevant_memories(
        &self,
        query: &str,
        conversation_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ConversationMemory>> {
        // Generate embedding for the query
        let query_embedding = self.embed_text(query).await?;

        // Search for similar memories
        let results = self.qdrant.search_similar(&query_embedding, limit).await?;

        let mut memories = Vec::new();

        for result in results {
            // Parse the stored memory data
            match serde_json::from_str::<ConversationMemory>(&result.text) {
                Ok(memory) => {
                    // If conversation_id is specified, filter to that conversation
                    if let Some(cid) = conversation_id {
                        if memory.conversation_id == cid {
                            memories.push(memory);
                        }
                    } else {
                        memories.push(memory);
                    }
                }
                Err(e) => {
                    eprintln!("Failed to parse conversation memory: {}", e);
                }
            }
        }

        Ok(memories)
    }

    /// Get all embeddings (used by cleanup service)
    pub async fn get_all_embeddings(&self) -> Result<Vec<domain::models::Embedding>> {
        self.qdrant.get_all_embeddings().await
    }

    /// Delete embeddings for a specific path (used by cleanup service)
    pub async fn delete_embeddings_for_path(&self, path: &str) -> Result<()> {
        self.qdrant.delete_embeddings_for_path(path).await
    }

    /// Get conversation history for a specific conversation
    pub async fn get_conversation_history(&self, conversation_id: &str) -> Result<Vec<ConversationMemory>> {
        // Use Qdrant path prefix filtering to get all messages for this conversation
        let path_prefix = format!("conversation/{}/", conversation_id);
        let results = self.qdrant.get_embeddings_by_path_prefix(&path_prefix, 1000).await?;

        let mut conversation_memories = Vec::new();

        for result in results {
            // Parse the stored memory data
            match serde_json::from_str::<ConversationMemory>(&result.text) {
                Ok(memory) => {
                    conversation_memories.push(memory);
                }
                Err(e) => {
                    eprintln!("Failed to parse conversation memory: {}", e);
                }
            }
        }

        // Sort by message index to maintain chronological order
        conversation_memories.sort_by_key(|m| m.message_index);

        Ok(conversation_memories)
    }

    /// Delete conversation memory for a specific conversation
    pub async fn delete_conversation(&self, conversation_id: &str) -> Result<()> {
        // Get all memories for this conversation and delete them
        let memories = self.get_conversation_history(conversation_id).await?;

        let ids_to_delete = memories
            .into_iter()
            .map(|m| format!("{}_{}", m.conversation_id, m.message_index))
            .collect::<Vec<_>>();

        if !ids_to_delete.is_empty() {
            self.qdrant
                .delete_embeddings_for_path(&format!("conversation/{}/", conversation_id))
                .await?;
        }

        Ok(())
    }

    /// Get memory statistics including age tracking
    pub async fn get_memory_stats(&self) -> Result<(usize, usize, Option<i64>, Option<i64>)> {
        // Get total count from Qdrant
        let all_embeddings = self.get_all_embeddings().await?;
        let total_memories = all_embeddings.len();

        // Count unique conversations and track timestamps
        let mut conversation_ids = std::collections::HashSet::new();
        let mut oldest_timestamp: Option<i64> = None;
        let mut newest_timestamp: Option<i64> = None;

        for embedding in all_embeddings {
            if let Ok(memory) = serde_json::from_str::<ConversationMemory>(&embedding.text) {
                conversation_ids.insert(memory.conversation_id);

                // Track oldest timestamp
                if let Some(current_oldest) = oldest_timestamp {
                    if memory.timestamp < current_oldest {
                        oldest_timestamp = Some(memory.timestamp);
                    }
                } else {
                    oldest_timestamp = Some(memory.timestamp);
                }

                // Track newest timestamp
                if let Some(current_newest) = newest_timestamp {
                    if memory.timestamp > current_newest {
                        newest_timestamp = Some(memory.timestamp);
                    }
                } else {
                    newest_timestamp = Some(memory.timestamp);
                }
            }
        }

        Ok((total_memories, conversation_ids.len(), oldest_timestamp, newest_timestamp))
    }
}
