use crate::embedding_storage::EmbeddingStorage;
use crate::qdrant_storage::QdrantStorage;
use domain::models::Embedding;
use shared::types::Result;
use std::collections::HashMap;
use std::path::Path;

/// Hybrid storage manager that automatically chooses between Qdrant and SQLite
pub struct HybridStorage {
    qdrant: Option<QdrantStorage>,
    sqlite: EmbeddingStorage,
    use_qdrant: bool,
}

impl HybridStorage {
    /// Create hybrid storage with automatic fallback
    pub async fn new(
        qdrant_url: Option<String>,
        sqlite_path: impl AsRef<Path>,
        collection_name: String,
        vector_dim: usize,
    ) -> Result<Self> {
        let sqlite = EmbeddingStorage::new(&sqlite_path).await?;
        let qdrant = if let Some(url) = qdrant_url {
            match QdrantStorage::new(Some(url), collection_name, vector_dim).await {
                Ok(storage) => Some(storage),
                Err(e) => {
                    eprintln!("Warning: Qdrant initialization failed: {}", e);
                    None
                }
            }
        } else {
            None
        };

        let use_qdrant = qdrant.is_some();

        Ok(Self {
            qdrant,
            sqlite,
            use_qdrant,
        })
    }

    /// Insert embeddings using the best available storage
    pub async fn insert_embeddings(&self, embeddings: Vec<Embedding>) -> Result<()> {
        if self.use_qdrant {
            if let Some(qdrant) = &self.qdrant {
                qdrant.insert_embeddings(embeddings).await
            } else {
                self.sqlite.insert_embeddings(embeddings).await
            }
        } else {
            self.sqlite.insert_embeddings(embeddings).await
        }
    }

    /// Search for similar embeddings
    pub async fn search_similar(
        &self,
        query_vector: &[f32],
        limit: usize,
    ) -> Result<Vec<Embedding>> {
        if self.use_qdrant {
            if let Some(qdrant) = &self.qdrant {
                qdrant.search_similar(query_vector, limit).await
            } else {
                self.fallback_search(query_vector, limit).await
            }
        } else {
            self.fallback_search(query_vector, limit).await
        }
    }

    /// Fallback search using SQLite
    async fn fallback_search(&self, query_vector: &[f32], limit: usize) -> Result<Vec<Embedding>> {
        let all_embeddings = self.sqlite.get_all_embeddings().await?;
        let mut scored: Vec<(f32, Embedding)> = all_embeddings
            .into_iter()
            .map(|emb| {
                let score = Self::cosine_similarity(query_vector, &emb.vector);
                (score, emb)
            })
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        Ok(scored.into_iter().take(limit).map(|(_, emb)| emb).collect())
    }

    /// Cosine similarity calculation
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            dot_product / (norm_a * norm_b)
        }
    }

    /// Get all embeddings
    pub async fn get_all_embeddings(&self) -> Result<Vec<Embedding>> {
        // Always use SQLite as the source of truth for getting all embeddings
        self.sqlite.get_all_embeddings().await
    }

    /// Get file hash
    pub async fn get_file_hash(&self, path: String) -> Result<Option<String>> {
        self.sqlite.get_file_hash(path).await
    }

    /// Upsert file hash
    pub async fn upsert_file_hash(&self, path: String, hash: String) -> Result<()> {
        self.sqlite.upsert_file_hash(path, hash).await
    }

    /// Delete embeddings for path
    pub async fn delete_embeddings_for_path(&self, path: String) -> Result<()> {
        if self.use_qdrant {
            if let Some(qdrant) = &self.qdrant {
                qdrant.delete_embeddings_for_path(path.as_str()).await?;
            }
        }
        self.sqlite.delete_embeddings_for_path(path).await
    }

    /// Get storage statistics
    pub async fn get_stats(&self) -> Result<HashMap<String, String>> {
        let mut stats = HashMap::new();

        if self.use_qdrant {
            if let Some(qdrant) = &self.qdrant {
                if let Ok(qdrant_stats) = qdrant.get_stats().await {
                    stats.extend(qdrant_stats);
                }
            }
        }

        // Add hybrid-specific stats
        stats.insert("hybrid_mode".to_string(), self.use_qdrant.to_string());
        stats.insert(
            "primary_storage".to_string(),
            if self.use_qdrant { "qdrant" } else { "sqlite" }.to_string(),
        );

        Ok(stats)
    }

    /// Check if Qdrant is available and working
    pub fn is_qdrant_available(&self) -> bool {
        self.use_qdrant && self.qdrant.is_some()
    }

    /// Force fallback to SQLite
    pub fn force_sqlite_fallback(&mut self) {
        self.use_qdrant = false;
    }
}
