use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

pub struct WebSearchService {}

impl WebSearchService {
    pub fn new() -> Self {
        Self {}
    }
    
    pub async fn search(&self, query: &str, num_results: usize) -> Result<Vec<SearchResult>, String> {
        self.duckduckgo_instant_answer(query, num_results).await
    }
    
    async fn duckduckgo_instant_answer(&self, query: &str, num_results: usize) -> Result<Vec<SearchResult>, String> {
        let encoded_query = url_encode(query);
        
        let url = format!(
            "https://api.duckduckgo.com/?q={}&format=json&no_html=1&skip_disambig=1",
            encoded_query
        );
        
        let output = Command::new("curl")
            .args(["-s", "--connect-timeout", "10", "-m", "15", "-H", "Accept: application/json", &url])
            .output()
            .map_err(|e| format!("Search request failed: {}", e))?;
        
        if !output.status.success() {
            return Err(format!("curl failed with status: {}", output.status));
        }
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        let json: serde_json::Value = serde_json::from_str(&stdout)
            .map_err(|e| format!("Parse error: {} - content: {}", e, &stdout[..stdout.len().min(200)]))?;
        
        let mut results = Vec::new();
        
        if let Some(abstract_text) = json.get("AbstractText").and_then(|v| v.as_str()) {
            if !abstract_text.is_empty() {
                results.push(SearchResult {
                    title: json.get("Heading").and_then(|v| v.as_str()).unwrap_or("Result").to_string(),
                    url: json.get("AbstractURL").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    snippet: abstract_text.to_string(),
                });
            }
        }
        
        if results.is_empty() || results.len() < num_results {
            if let Some(related) = json.get("RelatedTopics").and_then(|v| v.as_array()) {
                for topic in related.iter().take(num_results) {
                    if let Some(first_url) = topic.get("FirstURL").and_then(|v| v.as_str()) {
                        let text = topic.get("Text").and_then(|v| v.as_str()).unwrap_or("");
                        results.push(SearchResult {
                            title: text.chars().take(50).collect(),
                            url: first_url.to_string(),
                            snippet: text.to_string(),
                        });
                    }
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
