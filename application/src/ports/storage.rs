use async_trait::async_trait;
use shared::error::AppError;
use domain::repositories::{EmbeddingRepository, DocumentRepository, SessionRepository, CommandRepository};
use domain::value_objects::embedding::{Embedding, SearchResult};
use domain::EmbeddingStats;
use domain::value_objects::query::Query;
use domain::entities::document::Document;
use domain::entities::session::Session;
use domain::entities::command::Command;

/// Storage port for all repository operations
pub struct StorageService {
    embedding_repository: Box<dyn EmbeddingRepository>,
    document_repository: Box<dyn DocumentRepository>,
    session_repository: Box<dyn SessionRepository>,
    command_repository: Box<dyn CommandRepository>,
}

impl StorageService {
    pub fn new(
        embedding_repository: Box<dyn EmbeddingRepository>,
        document_repository: Box<dyn DocumentRepository>,
        session_repository: Box<dyn SessionRepository>,
        command_repository: Box<dyn CommandRepository>,
    ) -> Self {
        Self {
            embedding_repository,
            document_repository,
            session_repository,
            command_repository,
        }
    }

    // Embedding operations
    pub async fn save_embedding(&self, embedding: &Embedding) -> Result<(), AppError> {
        self.embedding_repository.save(embedding).await.map_err(|e| AppError::storage(e.to_string()))
    }

    pub async fn search_embeddings(&self, query: &Query, query_embedding: &Embedding) -> Result<Vec<domain::value_objects::embedding::SearchResult>, AppError> {
        self.embedding_repository.search_similar(query, query_embedding).await.map_err(|e| AppError::storage(e.to_string()))
    }

    // Document operations
    pub async fn save_document(&self, document: &Document) -> Result<(), AppError> {
        self.document_repository.save(document).await.map_err(|e| AppError::storage(e.to_string()))
    }

    pub async fn find_document_by_path(&self, path: &str) -> Result<Option<Document>, AppError> {
        self.document_repository.find_by_path(path).await.map_err(|e| AppError::storage(e.to_string()))
    }

    // Session operations
    pub async fn save_session(&self, session: &Session) -> Result<(), AppError> {
        self.session_repository.save(session).await.map_err(|e| AppError::storage(e.to_string()))
    }

    pub async fn find_session_by_id(&self, id: &str) -> Result<Option<Session>, AppError> {
        self.session_repository.find_by_id(id).await.map_err(|e| AppError::storage(e.to_string()))
    }

    // Command operations
    pub async fn save_command(&self, command: &Command) -> Result<(), AppError> {
        self.command_repository.save(command).await.map_err(|e| AppError::storage(e.to_string()))
    }

    pub async fn find_command_by_id(&self, id: &str) -> Result<Option<Command>, AppError> {
        self.command_repository.find_by_id(id).await.map_err(|e| AppError::storage(e.to_string()))
    }
}

/// Cache storage port
#[async_trait]
pub trait CacheStore: Send + Sync {
    /// Store a value with key
    async fn set(&self, key: &str, value: &str) -> Result<(), AppError>;

    /// Retrieve a value by key
    async fn get(&self, key: &str) -> Result<Option<String>, AppError>;

    /// Delete a value by key
    async fn delete(&self, key: &str) -> Result<(), AppError>;

    /// Check if key exists
    async fn exists(&self, key: &str) -> Result<bool, AppError>;

    /// Clear all cache entries
    async fn clear(&self) -> Result<(), AppError>;

    /// Get cache statistics
    async fn get_stats(&self) -> Result<CacheStats, AppError>;

    /// Set value with expiration (in seconds)
    async fn set_with_ttl(&self, key: &str, value: &str, ttl_seconds: u64) -> Result<(), AppError>;
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub total_keys: usize,
    pub total_size_bytes: u64,
    pub hit_rate: f32,
    pub miss_rate: f32,
    pub oldest_entry: Option<chrono::DateTime<chrono::Utc>>,
    pub newest_entry: Option<chrono::DateTime<chrono::Utc>>,
}

impl CacheStats {
    pub fn new(
        total_keys: usize,
        total_size_bytes: u64,
        hit_rate: f32,
        miss_rate: f32,
        oldest_entry: Option<chrono::DateTime<chrono::Utc>>,
        newest_entry: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Self {
        Self {
            total_keys,
            total_size_bytes,
            hit_rate,
            miss_rate,
            oldest_entry,
            newest_entry,
        }
    }

    pub fn total_requests(&self) -> f32 {
        self.hit_rate + self.miss_rate
    }

    pub fn efficiency(&self) -> f32 {
        if self.total_requests() > 0.0 {
            self.hit_rate / self.total_requests()
        } else {
            0.0
        }
    }
}

/// Vector store for embeddings
#[async_trait]
pub trait VectorStore: Send + Sync {
    /// Store embedding vector
    async fn store_vector(&self, id: &str, vector: &[f32], metadata: &str) -> Result<(), AppError>;

    /// Search for similar vectors
    async fn search_similar(
        &self,
        query_vector: &[f32],
        limit: usize,
        threshold: f32,
    ) -> Result<Vec<VectorSearchResult>, AppError>;

    /// Delete vector by ID
    async fn delete_vector(&self, id: &str) -> Result<(), AppError>;

    /// Get vector by ID
    async fn get_vector(&self, id: &str) -> Result<Option<Vec<f32>>, AppError>;

    /// Get vector statistics
    async fn get_stats(&self) -> Result<VectorStoreStats, AppError>;
}

/// Result of vector search
#[derive(Debug, Clone)]
pub struct VectorSearchResult {
    pub id: String,
    pub similarity: f32,
    pub metadata: String,
}

impl VectorSearchResult {
    pub fn new(id: String, similarity: f32, metadata: String) -> Self {
        Self { id, similarity, metadata }
    }
}

/// Vector store statistics
#[derive(Debug, Clone)]
pub struct VectorStoreStats {
    pub total_vectors: usize,
    pub dimensions: usize,
    pub index_size_bytes: u64,
    pub average_query_time_ms: f32,
    pub last_indexed: Option<chrono::DateTime<chrono::Utc>>,
}

impl VectorStoreStats {
    pub fn new(
        total_vectors: usize,
        dimensions: usize,
        index_size_bytes: u64,
        average_query_time_ms: f32,
        last_indexed: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Self {
        Self {
            total_vectors,
            dimensions,
            index_size_bytes,
            average_query_time_ms,
            last_indexed,
        }
    }
}