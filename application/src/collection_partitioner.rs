//! Multi-Language Collection Partitioning Service
//!
//! This module provides intelligent partitioning of semantic memory collections
//! by programming language, domain, or project for better organization and
//! performance in production deployments.

use crate::semantic_memory::{SemanticMemoryService, ConversationMemory};
use crate::advanced_qdrant::AdvancedQdrantManager;
use shared::types::Result;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub enum PartitionType {
    Language,
    Domain,
    Project,
    Custom,
}

#[derive(Debug, Clone)]
pub struct CollectionPartition {
    pub name: String,
    pub partition_type: PartitionType,
    pub partition_key: String,
    pub vector_dim: usize,
    pub description: String,
    pub created_at: i64,
    pub is_active: bool,
}

#[derive(Debug, Clone)]
pub struct PartitionConfig {
    pub enable_auto_partitioning: bool,
    pub default_vector_dim: usize,
    pub max_partitions_per_type: usize,
    pub partition_cleanup_threshold_days: u64,
}

impl Default for PartitionConfig {
    fn default() -> Self {
        Self {
            enable_auto_partitioning: true,
            default_vector_dim: usize::default(),
            max_partitions_per_type: 10,
            partition_cleanup_threshold_days: 365,
        }
    }
}

pub struct CollectionPartitioner {
    qdrant_manager: Arc<AdvancedQdrantManager>,
    semantic_memory: Arc<SemanticMemoryService>,
    partitions: HashMap<String, CollectionPartition>,
    config: PartitionConfig,
}

impl CollectionPartitioner {
    pub fn new(
        qdrant_manager: Arc<AdvancedQdrantManager>,
        semantic_memory: Arc<SemanticMemoryService>,
        config: PartitionConfig,
    ) -> Self {
        Self {
            qdrant_manager,
            semantic_memory,
            partitions: HashMap::new(),
            config,
        }
    }

    /// Create a new partition for a specific language/domain
    pub async fn create_partition(
        &mut self,
        partition_type: PartitionType,
        partition_key: &str,
        description: &str,
    ) -> Result<String> {
        let collection_name = self.generate_collection_name(&partition_type, partition_key);

        // Check if partition already exists
        if self.partitions.contains_key(&collection_name) {
            return Err(anyhow::anyhow!("Partition {} already exists", collection_name));
        }

        // Create the collection with optimized settings
        self.qdrant_manager.create_production_semantic_memory_collection(&collection_name).await?;

        let partition = CollectionPartition {
            name: collection_name.clone(),
            partition_type: partition_type.clone(),
            partition_key: partition_key.to_string(),
            vector_dim: 768, // Standard embedding dimension
            description: description.to_string(),
            created_at: chrono::Utc::now().timestamp(),
            is_active: true,
        };

        self.partitions.insert(collection_name.clone(), partition);

        println!("Created partition: {} ({})", collection_name, description);
        Ok(collection_name)
    }

    /// Get the appropriate partition for a given context
    pub async fn get_partition_for_context(
        &self,
        context: &PartitionContext,
    ) -> Result<String> {
        // Try to find existing partition
        for partition in self.partitions.values() {
            if self.partition_matches_context(partition, context) {
                return Ok(partition.name.clone());
            }
        }

        // If auto-partitioning is enabled and no partition exists, create one
        if self.config.enable_auto_partitioning {
            // Use language as default partitioning strategy
            if let Some(language) = &context.language {
                let partition_type = PartitionType::Language;
                let collection_name = self.generate_collection_name(&partition_type, language);

                if !self.partitions.contains_key(&collection_name) {
                    // Create new partition
                    let description = format!("Auto-created partition for {} language", language);
                    // Note: This would need to be async, but we're in a sync context
                    // In practice, this would be handled differently
                    return Ok(collection_name);
                } else {
                    return Ok(collection_name);
                }
            }
        }

        // Fallback to default collection
        Ok("conversation_memory".to_string())
    }

    /// Route a memory to the appropriate partition
    pub async fn route_memory_to_partition(
        &self,
        memory: &ConversationMemory,
        context: &PartitionContext,
    ) -> Result<String> {
        let partition_name = self.get_partition_for_context(context).await?;

        // In a real implementation, this would store the memory in the specific partition
        // For now, we just return the partition name
        Ok(partition_name)
    }

    /// Get statistics for all partitions
    pub async fn get_partition_statistics(&self) -> Result<PartitionStatistics> {
        let mut stats = PartitionStatistics {
            total_partitions: self.partitions.len(),
            active_partitions: 0,
            partitions_by_type: HashMap::new(),
            total_memory_across_partitions: 0,
            oldest_partition_age_days: None,
            newest_partition_age_days: None,
        };

        let now = chrono::Utc::now().timestamp();

        for partition in self.partitions.values() {
            if partition.is_active {
                stats.active_partitions += 1;
            }

            // Count by type
            let type_key = match partition.partition_type {
                PartitionType::Language => "language",
                PartitionType::Domain => "domain",
                PartitionType::Project => "project",
                PartitionType::Custom => "custom",
            };

            *stats.partitions_by_type.entry(type_key.to_string()).or_insert(0) += 1;

            // Calculate ages
            let age_days = (now - partition.created_at) / 86400;
            if let Some(oldest) = stats.oldest_partition_age_days {
                stats.oldest_partition_age_days = Some(oldest.max(age_days));
            } else {
                stats.oldest_partition_age_days = Some(age_days);
            }

            if let Some(newest) = stats.newest_partition_age_days {
                stats.newest_partition_age_days = Some(newest.min(age_days));
            } else {
                stats.newest_partition_age_days = Some(age_days);
            }
        }

        // Get total memory (simplified - would need to query each partition)
        let (total_memories, _, _, _) = self.semantic_memory.get_memory_stats().await?;
        stats.total_memory_across_partitions = total_memories;

        Ok(stats)
    }

    /// Clean up old or unused partitions
    pub async fn cleanup_partitions(&mut self) -> Result<CleanupResult> {
        let mut cleaned_partitions = Vec::new();
        let mut deleted_partitions = Vec::new();
        let now = chrono::Utc::now().timestamp();
        let threshold_seconds = self.config.partition_cleanup_threshold_days as i64 * 86400;

        // Find old partitions
        for partition in self.partitions.values_mut() {
            let age_seconds = now - partition.created_at;

            if age_seconds > threshold_seconds {
                // Mark as inactive (or delete in real implementation)
                partition.is_active = false;
                cleaned_partitions.push(partition.name.clone());
            }
        }

        // Remove inactive partitions from our tracking
        let mut to_remove = Vec::new();
        for (name, partition) in &self.partitions {
            if !partition.is_active {
                to_remove.push(name.clone());
                deleted_partitions.push(name.clone());
            }
        }

        for name in to_remove {
            self.partitions.remove(&name);
        }

        Ok(CleanupResult {
            cleaned_partitions,
            deleted_partitions,
            partitions_remaining: self.partitions.len(),
        })
    }

    /// List all available partitions
    pub fn list_partitions(&self) -> Vec<&CollectionPartition> {
        self.partitions.values().collect()
    }

    /// Get partition recommendations based on current usage
    pub async fn get_partition_recommendations(&self) -> Result<Vec<PartitionRecommendation>> {
        let stats = self.semantic_memory.get_memory_stats().await?;
        let mut recommendations = Vec::new();

        // Recommend language-based partitioning if we have many memories
        if stats.0 > 10000 {
            recommendations.push(PartitionRecommendation {
                partition_type: PartitionType::Language,
                reason: "High memory count suggests language-based partitioning for better organization".to_string(),
                expected_benefit: "Improved search relevance and reduced cross-contamination".to_string(),
            });
        }

        // Recommend project-based partitioning for large teams
        if self.partitions.len() > 5 {
            recommendations.push(PartitionRecommendation {
                partition_type: PartitionType::Project,
                reason: "Multiple active partitions suggest project-based organization".to_string(),
                expected_benefit: "Better isolation between different projects and teams".to_string(),
            });
        }

        Ok(recommendations)
    }

    // Helper methods
    fn generate_collection_name(&self, partition_type: &PartitionType, key: &str) -> String {
        let type_prefix = match partition_type {
            PartitionType::Language => "lang",
            PartitionType::Domain => "domain",
            PartitionType::Project => "project",
            PartitionType::Custom => "custom",
        };

        format!("{}_{}_{}", type_prefix, key.to_lowercase().replace(" ", "_"), chrono::Utc::now().timestamp())
    }

    fn partition_matches_context(&self, partition: &CollectionPartition, context: &PartitionContext) -> bool {
        match partition.partition_type {
            PartitionType::Language => {
                if let Some(language) = &context.language {
                    partition.partition_key == *language
                } else {
                    false
                }
            }
            PartitionType::Domain => {
                if let Some(domain) = &context.domain {
                    partition.partition_key == *domain
                } else {
                    false
                }
            }
            PartitionType::Project => {
                if let Some(project) = &context.project {
                    partition.partition_key == *project
                } else {
                    false
                }
            }
            PartitionType::Custom => {
                // Custom matching logic would go here
                false
            }
        }
    }
}

#[derive(Debug)]
pub struct PartitionContext {
    pub language: Option<String>,
    pub domain: Option<String>,
    pub project: Option<String>,
    pub custom_tags: Vec<String>,
}

impl Default for PartitionContext {
    fn default() -> Self {
        Self {
            language: None,
            domain: None,
            project: None,
            custom_tags: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct PartitionStatistics {
    pub total_partitions: usize,
    pub active_partitions: usize,
    pub partitions_by_type: HashMap<String, usize>,
    pub total_memory_across_partitions: usize,
    pub oldest_partition_age_days: Option<i64>,
    pub newest_partition_age_days: Option<i64>,
}

#[derive(Debug)]
pub struct CleanupResult {
    pub cleaned_partitions: Vec<String>,
    pub deleted_partitions: Vec<String>,
    pub partitions_remaining: usize,
}

#[derive(Debug)]
pub struct PartitionRecommendation {
    pub partition_type: PartitionType,
    pub reason: String,
    pub expected_benefit: String,
}

/// Predefined partition configurations for common use cases
pub mod partition_presets {
    use super::*;

    pub fn create_rust_partition() -> (PartitionType, &'static str, &'static str) {
        (PartitionType::Language, "rust", "Rust programming language conversations and code")
    }

    pub fn create_python_partition() -> (PartitionType, &'static str, &'static str) {
        (PartitionType::Language, "python", "Python programming language conversations and code")
    }

    pub fn create_web_development_partition() -> (PartitionType, &'static str, &'static str) {
        (PartitionType::Domain, "web_dev", "Web development conversations and technologies")
    }

    pub fn create_data_science_partition() -> (PartitionType, &'static str, &'static str) {
        (PartitionType::Domain, "data_science", "Data science and machine learning conversations")
    }

    pub fn create_project_partition(project_name: &str) -> (PartitionType, String, String) {
        (PartitionType::Project, project_name.to_string(),
         format!("Project-specific conversations for {}", project_name))
    }
}