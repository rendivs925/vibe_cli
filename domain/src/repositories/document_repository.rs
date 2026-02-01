use super::super::entities::document::{Document, DocumentType};
use super::embedding_repository::RepositoryError;
use async_trait::async_trait;

/// Repository interface for document storage and retrieval
#[async_trait]
pub trait DocumentRepository: Send + Sync {
    /// Store a new document
    async fn save(&self, document: &Document) -> Result<(), RepositoryError>;

    /// Retrieve document by ID
    async fn find_by_id(&self, id: &str) -> Result<Option<Document>, RepositoryError>;

    /// Retrieve document by path
    async fn find_by_path(&self, path: &str) -> Result<Option<Document>, RepositoryError>;

    /// List all documents
    async fn list_all(&self) -> Result<Vec<Document>, RepositoryError>;

    /// List documents by type
    async fn list_by_type(
        &self,
        document_type: &DocumentType,
    ) -> Result<Vec<Document>, RepositoryError>;

    /// Search documents by content
    async fn search_content(&self, query: &str) -> Result<Vec<Document>, RepositoryError>;

    /// Search documents by path pattern
    async fn search_by_path(&self, pattern: &str) -> Result<Vec<Document>, RepositoryError>;

    /// Update document content
    async fn update(&self, document: &Document) -> Result<(), RepositoryError>;

    /// Delete document by ID
    async fn delete(&self, id: &str) -> Result<(), RepositoryError>;

    /// Delete document by path
    async fn delete_by_path(&self, path: &str) -> Result<(), RepositoryError>;

    /// Count total documents
    async fn count(&self) -> Result<usize, RepositoryError>;

    /// Count documents by type
    async fn count_by_type(&self, document_type: &DocumentType) -> Result<usize, RepositoryError>;

    /// Check if document exists
    async fn exists(&self, id: &str) -> Result<bool, RepositoryError>;

    /// Check if document exists by path
    async fn exists_by_path(&self, path: &str) -> Result<bool, RepositoryError>;

    /// Get document statistics
    async fn get_stats(&self) -> Result<DocumentStats, RepositoryError>;

    /// Find recently modified documents
    async fn find_recent(&self, limit: usize) -> Result<Vec<Document>, RepositoryError>;

    /// Find documents modified within a date range
    async fn find_by_date_range(
        &self,
        start: chrono::DateTime<chrono::Utc>,
        end: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<Document>, RepositoryError>;
}

/// Statistics about documents in the repository
#[derive(Debug, Clone)]
pub struct DocumentStats {
    total_documents: usize,
    total_size_bytes: u64,
    average_size_bytes: f64,
    documents_by_type: std::collections::HashMap<String, usize>,
    last_updated: chrono::DateTime<chrono::Utc>,
}

impl DocumentStats {
    pub fn new(
        total_documents: usize,
        total_size_bytes: u64,
        documents_by_type: std::collections::HashMap<String, usize>,
        last_updated: chrono::DateTime<chrono::Utc>,
    ) -> Self {
        let average_size_bytes = if total_documents > 0 {
            total_size_bytes as f64 / total_documents as f64
        } else {
            0.0
        };

        Self {
            total_documents,
            total_size_bytes,
            average_size_bytes,
            documents_by_type,
            last_updated,
        }
    }

    pub fn total_documents(&self) -> usize {
        self.total_documents
    }

    pub fn total_size_bytes(&self) -> u64 {
        self.total_size_bytes
    }

    pub fn average_size_bytes(&self) -> f64 {
        self.average_size_bytes
    }

    pub fn documents_by_type(&self) -> &std::collections::HashMap<String, usize> {
        &self.documents_by_type
    }

    pub fn last_updated(&self) -> chrono::DateTime<chrono::Utc> {
        self.last_updated
    }
}
