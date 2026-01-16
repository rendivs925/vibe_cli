use domain::models::Embedding;
use qdrant_client::Qdrant;
use qdrant_client::qdrant::{
    PointId, Vectors, PointStruct, Value, CollectionStatus,
    SearchPoints, UpsertPointsBuilder, ScrollPoints, DeletePointsBuilder,
    point_id, vectors, value, vectors_output,
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
    pub async fn insert_embeddings(&self, embeddings: Vec<Embedding>) -> Result<()> {
        if embeddings.is_empty() {
            return Ok(());
        }

        let embeddings_len = embeddings.len();

        // Convert embeddings to Qdrant points
        let mut points = Vec::new();

        for embedding in embeddings {
            // Create payload with metadata
            let mut payload = std::collections::HashMap::new();
            payload.insert("text".to_string(), Value {
                kind: Some(value::Kind::StringValue(embedding.text.clone())),
            });
            payload.insert("path".to_string(), Value {
                kind: Some(value::Kind::StringValue(embedding.path.clone())),
            });

            let point = PointStruct {
                id: Some(PointId {
                    point_id_options: Some(point_id::PointIdOptions::Num(embedding.id.parse().unwrap_or(0))),
                }),
                vectors: Some(Vectors {
                    vectors_options: Some(vectors::VectorsOptions::Vector(
                        qdrant_client::qdrant::Vector {
                            data: embedding.vector,
                            ..Default::default()
                        }
                    )),
                }),
                payload,
            };

            points.push(point);
        }

        // Use the new upsert API
        let result = self.client
            .upsert_points(UpsertPointsBuilder::new(&self.collection_name, points))
            .await;

        match result {
            Ok(_) => {
                eprintln!("Successfully inserted {} embeddings into Qdrant collection '{}'",
                         embeddings_len, self.collection_name);
                Ok(())
            }
            Err(e) => {
                eprintln!("Failed to insert embeddings: {}", e);
                Err(anyhow::anyhow!("Failed to upsert points to Qdrant: {}", e))
            }
        }
    }

    /// Search for similar embeddings using vector similarity
    pub async fn search_similar(
        &self,
        query_vector: &[f32],
        limit: usize,
    ) -> Result<Vec<Embedding>> {
        let search_result = self.client
            .search_points(SearchPoints {
                collection_name: self.collection_name.clone(),
                vector: query_vector.to_vec(),
                limit: limit as u64,
                with_payload: Some(true.into()),
                with_vectors: Some(true.into()),
                ..Default::default()
            })
            .await
            .map_err(|e| anyhow::anyhow!("Failed to search points in Qdrant: {}", e))?;

        let mut results = Vec::new();

        for point in search_result.result {
            let id = match &point.id {
                Some(id) => match &id.point_id_options {
                    Some(point_id::PointIdOptions::Num(n)) => n.to_string(),
                    Some(point_id::PointIdOptions::Uuid(u)) => u.to_string(),
                    None => continue,
                },
                None => continue,
            };

            let text = point.payload
                .get("text")
                .and_then(|v| match &v.kind {
                    Some(value::Kind::StringValue(s)) => Some(s.as_str()),
                    _ => None,
                })
                .unwrap_or("")
                .to_string();

            let path = point.payload
                .get("path")
                .and_then(|v| match &v.kind {
                    Some(value::Kind::StringValue(s)) => Some(s.as_str()),
                    _ => None,
                })
                .unwrap_or("")
                .to_string();

            let vector = match &point.vectors {
                Some(vectors) => match &vectors.vectors_options {
                    Some(vectors_output::VectorsOptions::Vector(vec)) => vec.data.clone(),
                    _ => continue,
                },
                None => continue,
            };

            results.push(Embedding {
                id,
                vector,
                text,
                path,
            });
        }

        eprintln!("Qdrant search completed - found {} similar embeddings", results.len());
        Ok(results)
    }

    /// Search for similar embeddings with metadata filtering
    pub async fn search_with_filter(
        &self,
        query_vector: &[f32],
        limit: usize,
        filter_key: &str,
        filter_value: &str,
    ) -> Result<Vec<Embedding>> {
        use qdrant_client::qdrant::{Condition, Filter, FieldCondition, Match, r#match};

        // Create a filter for the specified field
        let filter = Filter {
            must: vec![Condition {
                condition_one_of: Some(qdrant_client::qdrant::condition::ConditionOneOf::Field(
                    FieldCondition {
                        key: filter_key.to_string(),
                        r#match: Some(Match {
                            match_value: Some(r#match::MatchValue::Keyword(filter_value.to_string())),
                        }),
                        ..Default::default()
                    }
                )),
            }],
            ..Default::default()
        };

        let search_result = self.client
            .search_points(SearchPoints {
                collection_name: self.collection_name.clone(),
                vector: query_vector.to_vec(),
                limit: limit as u64,
                filter: Some(filter),
                with_payload: Some(true.into()),
                with_vectors: Some(true.into()),
                ..Default::default()
            })
            .await
            .map_err(|e| anyhow::anyhow!("Failed to search points with filter in Qdrant: {}", e))?;

        let mut results = Vec::new();

        for point in search_result.result {
            let id = match &point.id {
                Some(id) => match &id.point_id_options {
                    Some(point_id::PointIdOptions::Num(n)) => n.to_string(),
                    Some(point_id::PointIdOptions::Uuid(u)) => u.to_string(),
                    None => continue,
                },
                None => continue,
            };

            let text = point.payload
                .get("text")
                .and_then(|v| match &v.kind {
                    Some(value::Kind::StringValue(s)) => Some(s.as_str()),
                    _ => None,
                })
                .unwrap_or("")
                .to_string();

            let path = point.payload
                .get("path")
                .and_then(|v| match &v.kind {
                    Some(value::Kind::StringValue(s)) => Some(s.as_str()),
                    _ => None,
                })
                .unwrap_or("")
                .to_string();

            let vector = match &point.vectors {
                Some(vectors) => match &vectors.vectors_options {
                    Some(vectors_output::VectorsOptions::Vector(vec)) => vec.data.clone(),
                    _ => continue,
                },
                None => continue,
            };

            results.push(Embedding {
                id,
                vector,
                text,
                path,
            });
        }

        eprintln!("Qdrant filtered search completed - found {} embeddings", results.len());
        Ok(results)
    }

    /// Get embeddings by path prefix (useful for retrieving all conversation memories)
    pub async fn get_embeddings_by_path_prefix(&self, path_prefix: &str, limit: usize) -> Result<Vec<Embedding>> {
        use qdrant_client::qdrant::{Condition, Filter, FieldCondition, Match, r#match};

        // Create a filter for path prefix matching
        let filter = Filter {
            must: vec![Condition {
                condition_one_of: Some(qdrant_client::qdrant::condition::ConditionOneOf::Field(
                    FieldCondition {
                        key: "path".to_string(),
                        r#match: Some(Match {
                            match_value: Some(r#match::MatchValue::Text(path_prefix.to_string())),
                        }),
                        ..Default::default()
                    }
                )),
            }],
            ..Default::default()
        };

        let mut all_embeddings = Vec::new();
        let mut offset = None;

        // Use scroll API with filter to get matching points in batches
        loop {
            let scroll_result = self.client
                .scroll(ScrollPoints {
                    collection_name: self.collection_name.clone(),
                    limit: Some(limit.min(1000) as u32),
                    offset,
                    filter: Some(filter.clone()),
                    with_payload: Some(true.into()),
                    with_vectors: Some(true.into()),
                    ..Default::default()
                })
                .await
                .map_err(|e| anyhow::anyhow!("Failed to scroll points with filter in Qdrant: {}", e))?;

            // Convert scroll results to embeddings
            for point in &scroll_result.result {
                let id = match &point.id {
                    Some(id) => match &id.point_id_options {
                        Some(point_id::PointIdOptions::Num(n)) => n.to_string(),
                        Some(point_id::PointIdOptions::Uuid(u)) => u.to_string(),
                        None => continue,
                    },
                    None => continue,
                };

                let text = point.payload
                    .get("text")
                    .and_then(|v| match &v.kind {
                        Some(value::Kind::StringValue(s)) => Some(s.as_str()),
                        _ => None,
                    })
                    .unwrap_or("")
                    .to_string();

                let path = point.payload
                    .get("path")
                    .and_then(|v| match &v.kind {
                        Some(value::Kind::StringValue(s)) => Some(s.as_str()),
                        _ => None,
                    })
                    .unwrap_or("")
                    .to_string();

                // Additional client-side filtering for prefix matching
                if path.starts_with(path_prefix) {
                    let vector = match &point.vectors {
                        Some(vectors) => match &vectors.vectors_options {
                            Some(vectors_output::VectorsOptions::Vector(vec)) => vec.data.clone(),
                            _ => continue,
                        },
                        None => continue,
                    };

                    all_embeddings.push(Embedding {
                        id,
                        vector,
                        text,
                        path,
                    });
                }

                if all_embeddings.len() >= limit {
                    break;
                }
            }

            // Check if we have enough results or no more pages
            if all_embeddings.len() >= limit || scroll_result.next_page_offset.is_none() {
                break;
            }

            offset = scroll_result.next_page_offset;
        }

        eprintln!("Retrieved {} embeddings with path prefix '{}'", all_embeddings.len(), path_prefix);
        Ok(all_embeddings)
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
        let mut all_embeddings = Vec::new();
        let mut offset = None;

        // Use scroll API to get all points in batches
        loop {
            let scroll_result = self.client
                .scroll(ScrollPoints {
                    collection_name: self.collection_name.clone(),
                    limit: Some(1000),
                    offset,
                    with_payload: Some(true.into()),
                    with_vectors: Some(true.into()),
                    ..Default::default()
                })
                .await
                .map_err(|e| anyhow::anyhow!("Failed to scroll points in Qdrant: {}", e))?;

            // Convert scroll results to embeddings
            for point in &scroll_result.result {
                let id = match &point.id {
                    Some(id) => match &id.point_id_options {
                        Some(point_id::PointIdOptions::Num(n)) => n.to_string(),
                        Some(point_id::PointIdOptions::Uuid(u)) => u.to_string(),
                        None => continue,
                    },
                    None => continue,
                };

                let text = point.payload
                    .get("text")
                    .and_then(|v| match &v.kind {
                        Some(value::Kind::StringValue(s)) => Some(s.as_str()),
                        _ => None,
                    })
                    .unwrap_or("")
                    .to_string();

                let path = point.payload
                    .get("path")
                    .and_then(|v| match &v.kind {
                        Some(value::Kind::StringValue(s)) => Some(s.as_str()),
                        _ => None,
                    })
                    .unwrap_or("")
                    .to_string();

                let vector = match &point.vectors {
                    Some(vectors) => match &vectors.vectors_options {
                        Some(vectors_output::VectorsOptions::Vector(vec)) => vec.data.clone(),
                        _ => continue,
                    },
                    None => continue,
                };

                all_embeddings.push(Embedding {
                    id,
                    vector,
                    text,
                    path,
                });
            }

            // Check if we have more results
            if scroll_result.next_page_offset.is_none() {
                break;
            }

            offset = scroll_result.next_page_offset;
        }

        eprintln!("Retrieved {} embeddings from Qdrant collection '{}'",
                 all_embeddings.len(), self.collection_name);
        Ok(all_embeddings)
    }

    /// Delete embeddings for a specific path using filter
    pub async fn delete_embeddings_for_path(&self, path: &str) -> Result<()> {
        use qdrant_client::qdrant::{Condition, Filter, FieldCondition, Match, r#match};

        // Create a filter for the path field
        let filter = Filter {
            must: vec![Condition {
                condition_one_of: Some(qdrant_client::qdrant::condition::ConditionOneOf::Field(
                    FieldCondition {
                        key: "path".to_string(),
                        r#match: Some(Match {
                            match_value: Some(r#match::MatchValue::Keyword(path.to_string())),
                        }),
                        ..Default::default()
                    }
                )),
            }],
            ..Default::default()
        };

        // Use delete by filter
        let result = self.client
            .delete_points(
                DeletePointsBuilder::new(&self.collection_name)
                    .points(filter)
            )
            .await;

        match result {
            Ok(_) => {
                eprintln!("Deleted embeddings matching path '{}' from Qdrant collection '{}'",
                         path, self.collection_name);
                Ok(())
            }
            Err(e) => {
                eprintln!("Failed to delete embeddings: {}", e);
                Err(anyhow::anyhow!("Failed to delete points from Qdrant: {}", e))
            }
        }
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
                    let status = match CollectionStatus::try_from(result.status) {
                        Ok(CollectionStatus::Green) => "healthy",
                        Ok(CollectionStatus::Yellow) => "degraded",
                        Ok(CollectionStatus::Red) => "unhealthy",
                        Ok(CollectionStatus::Grey) => "unknown",
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
