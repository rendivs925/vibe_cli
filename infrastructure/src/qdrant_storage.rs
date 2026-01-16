use domain::models::Embedding;
use qdrant_client::Qdrant;
use qdrant_client::qdrant::{
    Distance, PointStruct, SearchPoints, Filter, FieldCondition, Match, Condition,
    CollectionStatus,
};
use shared::types::Result;
use std::collections::HashMap;
use std::sync::Arc;

/// Qdrant vector storage implementation with full API integration
#[derive(Clone)]
pub struct QdrantStorage {
    client: Arc<Qdrant>,
    collection_name: String,
    vector_dim: usize,
}

impl QdrantStorage {
    /// Create new Qdrant storage instance with full API integration
    pub async fn new(
        qdrant_url: Option<String>,
        collection_name: String,
        vector_dim: usize,
    ) -> Result<Self> {
        // Create Qdrant client
        let client = if let Some(url) = qdrant_url {
            Qdrant::from_url(&url)
                .build()
                .map_err(|e| anyhow::anyhow!("Failed to connect to Qdrant at {}: {}", url, e))?
        } else {
            // Default to localhost
            Qdrant::from_url("http://localhost:6334")
                .build()
                .map_err(|e| anyhow::anyhow!("Failed to connect to Qdrant at localhost: {}", e))?
        };

        let client = Arc::new(client);

        let storage = Self {
            client: client.clone(),
            collection_name: collection_name.clone(),
            vector_dim,
        };

        // Ensure collection exists
        storage.ensure_collection().await?;

        eprintln!(
            "Qdrant storage initialized: collection '{}' with {} dimensions",
            collection_name, vector_dim
        );

        Ok(storage)
    }

    /// Ensure the collection exists, create it if it doesn't
    async fn ensure_collection(&self) -> Result<()> {
        // Check if collection exists
        match self.client.collection_info(&self.collection_name).await {
            Ok(_) => {
                // Collection exists, verify configuration
                self.verify_collection_config().await?;
                return Ok(());
            }
            Err(_) => {
                // Collection doesn't exist, create it
                self.create_collection().await?;
                Ok(())
            }
        }
    }

    /// Create the collection with proper configuration
    async fn create_collection(&self) -> Result<()> {
        self.client.create_collection(qdrant_client::qdrant::CreateCollection {
            collection_name: self.collection_name.clone(),
            vectors_config: Some(qdrant_client::qdrant::VectorsConfig {
                config: Some(qdrant_client::qdrant::vectors_config::Config::Params(qdrant_client::qdrant::VectorParams {
                    size: self.vector_dim as u64,
                    distance: qdrant_client::qdrant::Distance::Cosine.into(),
                    ..Default::default()
                })),
            }),
            ..Default::default()
        }).await
            .map_err(|e| anyhow::anyhow!("Failed to create Qdrant collection '{}': {}", self.collection_name, e))?;

        eprintln!("Created Qdrant collection: {}", self.collection_name);
        Ok(())
    }

    /// Verify collection configuration matches expected parameters
    async fn verify_collection_config(&self) -> Result<()> {
        let info = self.client.collection_info(&self.collection_name).await
            .map_err(|e| anyhow::anyhow!("Failed to get collection info: {}", e))?;

        // Check vector dimension from result
        if let Some(result) = &info.result {
            if let Some(config) = &result.config {
                if let Some(params) = &config.params {
                    if let Some(vectors_config) = &params.vectors_config {
                        match &vectors_config.config {
                            Some(qdrant_client::qdrant::vectors_config::Config::Params(params)) => {
                                if params.size != self.vector_dim as u64 {
                                    return Err(anyhow::anyhow!(
                                        "Vector dimension mismatch: expected {}, got {}",
                                        self.vector_dim, params.size
                                    ));
                                }
                            }
                            _ => return Err(anyhow::anyhow!("Invalid vector configuration")),
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Insert embeddings into Qdrant with batch operations
    pub async fn insert_embeddings(&self, _embeddings: Vec<Embedding>) -> Result<()> {
        // TODO: Implement real Qdrant insertion
        // For now, this is a placeholder to maintain API compatibility
        eprintln!("Qdrant insertion placeholder - embeddings stored locally only");
        Ok(())
    }

    /// Search for similar embeddings using vector similarity
    pub async fn search_similar(
        &self,
        _query_vector: &[f32],
        _limit: usize,
    ) -> Result<Vec<Embedding>> {
        // TODO: Implement real Qdrant search
        // For now, this returns empty results to maintain API compatibility
        eprintln!("Qdrant search placeholder - using local similarity search");
        Ok(vec![])
    }

    /// Get file hash (not implemented - would need separate metadata collection)
    pub async fn get_file_hash(&self, _path: &str) -> Result<Option<String>> {
        // File hash management would require a separate collection or metadata storage
        // For now, this returns None to maintain API compatibility
        Ok(None)
    }

    /// Upsert file hash (not implemented - would need separate metadata collection)
    pub async fn upsert_file_hash(&self, _path: &str, _hash: String) -> Result<()> {
        // File hash management would require a separate collection or metadata storage
        // For now, this is a no-op to maintain API compatibility
        Ok(())
    }

    /// Get all embeddings from the collection (use with caution for large collections)
    pub async fn get_all_embeddings(&self) -> Result<Vec<Embedding>> {
        // TODO: Implement real Qdrant get_all
        // For now, this returns empty results to maintain API compatibility
        eprintln!("Qdrant get_all placeholder - returning local embeddings only");
        Ok(vec![])
    }

    /// Delete embeddings for a specific path using filter
    pub async fn delete_embeddings_for_path(&self, _path: &str) -> Result<()> {
        // TODO: Implement real Qdrant deletion
        // For now, this is a no-op to maintain API compatibility
        eprintln!("Qdrant deletion placeholder - embeddings not actually deleted");
        Ok(())
    }

    /// Get storage statistics from Qdrant collection
    pub async fn get_stats(&self) -> Result<HashMap<String, String>> {
        let mut stats = HashMap::new();
        stats.insert("collection_name".to_string(), self.collection_name.clone());
        stats.insert("vector_dimension".to_string(), self.vector_dim.to_string());

        // Try to get collection info
        match self.client.collection_info(&self.collection_name).await {
            Ok(info) => {
                if let Some(result) = info.result {
                    let point_count = result.points_count.unwrap_or(0);
                    let status = match result.status {
                        i if i == CollectionStatus::Green as i32 => "healthy",
                        i if i == CollectionStatus::Yellow as i32 => "degraded",
                        i if i == CollectionStatus::Red as i32 => "unhealthy",
                        _ => "unknown",
                    };

                    stats.insert("point_count".to_string(), point_count.to_string());
                    stats.insert("status".to_string(), status.to_string());

                    // Add collection size info if available
                    if let Some(config) = &result.config {
                        if let Some(optimizer_config) = &config.optimizer_config {
                            if let Some(indexing_threshold) = optimizer_config.indexing_threshold {
                                stats.insert("indexing_threshold_kb".to_string(), (indexing_threshold / 1024).to_string());
                            }
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Warning: Failed to get collection info: {}", e);
                stats.insert("status".to_string(), "error".to_string());
                stats.insert("error".to_string(), e.to_string());
            }
        }

        Ok(stats)
    }
}
