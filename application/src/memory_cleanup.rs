//! Memory Cleanup Policies for Semantic Memory Management
//!
//! This module implements intelligent cleanup policies to manage memory usage
//! and prevent unbounded growth of conversation data in production environments.

use crate::semantic_memory::{SemanticMemoryService, ConversationMemory};
use shared::types::Result;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct CleanupPolicy {
    pub max_memories_per_conversation: usize,
    pub max_total_memories: usize,
    pub memory_ttl_days: u64,
    pub conversation_ttl_days: u64,
    pub cleanup_interval_hours: u64,
    pub enable_auto_cleanup: bool,
}

impl Default for CleanupPolicy {
    fn default() -> Self {
        Self {
            max_memories_per_conversation: 1000,  // Keep last 1000 messages per conversation
            max_total_memories: 100_000,          // Global limit
            memory_ttl_days: 90,                  // 90 days for individual memories
            conversation_ttl_days: 365,           // 1 year for conversations
            cleanup_interval_hours: 24,           // Clean up once per day
            enable_auto_cleanup: true,
        }
    }
}

pub struct MemoryCleanupService {
    semantic_memory: Arc<SemanticMemoryService>,
    policy: CleanupPolicy,
    last_cleanup: Option<SystemTime>,
}

impl MemoryCleanupService {
    pub fn new(semantic_memory: Arc<SemanticMemoryService>, policy: CleanupPolicy) -> Self {
        Self {
            semantic_memory,
            policy,
            last_cleanup: None,
        }
    }

    /// Check if cleanup is needed based on the schedule
    pub fn should_cleanup(&self) -> bool {
        if !self.policy.enable_auto_cleanup {
            return false;
        }

        let now = SystemTime::now();
        let cleanup_interval = Duration::from_secs(self.policy.cleanup_interval_hours * 3600);

        match self.last_cleanup {
            None => true, // Never cleaned before
            Some(last) => now.duration_since(last).unwrap_or_default() >= cleanup_interval,
        }
    }

    /// Perform comprehensive cleanup based on all policies
    pub async fn perform_cleanup(&mut self) -> Result<CleanupStats> {
        println!("🧹 Starting memory cleanup...");
        let start_time = SystemTime::now();

        let mut stats = CleanupStats::default();

        // 1. Clean up old memories by TTL
        stats.memories_deleted_ttl = self.cleanup_expired_memories().await?;

        // 2. Clean up oversized conversations
        stats.memories_deleted_size = self.cleanup_oversized_conversations().await?;

        // 3. Clean up old conversations
        stats.conversations_deleted = self.cleanup_old_conversations().await?;

        // 4. Global size limit enforcement
        stats.memories_deleted_global = self.enforce_global_limits().await?;

        self.last_cleanup = Some(SystemTime::now());

        let duration = SystemTime::now().duration_since(start_time)
            .unwrap_or_default();

        stats.duration_ms = duration.as_millis() as u64;

        println!("✅ Cleanup completed in {}ms", stats.duration_ms);
        println!("   Deleted: {} TTL memories, {} size memories, {} conversations, {} global memories",
                stats.memories_deleted_ttl, stats.memories_deleted_size,
                stats.conversations_deleted, stats.memories_deleted_global);

        Ok(stats)
    }

    /// Clean up memories older than TTL
    async fn cleanup_expired_memories(&self) -> Result<usize> {
        // Get all memories and filter by age
        let all_memories = self.get_all_memories_with_timestamps().await?;
        let now = SystemTime::now();
        let ttl_duration = Duration::from_secs(self.policy.memory_ttl_days * 24 * 3600);

        let expired_memories = all_memories.into_iter()
            .filter(|(_, timestamp)| {
                if let Ok(age) = now.duration_since(*timestamp) {
                    age > ttl_duration
                } else {
                    false
                }
            })
            .collect::<Vec<_>>();

        let count = expired_memories.len();

        if count > 0 {
            println!("   Cleaning up {} expired memories (older than {} days)", count, self.policy.memory_ttl_days);

            // Delete expired memories
            for (id, _) in expired_memories {
                // Note: This is a simplified approach. In production, you'd want batch deletion
                // or use Qdrant's scroll API for efficient bulk operations
                if let Err(e) = self.delete_memory_by_id(&id).await {
                    eprintln!("Failed to delete memory {}: {}", id, e);
                }
            }
        }

        Ok(count)
    }

    /// Clean up conversations that exceed size limits
    async fn cleanup_oversized_conversations(&self) -> Result<usize> {
        let conversation_sizes = self.get_conversation_sizes().await?;
        let mut total_deleted = 0;

        for (conversation_id, size) in conversation_sizes {
            if size > self.policy.max_memories_per_conversation {
                let to_delete = size - self.policy.max_memories_per_conversation;
                println!("   Conversation '{}' has {} memories, trimming {} oldest",
                        conversation_id, size, to_delete);

                total_deleted += self.trim_conversation(&conversation_id, to_delete).await?;
            }
        }

        Ok(total_deleted)
    }

    /// Clean up conversations older than conversation TTL
    async fn cleanup_old_conversations(&self) -> Result<usize> {
        let conversations_with_age = self.get_conversation_ages().await?;
        let now = SystemTime::now();
        let ttl_duration = Duration::from_secs(self.policy.conversation_ttl_days * 24 * 3600);

        let mut conversations_to_delete = Vec::new();

        for (conversation_id, last_activity) in conversations_with_age {
            if let Ok(age) = now.duration_since(last_activity) {
                if age > ttl_duration {
                    conversations_to_delete.push(conversation_id);
                }
            }
        }

        let count = conversations_to_delete.len();

        if count > 0 {
            println!("   Deleting {} old conversations (inactive for {} days)",
                    count, self.policy.conversation_ttl_days);

            for conversation_id in conversations_to_delete {
                if let Err(e) = self.semantic_memory.delete_conversation(&conversation_id).await {
                    eprintln!("Failed to delete conversation {}: {}", conversation_id, e);
                }
            }
        }

        Ok(count)
    }

    /// Enforce global memory limits
    async fn enforce_global_limits(&self) -> Result<usize> {
        let (total_memories, _) = self.semantic_memory.get_memory_stats().await?;

        if total_memories > self.policy.max_total_memories {
            let to_delete = total_memories - self.policy.max_total_memories;
            println!("   Global limit exceeded: {} memories, need to delete {}",
                    total_memories, to_delete);

            // Delete oldest memories across all conversations
            self.delete_oldest_memories(to_delete).await
        } else {
            Ok(0)
        }
    }

    /// Helper: Get all memories with their timestamps
    async fn get_all_memories_with_timestamps(&self) -> Result<Vec<(String, SystemTime)>> {
        // This is a simplified implementation
        // In production, you'd use Qdrant's scroll API for efficient pagination
        let all_embeddings = self.semantic_memory.qdrant.get_all_embeddings().await?;

        let mut memories_with_timestamps = Vec::new();

        for embedding in all_embeddings {
            if let Ok(memory) = serde_json::from_str::<ConversationMemory>(&embedding.text) {
                // Convert timestamp from i64 back to SystemTime
                let timestamp = SystemTime::UNIX_EPOCH + Duration::from_secs(memory.timestamp as u64);
                memories_with_timestamps.push((embedding.id, timestamp));
            }
        }

        Ok(memories_with_timestamps)
    }

    /// Helper: Get conversation sizes
    async fn get_conversation_sizes(&self) -> Result<HashMap<String, usize>> {
        let all_embeddings = self.semantic_memory.qdrant.get_all_embeddings().await?;
        let mut sizes = HashMap::new();

        for embedding in all_embeddings {
            if let Ok(memory) = serde_json::from_str::<ConversationMemory>(&embedding.text) {
                *sizes.entry(memory.conversation_id).or_insert(0) += 1;
            }
        }

        Ok(sizes)
    }

    /// Helper: Get conversation last activity times
    async fn get_conversation_ages(&self) -> Result<HashMap<String, SystemTime>> {
        let all_embeddings = self.semantic_memory.qdrant.get_all_embeddings().await?;
        let mut ages = HashMap::new();

        for embedding in all_embeddings {
            if let Ok(memory) = serde_json::from_str::<ConversationMemory>(&embedding.text) {
                let timestamp = SystemTime::UNIX_EPOCH + Duration::from_secs(memory.timestamp as u64);

                ages.entry(memory.conversation_id)
                    .and_modify(|e| *e = timestamp.max(*e))
                    .or_insert(timestamp);
            }
        }

        Ok(ages)
    }

    /// Helper: Trim oldest messages from a conversation
    async fn trim_conversation(&self, conversation_id: &str, count: usize) -> Result<usize> {
        let memories = self.semantic_memory.get_conversation_history(conversation_id).await?;

        // Sort by index (chronological order)
        let mut sorted_memories = memories;
        sorted_memories.sort_by_key(|m| m.message_index);

        // Delete oldest messages
        let to_delete = sorted_memories.iter().take(count).collect::<Vec<_>>();
        let mut deleted = 0;

        for memory in to_delete {
            let id = format!("{}_{}", conversation_id, memory.message_index);
            if let Err(e) = self.delete_memory_by_id(&id).await {
                eprintln!("Failed to delete memory {}: {}", id, e);
            } else {
                deleted += 1;
            }
        }

        Ok(deleted)
    }

    /// Helper: Delete oldest memories globally
    async fn delete_oldest_memories(&self, count: usize) -> Result<usize> {
        let all_memories = self.get_all_memories_with_timestamps().await?;

        // Sort by timestamp (oldest first)
        let mut sorted_memories = all_memories;
        sorted_memories.sort_by_key(|(_, timestamp)| *timestamp);

        let to_delete = sorted_memories.into_iter().take(count).collect::<Vec<_>>();
        let mut deleted = 0;

        for (id, _) in to_delete {
            if let Err(e) = self.delete_memory_by_id(&id).await {
                eprintln!("Failed to delete memory {}: {}", id, e);
            } else {
                deleted += 1;
            }
        }

        Ok(deleted)
    }

    /// Helper: Delete a memory by ID (simplified implementation)
    async fn delete_memory_by_id(&self, id: &str) -> Result<()> {
        // This is a simplified deletion - in production you'd implement proper deletion
        // For now, we'll use a placeholder path that matches the memory
        self.semantic_memory.qdrant.delete_embeddings_for_path(&format!("memory/{}", id)).await
    }
}

#[derive(Debug, Default)]
pub struct CleanupStats {
    pub memories_deleted_ttl: usize,
    pub memories_deleted_size: usize,
    pub conversations_deleted: usize,
    pub memories_deleted_global: usize,
    pub duration_ms: u64,
}

impl CleanupStats {
    pub fn total_deleted(&self) -> usize {
        self.memories_deleted_ttl + self.memories_deleted_size +
        self.memories_deleted_global
    }
}