use async_trait::async_trait;
use shared::error::AppError;
use domain::entities::document::{Document, DocumentType};
use domain::services::document_analyzer::DocumentAnalyzer;
use crate::ports::{DocumentReader, FileScanner, StorageService};

/// Use case for document processing and analysis
pub struct DocumentUseCase {
    document_reader: Box<dyn DocumentReader>,
    file_scanner: Box<dyn FileScanner>,
    storage: StorageService,
    analyzer: DocumentAnalyzer,
}

impl DocumentUseCase {
    pub fn new(
        document_reader: Box<dyn DocumentReader>,
        file_scanner: Box<dyn FileScanner>,
        storage: StorageService,
        analyzer: DocumentAnalyzer,
    ) -> Self {
        Self {
            document_reader,
            file_scanner,
            storage,
            analyzer,
        }
    }

    /// Process and index a single document
    pub async fn process_document(&self, file_path: &str) -> Result<DocumentProcessingResult, AppError> {
        // Read document
        let document = self.document_reader.read_document(file_path).await?;

        // Analyze document
        let analysis = self.analyzer.analyze(&document);

        // Store document
        self.storage.save_document(&document).await?;

        Ok(DocumentProcessingResult::new(
            document,
            analysis,
            false,
        ))
    }

    /// Process and index multiple documents
    pub async fn process_documents(&self, file_paths: &[String]) -> Result<Vec<DocumentProcessingResult>, AppError> {
        let mut results = Vec::new();

        for file_path in file_paths {
            match self.process_document(file_path).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    // Log error but continue processing other documents
                    eprintln!("Error processing {}: {}", file_path, e);
                }
            }
        }

        Ok(results)
    }

    /// Scan directory and process all supported documents
    pub async fn process_directory(&self, dir_path: &str, recursive: bool) -> Result<DirectoryProcessingResult, AppError> {
        // Find all documents
        let file_paths = self.file_scanner.scan_directory(dir_path, recursive).await?;

        // Process documents
        let results = self.process_documents(&file_paths).await?;

        let successful = results.iter().filter(|r| r.analysis().word_count() > 0).count();
        let failed = results.len() - successful;

        Ok(DirectoryProcessingResult::new(
            file_paths.len(),
            successful,
            failed,
            results,
        ))
    }

    /// Search documents by content
    pub async fn search_documents(&self, query: &str, limit: usize) -> Result<Vec<DocumentSearchResult>, AppError> {
        // This would use the storage service to search
        // For now, return empty results
        Ok(vec![])
    }

    /// Get document analysis
    pub async fn get_document_analysis(&self, document_id: &str) -> Result<Option<domain::services::document_analyzer::DocumentAnalysis>, AppError> {
        // Find document
        let document = self.storage.find_document_by_id(document_id).await?;
        
        match document {
            Some(doc) => {
                let analysis = self.analyzer.analyze(&doc);
                Ok(Some(analysis))
            }
            None => Ok(None),
        }
    }

    /// Find similar documents
    pub async fn find_similar_documents(&self, document_id: &str, limit: usize) -> Result<Vec<DocumentSimilarityResult>, AppError> {
        // Find target document
        let target_document = self.storage.find_document_by_id(document_id).await?;
        
        match target_document {
            Some(target) => {
                // Get all documents for comparison
                let all_documents = self.storage.list_all_documents().await?;
                
                // Find similar ones
                let similar_scores = self.analyzer.find_similar_documents(&target, &all_documents);
                
                let results: Vec<DocumentSimilarityResult> = similar_scores
                    .into_iter()
                    .take(limit)
                    .map(|score| DocumentSimilarityResult::new(score.document_id().to_string(), score.similarity()))
                    .collect();

                Ok(results)
            }
            None => Ok(vec![]),
        }
    }

    /// Get document statistics
    pub async fn get_document_stats(&self) -> Result<DocumentStats, AppError> {
        // This would fetch from storage
        Ok(DocumentStats::new(100, 50, 25, 1000))
    }
}

/// Result of document processing
#[derive(Debug, Clone)]
pub struct DocumentProcessingResult {
    document: Document,
    analysis: domain::services::document_analyzer::DocumentAnalysis,
    from_cache: bool,
}

impl DocumentProcessingResult {
    pub fn new(
        document: Document,
        analysis: domain::services::document_analyzer::DocumentAnalysis,
        from_cache: bool,
    ) -> Self {
        Self {
            document,
            analysis,
            from_cache,
        }
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn analysis(&self) -> &domain::services::document_analyzer::DocumentAnalysis {
        &self.analysis
    }

    pub fn is_from_cache(&self) -> bool {
        self.from_cache
    }
}

/// Result of directory processing
#[derive(Debug, Clone)]
pub struct DirectoryProcessingResult {
    total_files: usize,
    successful: usize,
    failed: usize,
    results: Vec<DocumentProcessingResult>,
}

impl DirectoryProcessingResult {
    pub fn new(
        total_files: usize,
        successful: usize,
        failed: usize,
        results: Vec<DocumentProcessingResult>,
    ) -> Self {
        Self {
            total_files,
            successful,
            failed,
            results,
        }
    }

    pub fn total_files(&self) -> usize {
        self.total_files
    }

    pub fn successful(&self) -> usize {
        self.successful
    }

    pub fn failed(&self) -> usize {
        self.failed
    }

    pub fn results(&self) -> &[DocumentProcessingResult] {
        &self.results
    }

    pub fn success_rate(&self) -> f32 {
        if self.total_files > 0 {
            self.successful as f32 / self.total_files as f32
        } else {
            0.0
        }
    }
}

/// Document search result
#[derive(Debug, Clone)]
pub struct DocumentSearchResult {
    document_id: String,
    title: Option<String>,
    snippet: String,
    relevance_score: f32,
}

impl DocumentSearchResult {
    pub fn new(
        document_id: String,
        title: Option<String>,
        snippet: String,
        relevance_score: f32,
    ) -> Self {
        Self {
            document_id,
            title,
            snippet,
            relevance_score,
        }
    }

    pub fn document_id(&self) -> &str {
        &self.document_id
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn snippet(&self) -> &str {
        &self.snippet
    }

    pub fn relevance_score(&self) -> f32 {
        self.relevance_score
    }
}

/// Document similarity result
#[derive(Debug, Clone)]
pub struct DocumentSimilarityResult {
    document_id: String,
    similarity: f32,
}

impl DocumentSimilarityResult {
    pub fn new(document_id: String, similarity: f32) -> Self {
        Self { document_id, similarity }
    }

    pub fn document_id(&self) -> &str {
        &self.document_id
    }

    pub fn similarity(&self) -> f32 {
        self.similarity
    }
}

/// Document statistics
#[derive(Debug, Clone)]
pub struct DocumentStats {
    pub total_documents: usize,
    pub total_size_bytes: u64,
    pub average_size_bytes: f64,
    pub documents_by_type: std::collections::HashMap<String, usize>,
}

impl DocumentStats {
    pub fn new(
        total_documents: usize,
        total_size_bytes: u64,
        average_size_bytes: f64,
        documents_by_type: std::collections::HashMap<String, usize>,
    ) -> Self {
        Self {
            total_documents,
            total_size_bytes,
            average_size_bytes,
            documents_by_type,
        }
    }
}

#[async_trait]
pub trait AsyncDocumentService: Send + Sync {
    async fn process_document(&self, file_path: &str) -> Result<DocumentProcessingResult, AppError>;
    async fn process_directory(&self, dir_path: &str, recursive: bool) -> Result<DirectoryProcessingResult, AppError>;
    async fn search_documents(&self, query: &str, limit: usize) -> Result<Vec<DocumentSearchResult>, AppError>;
}