use async_trait::async_trait;
use shared::error::AppError;
use domain::value_objects::query::{Query, QueryResult};
use domain::value_objects::embedding::{Embedding, SearchResult};
use crate::ports::{AiClient, StorageService, Cache};

/// Use case for RAG (Retrieval-Augmented Generation) operations
pub struct RagUseCase {
    ai_client: Box<dyn AiClient>,
    storage: StorageService,
    cache: Box<dyn Cache>,
}

impl RagUseCase {
    pub fn new(
        ai_client: Box<dyn AiClient>,
        storage: StorageService,
        cache: Box<dyn Cache>,
    ) -> Self {
        Self {
            ai_client,
            storage,
            cache,
        }
    }

    /// Process a query using RAG
    pub async fn process_query(&self, query_text: &str) -> Result<RagResponse, AppError> {
        // Check cache first
        let cache_key = format!("rag:{}", md5::compute(query_text.as_bytes()));
        if let Some(cached_result) = self.cache.get(&cache_key).await? {
            return Ok(RagResponse::cached(cached_result));
        }

        // Generate embedding for query
        let query_embedding = self.ai_client.generate_embedding(query_text).await?;
        
        // Search for relevant documents
        let query_obj = Query::new(query_text.to_string());
        let search_results = self.storage.search_embeddings(&query_obj, &Embedding::new(
            "query".to_string(),
            query_embedding,
            query_text.to_string(),
            "".to_string(),
        )).await?;

        // Build context from search results
        let context = self.build_context(&search_results);
        
        // Generate augmented response
        let prompt = self.build_rag_prompt(query_text, &context);
        let response = self.ai_client.generate_response_with_context(&prompt, &context).await?;

        // Cache the result
        self.cache.set(&cache_key, &response).await?;

        Ok(RagResponse::new(
            query_text.to_string(),
            response,
            search_results,
            false,
        ))
    }

    /// Stream a RAG response
    pub async fn stream_query(
        &self,
        query_text: &str,
    ) -> Result<Box<dyn futures::Stream<Item = Result<String, AppError>> + Send>, AppError> {
        // Generate embedding for query
        let query_embedding = self.ai_client.generate_embedding(query_text).await?;
        
        // Search for relevant documents
        let query_obj = Query::new(query_text.to_string());
        let search_results = self.storage.search_embeddings(&query_obj, &Embedding::new(
            "query".to_string(),
            query_embedding,
            query_text.to_string(),
            "".to_string(),
        )).await?;

        // Build context
        let context = self.build_context(&search_results);
        
        // Build prompt and stream response
        let prompt = self.build_rag_prompt(query_text, &context);
        self.ai_client.stream_response(&prompt).await
    }

    /// Index a document for RAG
    pub async fn index_document(&self, document_text: &str, document_path: &str) -> Result<(), AppError> {
        // Split document into chunks
        let chunks = self.chunk_document(document_text);
        
        for (i, chunk) in chunks.iter().enumerate() {
            // Generate embedding for chunk
            let embedding = self.ai_client.generate_embedding(chunk).await?;
            
            // Store embedding
            let doc_embedding = Embedding::new(
                format!("{}:{}", document_path, i),
                embedding,
                chunk.clone(),
                document_path.to_string(),
            );
            
            self.storage.save_embedding(&doc_embedding).await?;
        }

        Ok(())
    }

    /// Batch index multiple documents
    pub async fn index_documents(&self, documents: &[(&str, &str)]) -> Result<(), AppError> {
        for (path, content) in documents {
            self.index_document(content, path).await?;
        }
        Ok(())
    }

    /// Get RAG statistics
    pub async fn get_stats(&self) -> Result<RagStats, AppError> {
        // This would fetch statistics from storage and cache
        Ok(RagStats::new(100, 50, 25, 0.8))
    }

    // Private helper methods
    fn build_context(&self, search_results: &[SearchResult]) -> Vec<String> {
        search_results
            .iter()
            .take(5) // Use top 5 results
            .map(|result| result.embedding().text())
            .cloned()
            .collect()
    }

    fn build_rag_prompt(&self, query: &str, context: &[String]) -> String {
        let context_text = context.iter().enumerate()
            .map(|(i, ctx)| format!("{}. {}", i + 1, ctx))
            .collect::<Vec<_>>()
            .join("\n\n");

        format!(
            "Based on the following context, please answer the question:\n\nContext:\n{}\n\nQuestion: {}\n\nAnswer:",
            context_text, query
        )
    }

    fn chunk_document(&self, document: &str) -> Vec<String> {
        // Simple chunking - split by paragraphs with overlap
        let paragraphs: Vec<&str> = document.split("\n\n").collect();
        let mut chunks = Vec::new();
        let chunk_size = 500; // words per chunk
        let overlap = 50; // words overlap

        let mut current_chunk = Vec::new();
        let mut word_count = 0;

        for paragraph in paragraphs {
            let words: Vec<&str> = paragraph.split_whitespace().collect();
            
            if word_count + words.len() > chunk_size && !current_chunk.is_empty() {
                chunks.push(current_chunk.join(" "));
                current_chunk.clear();
                word_count = 0;
                
                // Add overlap
                let overlap_words = words.iter().rev().take(overlap).rev().cloned().collect::<Vec<_>>();
                current_chunk.extend(overlap_words);
                word_count = overlap_words.len();
            }
            
            current_chunk.extend(words);
            word_count += words.len();
        }

        if !current_chunk.is_empty() {
            chunks.push(current_chunk.join(" "));
        }

        chunks
    }
}

/// RAG response
#[derive(Debug, Clone)]
pub struct RagResponse {
    query: String,
    response: String,
    sources: Vec<SearchResult>,
    from_cache: bool,
}

impl RagResponse {
    pub fn new(
        query: String,
        response: String,
        sources: Vec<SearchResult>,
        from_cache: bool,
    ) -> Self {
        Self {
            query,
            response,
            sources,
            from_cache,
        }
    }

    pub fn cached(response: String) -> Self {
        Self {
            query: String::new(),
            response,
            sources: Vec::new(),
            from_cache: true,
        }
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn response(&self) -> &str {
        &self.response
    }

    pub fn sources(&self) -> &[SearchResult] {
        &self.sources
    }

    pub fn is_from_cache(&self) -> bool {
        self.from_cache
    }

    pub fn source_count(&self) -> usize {
        self.sources.len()
    }
}

/// RAG statistics
#[derive(Debug, Clone)]
pub struct RagStats {
    pub total_documents: usize,
    pub total_embeddings: usize,
    pub average_chunks_per_document: f32,
    pub cache_hit_rate: f32,
}

impl RagStats {
    pub fn new(
        total_documents: usize,
        total_embeddings: usize,
        average_chunks_per_document: f32,
        cache_hit_rate: f32,
    ) -> Self {
        Self {
            total_documents,
            total_embeddings,
            average_chunks_per_document,
            cache_hit_rate,
        }
    }

    pub fn total_documents(&self) -> usize {
        self.total_documents
    }

    pub fn total_embeddings(&self) -> usize {
        self.total_embeddings
    }

    pub fn average_chunks_per_document(&self) -> f32 {
        self.average_chunks_per_document
    }

    pub fn cache_hit_rate(&self) -> f32 {
        self.cache_hit_rate
    }
}

#[async_trait]
pub trait AsyncRagService: Send + Sync {
    async fn process_query(&self, query: &str) -> Result<RagResponse, AppError>;
    async fn stream_query(&self, query: &str) -> Result<Box<dyn futures::Stream<Item = Result<String, AppError>> + Send>, AppError>;
    async fn index_document(&self, document_text: &str, document_path: &str) -> Result<(), AppError>;
}