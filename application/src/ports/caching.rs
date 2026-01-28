use async_trait::async_trait;
use shared::error::AppError;

/// Caching port for command and query results
#[async_trait]
pub trait Cache: Send + Sync {
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

    /// Set value with expiration (in seconds)
    async fn set_with_ttl(&self, key: &str, value: &str, ttl_seconds: u64) -> Result<(), AppError>;

    /// Get multiple values by keys
    async fn get_multiple(&self, keys: &[String]) -> Result<Vec<Option<String>>, AppError>;

    /// Set multiple values
    async fn set_multiple(&self, entries: &[(&str, &str)]) -> Result<(), AppError>;
}

/// Command cache for storing generated commands
#[async_trait]
pub trait CommandCache: Send + Sync {
    /// Store a command for a query
    async fn store_command(&self, query: &str, command: &CachedCommand) -> Result<(), AppError>;

    /// Retrieve commands for a query
    async fn get_commands(&self, query: &str) -> Result<Vec<CachedCommand>, AppError>;

    /// Store multiple commands for a query
    async fn store_commands(&self, query: &str, commands: &[CachedCommand]) -> Result<(), AppError>;

    /// Get popular commands
    async fn get_popular_commands(&self, limit: usize) -> Result<Vec<CachedCommand>, AppError>;

    /// Increment command usage count
    async fn increment_usage(&self, command_id: &str) -> Result<(), AppError>;

    /// Get command usage statistics
    async fn get_command_stats(&self, command_id: &str) -> Result<Option<CommandStats>, AppError>;

    /// Clean up old entries
    async fn cleanup_old_entries(&self, older_than_days: u32) -> Result<usize, AppError>;
}

/// Cached command with metadata
#[derive(Debug, Clone)]
pub struct CachedCommand {
    pub id: String,
    pub command: String,
    pub description: Option<String>,
    pub label: Option<String>,
    pub confidence: f32,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used: Option<chrono::DateTime<chrono::Utc>>,
    pub usage_count: u32,
    pub query_hash: String,
}

impl CachedCommand {
    pub fn new(
        id: String,
        command: String,
        query_hash: String,
    ) -> Self {
        Self {
            id,
            command,
            description: None,
            label: None,
            confidence: 0.0,
            created_at: chrono::Utc::now(),
            last_used: None,
            usage_count: 0,
            query_hash,
        }
    }

    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    pub fn with_label(mut self, label: String) -> Self {
        self.label = Some(label);
        self
    }

    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence;
        self
    }

    pub fn mark_used(&mut self) {
        self.last_used = Some(chrono::Utc::now());
        self.usage_count += 1;
    }

    pub fn is_recent(&self, days: u32) -> bool {
        if let Some(last_used) = self.last_used {
            let duration = chrono::Utc::now() - last_used;
            duration.num_days() <= days as i64
        } else {
            false
        }
    }

    pub fn popularity_score(&self) -> f32 {
        let recency_factor = if let Some(last_used) = self.last_used {
            let hours_ago = (chrono::Utc::now() - last_used).num_hours() as f32;
            1.0 / (1.0 + hours_ago / 24.0) // Decay over time
        } else {
            0.0
        };

        let usage_factor = (self.usage_count as f32).ln_1p();
        self.confidence * 0.5 + usage_factor * 0.3 + recency_factor * 0.2
    }
}

/// Command usage statistics
#[derive(Debug, Clone)]
pub struct CommandStats {
    pub usage_count: u32,
    pub first_used: chrono::DateTime<chrono::Utc>,
    pub last_used: chrono::DateTime<chrono::Utc>,
    pub average_usage_per_day: f32,
}

impl CommandStats {
    pub fn new(
        usage_count: u32,
        first_used: chrono::DateTime<chrono::Utc>,
        last_used: chrono::DateTime<chrono::Utc>,
    ) -> Self {
        let days_since_first = (chrono::Utc::now() - first_used).num_days() as f32;
        let average_usage_per_day = if days_since_first > 0.0 {
            usage_count as f32 / days_since_first
        } else {
            usage_count as f32
        };

        Self {
            usage_count,
            first_used,
            last_used,
            average_usage_per_day,
        }
    }

    pub fn usage_frequency(&self) -> &'static str {
        if self.average_usage_per_day >= 1.0 {
            "Daily"
        } else if self.average_usage_per_day >= 0.14 {
            "Weekly"
        } else if self.average_usage_per_day >= 0.02 {
            "Monthly"
        } else {
            "Rare"
        }
    }
}

/// Query cache for storing query results
#[async_trait]
pub trait QueryCache: Send + Sync {
    /// Store query result
    async fn store_query_result(&self, query: &str, result: &QueryResult) -> Result<(), AppError>;

    /// Get cached query result
    async fn get_query_result(&self, query: &str) -> Result<Option<QueryResult>, AppError>;

    /// Store embedding for query
    async fn store_query_embedding(&self, query: &str, embedding: &[f32]) -> Result<(), AppError>;

    /// Get cached query embedding
    async fn get_query_embedding(&self, query: &str) -> Result<Option<Vec<f32>>, AppError>;

    /// Get similar cached queries
    async fn get_similar_queries(&self, query_embedding: &[f32], limit: usize) -> Result<Vec<SimilarQuery>, AppError>;
}

/// Cached query result
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub query: String,
    pub results: Vec<String>,
    pub execution_time_ms: u64,
    pub cached_at: chrono::DateTime<chrono::Utc>,
    pub ttl_seconds: u32,
}

impl QueryResult {
    pub fn new(
        query: String,
        results: Vec<String>,
        execution_time_ms: u64,
        ttl_seconds: u32,
    ) -> Self {
        Self {
            query,
            results,
            execution_time_ms,
            cached_at: chrono::Utc::now(),
            ttl_seconds,
        }
    }

    pub fn is_expired(&self) -> bool {
        let elapsed = (chrono::Utc::now() - self.cached_at).num_seconds() as u32;
        elapsed > self.ttl_seconds
    }

    pub fn age_seconds(&self) -> u64 {
        (chrono::Utc::now() - self.cached_at).num_seconds() as u64
    }
}

/// Similar query with similarity score
#[derive(Debug, Clone)]
pub struct SimilarQuery {
    pub query: String,
    pub similarity: f32,
    pub cached_at: chrono::DateTime<chrono::Utc>,
}

impl SimilarQuery {
    pub fn new(query: String, similarity: f32) -> Self {
        Self {
            query,
            similarity,
            cached_at: chrono::Utc::now(),
        }
    }
}