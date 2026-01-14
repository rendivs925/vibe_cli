use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
/// Smart routing and caching system for hybrid local/remote AI processing
/// Routes queries between local models and remote ChatGPT based on complexity and cost
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Query routing decision
#[derive(Debug, Clone, PartialEq)]
pub enum QueryDestination {
    /// Use local AI models (fast, free, private)
    Local,
    /// Use remote ChatGPT via browser (slower, free via web, requires authentication)
    Remote,
    /// Ask user to choose (for borderline cases)
    AskUser,
}

/// Query complexity analysis
#[derive(Debug, Clone)]
pub struct QueryComplexity {
    pub score: f32,             // 0.0 to 1.0 complexity score
    pub has_code: bool,         // Contains code snippets
    pub has_architecture: bool, // Architecture or design questions
    pub has_research: bool,     // Requires external knowledge/research
    pub word_count: usize,      // Length indicator
    pub technical_terms: usize, // Count of technical keywords
}

/// Cost tracking for optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryCost {
    pub query_id: String,
    pub destination: String,
    pub processing_time_ms: u64,
    pub cost_cents: u64, // 0 for local/remote (web interface)
    pub timestamp: DateTime<Utc>,
    pub success: bool,
}

/// Cached query response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedResponse {
    pub query_hash: String,
    pub response: String,
    pub destination: String,
    pub timestamp: DateTime<Utc>,
    pub ttl_seconds: u64,
}

/// Smart router for AI queries
pub struct SmartRouter {
    /// Complexity thresholds for routing decisions
    complexity_thresholds: RoutingThresholds,
    /// Response cache for performance
    response_cache: Arc<RwLock<HashMap<String, CachedResponse>>>,
    /// Cost tracking for optimization
    cost_history: Arc<RwLock<Vec<QueryCost>>>,
    /// User preferences for routing
    user_preferences: UserRoutingPreferences,
}

#[derive(Debug, Clone)]
pub struct RoutingThresholds {
    /// Complexity score above which to use remote (0.0-1.0)
    pub remote_threshold: f32,
    /// Complexity score below which to always use local (0.0-1.0)
    pub local_threshold: f32,
    /// Cache TTL in seconds
    pub cache_ttl_seconds: u64,
    /// Maximum cache size
    pub max_cache_entries: usize,
}

#[derive(Debug, Clone)]
pub struct UserRoutingPreferences {
    /// Prefer local processing even for complex queries
    pub prefer_local: bool,
    /// Allow remote processing
    pub allow_remote: bool,
    /// Ask for confirmation on borderline cases
    pub confirm_complex: bool,
    /// Cache responses
    pub enable_caching: bool,
}

/// Query analysis result
#[derive(Debug)]
pub struct QueryAnalysis {
    pub complexity: QueryComplexity,
    pub recommended_destination: QueryDestination,
    pub confidence: f32,
    pub reasoning: Vec<String>,
}

impl Default for RoutingThresholds {
    fn default() -> Self {
        Self {
            remote_threshold: 0.7,   // Complex queries go remote
            local_threshold: 0.3,    // Simple queries stay local
            cache_ttl_seconds: 3600, // 1 hour cache
            max_cache_entries: 1000, // Reasonable cache size
        }
    }
}

impl Default for UserRoutingPreferences {
    fn default() -> Self {
        Self {
            prefer_local: false,
            allow_remote: true,
            confirm_complex: true,
            enable_caching: true,
        }
    }
}

impl SmartRouter {
    /// Create a new smart router with default settings
    pub fn new() -> Self {
        Self {
            complexity_thresholds: RoutingThresholds::default(),
            response_cache: Arc::new(RwLock::new(HashMap::new())),
            cost_history: Arc::new(RwLock::new(Vec::new())),
            user_preferences: UserRoutingPreferences::default(),
        }
    }

    /// Create router with custom settings
    pub fn with_config(thresholds: RoutingThresholds, preferences: UserRoutingPreferences) -> Self {
        Self {
            complexity_thresholds: thresholds,
            response_cache: Arc::new(RwLock::new(HashMap::new())),
            cost_history: Arc::new(RwLock::new(Vec::new())),
            user_preferences: preferences,
        }
    }

    /// Analyze query and determine optimal routing
    pub async fn analyze_query(&self, query: &str) -> Result<QueryAnalysis> {
        // Check cache first
        if self.user_preferences.enable_caching {
            if let Some(cached) = self.get_cached_response(query).await? {
                return Ok(QueryAnalysis {
                    complexity: QueryComplexity {
                        score: 0.0, // Cached, so complexity doesn't matter
                        has_code: false,
                        has_architecture: false,
                        has_research: false,
                        word_count: 0,
                        technical_terms: 0,
                    },
                    recommended_destination: QueryDestination::Local, // Cached responses are local
                    confidence: 1.0,
                    reasoning: vec!["Using cached response".to_string()],
                });
            }
        }

        // Analyze query complexity
        let complexity = self.analyze_complexity(query);
        let (destination, confidence, reasoning) = self.determine_destination(&complexity);

        Ok(QueryAnalysis {
            complexity,
            recommended_destination: destination,
            confidence,
            reasoning,
        })
    }

    /// Route query to appropriate destination
    pub async fn route_query(&self, query: &str) -> Result<QueryDestination> {
        let analysis = self.analyze_query(query).await?;

        // Apply user preferences
        let final_destination = match analysis.recommended_destination {
            QueryDestination::Remote if !self.user_preferences.allow_remote => {
                QueryDestination::Local
            }
            QueryDestination::Local if self.user_preferences.prefer_local => {
                QueryDestination::Local
            }
            QueryDestination::AskUser if !self.user_preferences.confirm_complex => {
                // Auto-decide based on complexity
                if analysis.complexity.score > 0.5 {
                    QueryDestination::Remote
                } else {
                    QueryDestination::Local
                }
            }
            other => other,
        };

        Ok(final_destination)
    }

    /// Cache a response
    pub async fn cache_response(
        &self,
        query: &str,
        response: &str,
        destination: &str,
    ) -> Result<()> {
        if !self.user_preferences.enable_caching {
            return Ok(());
        }

        let query_hash = self.hash_query(query);
        let cached = CachedResponse {
            query_hash: query_hash.clone(),
            response: response.to_string(),
            destination: destination.to_string(),
            timestamp: Utc::now(),
            ttl_seconds: self.complexity_thresholds.cache_ttl_seconds,
        };

        let mut cache = self.response_cache.write().await;

        // Enforce cache size limits
        if cache.len() >= self.complexity_thresholds.max_cache_entries {
            // Remove oldest entries (simple strategy)
            let to_remove: Vec<String> = cache
                .iter()
                .filter(|(_, v)| {
                    let age = Utc::now().signed_duration_since(v.timestamp).num_seconds() as u64;
                    age > v.ttl_seconds
                })
                .map(|(k, _)| k.clone())
                .collect();

            for key in to_remove {
                cache.remove(&key);
            }
        }

        cache.insert(query_hash, cached);
        Ok(())
    }

    /// Get cached response if available
    pub async fn get_cached_response(&self, query: &str) -> Result<Option<String>> {
        if !self.user_preferences.enable_caching {
            return Ok(None);
        }

        let query_hash = self.hash_query(query);
        let cache = self.response_cache.read().await;

        if let Some(cached) = cache.get(&query_hash) {
            let age = Utc::now()
                .signed_duration_since(cached.timestamp)
                .num_seconds() as u64;
            if age < cached.ttl_seconds {
                return Ok(Some(cached.response.clone()));
            }
        }

        Ok(None)
    }

    /// Record query cost for analytics
    pub async fn record_cost(
        &self,
        query_id: &str,
        destination: &str,
        processing_time_ms: u64,
        success: bool,
    ) -> Result<()> {
        let cost = QueryCost {
            query_id: query_id.to_string(),
            destination: destination.to_string(),
            processing_time_ms,
            cost_cents: 0, // Always 0 for local/remote web interface
            timestamp: Utc::now(),
            success,
        };

        let mut history = self.cost_history.write().await;
        history.push(cost);

        // Keep only recent history (last 1000 queries)
        if history.len() > 1000 {
            history.remove(0);
        }

        Ok(())
    }

    /// Get routing statistics
    pub async fn get_statistics(&self) -> Result<RoutingStats> {
        let cache = self.response_cache.read().await;
        let history = self.cost_history.read().await;

        let total_queries = history.len();
        let local_queries = history.iter().filter(|h| h.destination == "local").count();
        let remote_queries = history.iter().filter(|h| h.destination == "remote").count();
        let successful_queries = history.iter().filter(|h| h.success).count();

        let avg_processing_time = if !history.is_empty() {
            history.iter().map(|h| h.processing_time_ms).sum::<u64>() / history.len() as u64
        } else {
            0
        };

        Ok(RoutingStats {
            total_queries,
            local_queries,
            remote_queries,
            successful_queries,
            cache_size: cache.len(),
            average_processing_time_ms: avg_processing_time,
        })
    }

    /// Analyze query complexity
    fn analyze_complexity(&self, query: &str) -> QueryComplexity {
        let lower_query = query.to_lowercase();
        let words: Vec<&str> = query.split_whitespace().collect();

        let has_code = lower_query.contains("```")
            || lower_query.contains("fn ")
            || lower_query.contains("struct ")
            || lower_query.contains("impl ")
            || lower_query.contains("use ")
            || lower_query.contains("let ")
            || lower_query.contains("const ");

        let has_architecture = lower_query.contains("architecture")
            || lower_query.contains("design")
            || lower_query.contains("system")
            || lower_query.contains("component")
            || lower_query.contains("module")
            || lower_query.contains("pattern");

        let has_research = lower_query.contains("research")
            || lower_query.contains("latest")
            || lower_query.contains("current")
            || lower_query.contains("trend")
            || lower_query.contains("best practice")
            || lower_query.contains("how to")
            || lower_query.contains("tutorial");

        let technical_terms = [
            "algorithm",
            "asynchronous",
            "authentication",
            "authorization",
            "backend",
            "blockchain",
            "cache",
            "concurrency",
            "container",
            "database",
            "debugging",
            "deployment",
            "distributed",
            "encryption",
            "framework",
            "frontend",
            "inheritance",
            "interface",
            "kubernetes",
            "lambda",
            "machine learning",
            "microservice",
            "middleware",
            "optimization",
            "parallel",
            "polymorphism",
            "protocol",
            "refactoring",
            "repository",
            "scalability",
            "security",
            "serialization",
            "serverless",
            "streaming",
            "synchronization",
            "testing",
            "transaction",
            "virtualization",
            "websocket",
        ]
        .iter()
        .filter(|term| lower_query.contains(*term))
        .count();

        // Calculate complexity score (0.0 to 1.0)
        let mut score = 0.0;

        // Length factor
        score += (words.len() as f32 / 80.0).min(0.25);

        // Code factor
        if has_code {
            score += 0.3;
        }

        // Architecture factor
        if has_architecture {
            score += 0.3;
        }

        // Research factor
        if has_research {
            score += 0.15;
        }

        // Technical terms factor
        score += (technical_terms as f32 / 8.0).min(0.25);

        QueryComplexity {
            score: score.min(1.0),
            has_code,
            has_architecture,
            has_research,
            word_count: words.len(),
            technical_terms,
        }
    }

    /// Determine destination based on complexity
    fn determine_destination(
        &self,
        complexity: &QueryComplexity,
    ) -> (QueryDestination, f32, Vec<String>) {
        let mut reasoning = Vec::new();

        if complexity.score >= self.complexity_thresholds.remote_threshold {
            reasoning.push(format!("High complexity score: {:.2}", complexity.score));
            if complexity.has_architecture {
                reasoning.push("Architecture/design question detected".to_string());
            }
            if complexity.has_research {
                reasoning.push("Research/knowledge question detected".to_string());
            }
            return (QueryDestination::Remote, 0.8, reasoning);
        }

        if complexity.score <= self.complexity_thresholds.local_threshold {
            reasoning.push(format!("Low complexity score: {:.2}", complexity.score));
            reasoning.push("Suitable for local processing".to_string());
            return (QueryDestination::Local, 0.9, reasoning);
        }

        // Borderline case
        reasoning.push(format!(
            "Borderline complexity score: {:.2}",
            complexity.score
        ));
        reasoning.push("Could be handled locally or remotely".to_string());

        if self.user_preferences.confirm_complex {
            (QueryDestination::AskUser, 0.6, reasoning)
        } else {
            // Auto-decide
            if complexity.score > 0.5 {
                reasoning.push("Auto-selected remote due to complexity".to_string());
                (QueryDestination::Remote, 0.7, reasoning)
            } else {
                reasoning.push("Auto-selected local due to simplicity".to_string());
                (QueryDestination::Local, 0.7, reasoning)
            }
        }
    }

    /// Generate query hash for caching
    fn hash_query(&self, query: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        query.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }
}

/// Routing statistics
#[derive(Debug, Clone)]
pub struct RoutingStats {
    pub total_queries: usize,
    pub local_queries: usize,
    pub remote_queries: usize,
    pub successful_queries: usize,
    pub cache_size: usize,
    pub average_processing_time_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_complexity_analysis() {
        let router = SmartRouter::new();

        // Test simple query
        let simple = router
            .analyze_query("How do I print hello world in Rust?")
            .await
            .unwrap();
        assert!(simple.complexity.score < 0.5);
        assert_eq!(simple.recommended_destination, QueryDestination::Local);

        // Test complex query
        let complex = router.analyze_query("Design a microservices architecture for a high-traffic e-commerce platform with real-time inventory management, distributed caching, and event-driven communication patterns.").await.unwrap();
        assert!(complex.complexity.score > 0.7);
        assert_eq!(complex.recommended_destination, QueryDestination::Remote);
    }

    #[tokio::test]
    async fn test_caching() {
        let router = SmartRouter::new();

        let query = "What is the capital of France?";
        let response = "Paris";

        // Cache response
        router
            .cache_response(query, response, "local")
            .await
            .unwrap();

        // Retrieve from cache
        let cached = router.get_cached_response(query).await.unwrap();
        assert_eq!(cached, Some(response.to_string()));
    }

    #[tokio::test]
    async fn test_statistics() {
        let router = SmartRouter::new();

        // Record some costs
        router.record_cost("1", "local", 100, true).await.unwrap();
        router.record_cost("2", "remote", 5000, true).await.unwrap();

        let stats = router.get_statistics().await.unwrap();
        assert_eq!(stats.total_queries, 2);
        assert_eq!(stats.local_queries, 1);
        assert_eq!(stats.remote_queries, 1);
        assert_eq!(stats.average_processing_time_ms, 2550);
    }
}
