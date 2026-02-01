use super::super::value_objects::embedding::{Embedding, SearchResult};
use super::super::value_objects::query::Query;
use async_trait::async_trait;

/// Repository interface for embedding storage and retrieval
#[async_trait]
pub trait EmbeddingRepository: Send + Sync {
    /// Store a new embedding
    async fn save(&self, embedding: &Embedding) -> Result<(), RepositoryError>;

    /// Store multiple embeddings
    async fn save_batch(&self, embeddings: &[Embedding]) -> Result<(), RepositoryError>;

    /// Retrieve embedding by ID
    async fn find_by_id(&self, id: &str) -> Result<Option<Embedding>, RepositoryError>;

    /// Retrieve embeddings by document path
    async fn find_by_document(
        &self,
        document_path: &str,
    ) -> Result<Vec<Embedding>, RepositoryError>;

    /// Search for similar embeddings
    async fn search_similar(
        &self,
        query: &Query,
        query_embedding: &Embedding,
    ) -> Result<Vec<SearchResult>, RepositoryError>;

    /// Delete embedding by ID
    async fn delete(&self, id: &str) -> Result<(), RepositoryError>;

    /// Delete all embeddings for a document
    async fn delete_by_document(&self, document_path: &str) -> Result<(), RepositoryError>;

    /// Count total embeddings
    async fn count(&self) -> Result<usize, RepositoryError>;

    /// List all document paths
    async fn list_documents(&self) -> Result<Vec<String>, RepositoryError>;

    /// Check if embedding exists
    async fn exists(&self, id: &str) -> Result<bool, RepositoryError>;

    /// Get embedding statistics
    async fn get_stats(&self) -> Result<EmbeddingStats, RepositoryError>;
}

/// Statistics about embeddings in the repository
#[derive(Debug, Clone)]
pub struct EmbeddingStats {
    total_embeddings: usize,
    total_documents: usize,
    average_embedding_size: f32,
    last_updated: chrono::DateTime<chrono::Utc>,
}

impl EmbeddingStats {
    pub fn new(
        total_embeddings: usize,
        total_documents: usize,
        average_embedding_size: f32,
        last_updated: chrono::DateTime<chrono::Utc>,
    ) -> Self {
        Self {
            total_embeddings,
            total_documents,
            average_embedding_size,
            last_updated,
        }
    }

    pub fn total_embeddings(&self) -> usize {
        self.total_embeddings
    }

    pub fn total_documents(&self) -> usize {
        self.total_documents
    }

    pub fn average_embedding_size(&self) -> f32 {
        self.average_embedding_size
    }

    pub fn last_updated(&self) -> chrono::DateTime<chrono::Utc> {
        self.last_updated
    }
}

/// Repository error types
#[derive(Debug, Clone)]
pub enum RepositoryError {
    ConnectionError(String),
    NotFound(String),
    ValidationError(String),
    StorageError(String),
    SerializationError(String),
    DuplicateError(String),
}

impl std::fmt::Display for RepositoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RepositoryError::ConnectionError(msg) => write!(f, "Connection error: {}", msg),
            RepositoryError::NotFound(msg) => write!(f, "Not found: {}", msg),
            RepositoryError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
            RepositoryError::StorageError(msg) => write!(f, "Storage error: {}", msg),
            RepositoryError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            RepositoryError::DuplicateError(msg) => write!(f, "Duplicate error: {}", msg),
        }
    }
}

impl std::error::Error for RepositoryError {}
