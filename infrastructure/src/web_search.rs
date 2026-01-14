use crate::network_security::SecureHttpClient;
use scraper::{Html, Selector};
use shared::types::Result;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use url::Url;

/// Secure web search integration using DuckDuckGo with network security
pub struct WebSearch {
    client: SecureHttpClient,
    last_search: Mutex<Instant>,
    min_interval: Duration,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub relevance_score: f32,
}

#[derive(Debug, Clone)]
pub struct SearchOptions {
    pub max_results: usize,
    pub programming_focus: bool,
    pub timeout: Duration,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            max_results: 8,
            programming_focus: true,
            timeout: Duration::from_secs(10),
        }
    }
}

impl WebSearch {
    /// Create new secure web search instance
    pub fn new() -> Result<Self> {
        let client = SecureHttpClient::new()
            .map_err(|e| anyhow::anyhow!("Failed to create secure HTTP client: {}", e))?;

        // Rate limit: 20 searches per minute (minimum 3 seconds between requests)
        let min_interval = Duration::from_secs(3);

        Ok(Self {
            client,
            last_search: Mutex::new(Instant::now() - min_interval), // Allow immediate first request
            min_interval,
        })
    }

    /// Search the web for programming-related information
    pub async fn search_programming(
        &self,
        query: &str,
        options: SearchOptions,
    ) -> Result<Vec<SearchResult>> {
        // Enforce rate limiting
        self.enforce_rate_limit().await?;

        let enhanced_query = if options.programming_focus {
            self.enhance_programming_query(query)
        } else {
            query.to_string()
        };

        self.search_duckduckgo(&enhanced_query, &options).await
    }

    /// Search using DuckDuckGo HTML interface with security checks
    async fn search_duckduckgo(
        &self,
        query: &str,
        options: &SearchOptions,
    ) -> Result<Vec<SearchResult>> {
        let search_url = format!(
            "https://html.duckduckgo.com/html/?q={}&kl=us-en",
            urlencoding::encode(query)
        );

        // Network security check is handled by SecureHttpClient
        let response = self
            .client
            .get(&search_url)
            .await
            .map_err(|e| anyhow::anyhow!("Network security violation: {}", e))?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "DuckDuckGo search failed: {}",
                response.status()
            ));
        }

        let html = response.text().await?;
        self.parse_duckduckgo_results(&html, options.max_results)
    }

    /// Parse DuckDuckGo HTML results
    fn parse_duckduckgo_results(
        &self,
        html: &str,
        max_results: usize,
    ) -> Result<Vec<SearchResult>> {
        let document = Html::parse_document(html);

        // Use simpler selectors that are more likely to work
        let result_selector = Selector::parse(".result")
            .map_err(|e| anyhow::anyhow!("Selector parse error: {:?}", e))?;
        let title_selector = Selector::parse(".result__title a")
            .map_err(|e| anyhow::anyhow!("Selector parse error: {:?}", e))?;
        let url_selector = Selector::parse(".result__url")
            .map_err(|e| anyhow::anyhow!("Selector parse error: {:?}", e))?;
        let snippet_selector = Selector::parse(".result__snippet")
            .map_err(|e| anyhow::anyhow!("Selector parse error: {:?}", e))?;

        let mut results = Vec::new();

        for result_element in document.select(&result_selector).take(max_results) {
            let title = result_element
                .select(&title_selector)
                .next()
                .and_then(|el| el.text().next())
                .unwrap_or("")
                .trim()
                .to_string();

            let url = result_element
                .select(&url_selector)
                .next()
                .and_then(|el| el.text().next())
                .unwrap_or("")
                .trim()
                .to_string();

            let snippet = result_element
                .select(&snippet_selector)
                .next()
                .map(|el| el.text().collect::<Vec<_>>().join(" "))
                .unwrap_or_default()
                .trim()
                .to_string();

            if !title.is_empty() && !url.is_empty() {
                let relevance_score = self.calculate_relevance_score(&title, &snippet);
                results.push(SearchResult {
                    title,
                    url,
                    snippet,
                    relevance_score,
                });
            }
        }

        // Sort by relevance score
        results.sort_by(|a, b| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(results)
    }

    /// Enhance query for programming-specific searches
    pub fn enhance_programming_query(&self, query: &str) -> String {
        let query_lower = query.to_lowercase();

        // Add programming-specific keywords if not present
        let mut enhanced = query.to_string();

        if query_lower.contains("error") || query_lower.contains("exception") {
            enhanced.push_str(" programming solution");
        } else if query_lower.contains("how to") || query_lower.contains("tutorial") {
            enhanced.push_str(" code example");
        } else if query_lower.contains("library") || query_lower.contains("framework") {
            enhanced.push_str(" documentation api");
        } else if !query_lower.contains("programming")
            && !query_lower.contains("code")
            && !query_lower.contains("development")
        {
            enhanced.push_str(" programming");
        }

        // Add language-specific enhancements
        if query_lower.contains("rust") {
            enhanced.push_str(" cargo crate");
        } else if query_lower.contains("python") {
            enhanced.push_str(" pip package");
        } else if query_lower.contains("javascript") || query_lower.contains("js") {
            enhanced.push_str(" npm package");
        } else if query_lower.contains("typescript") || query_lower.contains("ts") {
            enhanced.push_str(" npm package");
        }

        enhanced
    }

    /// Calculate relevance score based on content analysis
    fn calculate_relevance_score(&self, title: &str, snippet: &str) -> f32 {
        let mut score = 0.5; // Base score

        let combined_text = format!("{} {}", title, snippet).to_lowercase();

        // Boost score for programming-related content
        if combined_text.contains("code") || combined_text.contains("function") {
            score += 0.2;
        }
        if combined_text.contains("api") || combined_text.contains("documentation") {
            score += 0.15;
        }
        if combined_text.contains("example") || combined_text.contains("tutorial") {
            score += 0.1;
        }
        if combined_text.contains("github") || combined_text.contains("stackoverflow") {
            score += 0.1;
        }

        // Penalize for non-programming content
        if combined_text.contains("news") || combined_text.contains("article") {
            score -= 0.1;
        }
        if combined_text.contains("advertisement") || combined_text.contains("sponsored") {
            score -= 0.2;
        }

        // Length bonus (prefer more detailed results)
        if snippet.len() > 100 {
            score += 0.1;
        }

        if score > 1.0 {
            1.0
        } else if score < 0.0 {
            0.0
        } else {
            score
        }
    }

    /// Enforce rate limiting
    async fn enforce_rate_limit(&self) -> Result<()> {
        let mut last_search = self.last_search.lock().await;
        let now = Instant::now();
        let time_since_last = now.duration_since(*last_search);

        if time_since_last < self.min_interval {
            let sleep_duration = self.min_interval - time_since_last;
            tokio::time::sleep(sleep_duration).await;
        }

        *last_search = Instant::now();
        Ok(())
    }

    /// Get search statistics
    pub async fn get_stats(&self) -> HashMap<String, String> {
        let mut stats = HashMap::new();

        let last_search = *self.last_search.lock().await;
        let time_since_last = last_search.elapsed();

        stats.insert(
            "last_search_seconds".to_string(),
            time_since_last.as_secs().to_string(),
        );
        stats.insert("rate_limit_per_minute".to_string(), "20".to_string());
        stats.insert("search_provider".to_string(), "DuckDuckGo".to_string());

        stats
    }

    /// Search with automatic retries and fallback
    pub async fn search_with_retry(
        &self,
        query: &str,
        options: SearchOptions,
        max_retries: usize,
    ) -> Result<Vec<SearchResult>> {
        let mut last_error = None;

        for attempt in 0..=max_retries {
            match self.search_programming(query, options.clone()).await {
                Ok(results) => return Ok(results),
                Err(e) => {
                    last_error = Some(e);
                    if attempt < max_retries {
                        // Exponential backoff
                        let delay = Duration::from_millis(500 * (2_u64.pow(attempt as u32)));
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Search failed after all retries")))
    }

    /// Validate search results for quality
    pub fn validate_results(results: &[SearchResult]) -> Vec<SearchResult> {
        results
            .iter()
            .filter(|result| {
                // Basic quality checks
                !result.title.is_empty() &&
                !result.url.is_empty() &&
                result.snippet.len() > 20 &&
                result.relevance_score > 0.3 &&
                !result.url.contains("duckduckgo.com") && // Avoid self-references
                Self::is_valid_url(&result.url)
            })
            .cloned()
            .collect()
    }

    /// Check if URL is valid and accessible
    fn is_valid_url(url_str: &str) -> bool {
        if let Ok(url) = Url::parse(url_str) {
            matches!(url.scheme(), "http" | "https")
        } else {
            false
        }
    }

    /// Format results for display
    pub fn format_results(results: &[SearchResult]) -> String {
        if results.is_empty() {
            return "No relevant results found.".to_string();
        }

        let mut output = format!("Found {} results:\n\n", results.len());

        for (i, result) in results.iter().enumerate() {
            output.push_str(&format!("{}. **{}**\n", i + 1, result.title));
            output.push_str(&format!("   URL: {}\n", result.url));
            output.push_str(&format!("   Relevance: {:.2}\n", result.relevance_score));
            if !result.snippet.is_empty() {
                output.push_str(&format!("   Summary: {}\n", result.snippet));
            }
            output.push_str("\n");
        }

        output
    }

    /// Extract programming-specific insights from results
    pub fn extract_programming_insights(results: &[SearchResult]) -> Vec<String> {
        let mut insights = Vec::new();

        for result in results {
            let combined = format!("{} {}", result.title, result.snippet);

            // Look for common programming patterns
            if combined.contains("error") && combined.contains("solution") {
                insights.push(format!("Error solution: {}", result.title));
            }
            if combined.contains("tutorial") || combined.contains("guide") {
                insights.push(format!("Tutorial: {}", result.title));
            }
            if combined.contains("api") && combined.contains("documentation") {
                insights.push(format!("API docs: {}", result.title));
            }
            if combined.contains("example") && combined.contains("code") {
                insights.push(format!("Code example: {}", result.title));
            }
        }

        insights.truncate(5); // Limit to top 5 insights
        insights
    }
}
