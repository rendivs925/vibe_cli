use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

/// Query value object representing a search query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Query {
    text: String,
    context: SmallVec<[String; 4]>,
    max_results: usize,
    min_similarity: f32,
}

impl Query {
    pub fn new(text: String) -> Self {
        Self {
            text,
            context: SmallVec::new(),
            max_results: 10,
            min_similarity: 0.7,
        }
    }

    pub fn with_context(mut self, context: Vec<String>) -> Self {
        self.context = context.into();
        self
    }

    pub fn with_max_results(mut self, max_results: usize) -> Self {
        self.max_results = max_results;
        self
    }

    pub fn with_min_similarity(mut self, min_similarity: f32) -> Self {
        self.min_similarity = min_similarity.clamp(0.0, 1.0);
        self
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn context(&self) -> &[String] {
        &self.context
    }

    pub fn max_results(&self) -> usize {
        self.max_results
    }

    pub fn min_similarity(&self) -> f32 {
        self.min_similarity
    }

    pub fn add_context(&mut self, context: String) {
        self.context.push(context);
    }

    pub fn clear_context(&mut self) {
        self.context.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }

    pub fn word_count(&self) -> usize {
        self.text.split_whitespace().count()
    }

    pub fn character_count(&self) -> usize {
        self.text.len()
    }
}

/// Query result containing relevant documents and their context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    query: Query,
    results: SmallVec<[super::embedding::SearchResult; 8]>,
    total_found: usize,
    execution_time_ms: u64,
}

impl QueryResult {
    pub fn new(
        query: Query,
        results: SmallVec<[super::embedding::SearchResult; 8]>,
        total_found: usize,
        execution_time_ms: u64,
    ) -> Self {
        Self {
            query,
            results,
            total_found,
            execution_time_ms,
        }
    }

    pub fn query(&self) -> &Query {
        &self.query
    }

    pub fn results(&self) -> &[super::embedding::SearchResult] {
        &self.results
    }

    pub fn total_found(&self) -> usize {
        self.total_found
    }

    pub fn execution_time_ms(&self) -> u64 {
        self.execution_time_ms
    }

    pub fn has_results(&self) -> bool {
        !self.results.is_empty()
    }

    pub fn len(&self) -> usize {
        self.results.len()
    }

    pub fn is_empty(&self) -> bool {
        self.results.is_empty()
    }

    pub fn top_result(&self) -> Option<&super::embedding::SearchResult> {
        self.results.first()
    }

    pub fn relevant_results(&self) -> Vec<&super::embedding::SearchResult> {
        self.results
            .iter()
            .filter(|r| r.similarity() >= self.query.min_similarity())
            .collect()
    }
}
