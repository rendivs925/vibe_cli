use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

pub struct WebSearchService {
    api_key: Option<String>,
    engine: String,
}

impl WebSearchService {
    pub fn new() -> Self {
        Self {
            api_key: std::env::var("SEARCH_API_KEY").ok(),
            engine: "google".to_string(),
        }
    }
    
    pub async fn search(&self, query: &str, num_results: usize) -> Result<Vec<String>, String> {
        let query = urlencoding::encode(query);
        
        let search_url = if let Some(ref key) = self.api_key {
            format!(
                "https://www.googleapis.com/customsearch/v1?key={}&cx={}&q={}&num={}",
                key, self.engine, query, num_results
            )
        } else {
            return self.fallback_search(query, num_results).await;
        };
        
        let response = reqwest::get(&search_url)
            .await
            .map_err(|e| e.to_string())?;
        
        let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
        
        let urls: Vec<String> = json["items"]
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .filter_map(|item| item["link"].as_str().map(|s| s.to_string()))
            .take(num_results)
            .collect();
        
        Ok(urls)
    }
    
    async fn fallback_search(&self, query: String, num_results: usize) -> Result<Vec<String>, String> {
        let search_url = format!(
            "https://duckduckgo.com/html/?q={}&n={}",
            query, num_results
        );
        
        let response = reqwest::get(&search_url)
            .await
            .map_err(|e| e.to_string())?;
        
        let html = response.text().await.map_err(|e| e.to_string())?;
        
        let urls: Vec<String> = html
            .split("href=\"")
            .skip(1)
            .filter_map(|part| {
                part.split('"').next()
            })
            .filter(|url| url.starts_with("http") && !url.contains("duckduckgo.com"))
            .take(num_results)
            .map(|s| s.to_string())
            .collect();
        
        Ok(urls)
    }
}
