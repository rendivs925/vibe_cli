use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

pub struct WebSearchService {
    user_agent: String,
}

impl WebSearchService {
    pub fn new() -> Self {
        Self {
            user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".to_string(),
        }
    }
    
    pub async fn search(&self, query: &str, num_results: usize) -> Result<Vec<SearchResult>, String> {
        self.duckduckgo_api_search(query, num_results).await
    }
    
    async fn duckduckgo_api_search(&self, query: &str, num_results: usize) -> Result<Vec<SearchResult>, String> {
        let encoded_query = url_encode(query);
        
        let url = format!(
            "https://api.duckduckgo.com/?q={}&format=json&no_html=1&skip_disambig=1",
            encoded_query
        );
        
        let output = Command::new("curl")
            .args(["-s", "--connect-timeout", "10", "-m", "15", &url])
            .output()
            .map_err(|e| format!("Search request failed: {}", e))?;
        
        if !output.status.success() {
            return Err(format!("curl failed with status: {}", output.status));
        }
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        if stdout.trim().is_empty() {
            return Err("Empty response from search API".to_string());
        }
        
        let json: serde_json::Value = serde_json::from_str(&stdout)
            .map_err(|e| format!("Failed to parse response: {} - content: {}", e, &stdout[..stdout.len().min(200)]))?;
        
        let mut results = Vec::new();
        
        if let Some(abstract_text) = json["AbstractText"].as_str() {
            if !abstract_text.is_empty() {
                results.push(SearchResult {
                    title: json["Heading"].as_str().unwrap_or("Result").to_string(),
                    url: json["AbstractURL"].as_str().unwrap_or("").to_string(),
                    snippet: abstract_text.to_string(),
                });
            }
        }
        
        if let Some(related) = json["RelatedTopics"].as_array() {
            for topic in related.iter().take(num_results) {
                if let Some(first_url) = topic["FirstURL"].as_str() {
                    let text = topic["Text"].as_str().unwrap_or("");
                    results.push(SearchResult {
                        title: text.chars().take(50).collect(),
                        url: first_url.to_string(),
                        snippet: text.to_string(),
                    });
                }
            }
        }
        
        Ok(results)
    }
    
    pub async fn fetch_page(&self, url: &str) -> Result<String, String> {
        let output = Command::new("curl")
            .args(["-s", "--connect-timeout", "10", "-m", "15", "-L", url])
            .output()
            .map_err(|e| format!("Fetch failed: {}", e))?;
        
        let text = String::from_utf8_lossy(&output.stdout).to_string();
        
        Ok(text)
    }
}

fn url_encode(s: &str) -> String {
    let mut encoded = String::new();
    for c in s.chars() {
        match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' => encoded.push(c),
            ' ' => encoded.push_str("%20"),
            _ => {
                for byte in c.to_string().as_bytes() {
                    encoded.push_str(&format!("%{:02X}", byte));
                }
            }
        }
    }
    encoded
}
