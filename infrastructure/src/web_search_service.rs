use serde::{Deserialize, Serialize};
use std::process::Command;
use tokio::time::{sleep, Duration};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

pub struct WebSearchService {
    searxng_url: String,
}

impl WebSearchService {
    pub fn new(searxng_url: String) -> Self {
        Self { searxng_url }
    }
    
    pub async fn search(&self, query: &str, num_results: usize) -> Result<Vec<SearchResult>, String> {
        match self.searxng_search(query, num_results).await {
            Ok(results) => Ok(results),
            Err(e) if e.contains("Failed to connect") || e.contains("Connection refused") => {
                println!("SearXNG not running. Starting container...");
                self.start_searxng()?;
                tokio::time::sleep(Duration::from_secs(3)).await;
                self.searxng_search(query, num_results).await
            }
            Err(e) => Err(e),
        }
    }

    fn start_searxng(&self) -> Result<(), String> {
        let output = Command::new("docker")
            .args(["ps", "-a", "--filter", "name=vibe-searxng", "--format", "{{.Names}}"])
            .output()
            .map_err(|e| format!("docker ps failed: {}", e))?;

        let container_exists = String::from_utf8_lossy(&output.stdout).contains("vibe-searxng");

        if container_exists {
            Command::new("docker")
                .args(["start", "vibe-searxng"])
                .output()
                .map_err(|e| format!("docker start failed: {}", e))?;
        } else {
            let secret = Command::new("openssl")
                .args(["rand", "-hex", "32"])
                .output()
                .map_err(|_| "openssl not found, using random")?;

            let secret_str = if secret.status.success() {
                String::from_utf8_lossy(&secret.stdout).trim().to_string()
            } else {
                "changeme".to_string()
            };

            Command::new("docker")
                .args([
                    "run", "-d",
                    "--name", "vibe-searxng",
                    "-p", "8080:8080",
                    "-e", "SEARXNG_BASE_URL=http://localhost:8080",
                    "-e", &format!("SEARXNG_SECRET={}", secret_str),
                    "-v", "searxng-data:/etc/searxng",
                    "searxng/searxng:latest",
                ])
                .output()
                .map_err(|e| format!("docker run failed: {}", e))?;
        }
        Ok(())
    }
    
    async fn searxng_search(&self, query: &str, num_results: usize) -> Result<Vec<SearchResult>, String> {
        let encoded_query = url_encode(query);
        
        let url = format!(
            "{}/search?q={}&format=json&engines=general&language=en&safesearch=1&count={}",
            self.searxng_url, encoded_query, num_results
        );
        
        let output = Command::new("curl")
            .args(["-s", "--connect-timeout", "10", "-m", "15", "-H", "Accept: application/json", &url])
            .output()
            .map_err(|e| format!("Search request failed: {}", e))?;
        
        if !output.status.success() {
            return Err(format!("curl failed with status: {}", output.status));
        }
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        #[derive(Deserialize)]
        struct SearxngResponse {
            results: Vec<SearxngResult>,
        }
        
        #[derive(Deserialize)]
        struct SearxngResult {
            title: String,
            url: String,
            content: Option<String>,
        }
        
        let response: SearxngResponse = serde_json::from_str(&stdout)
            .map_err(|e| format!("Parse error: {} - content: {}", e, &stdout[..stdout.len().min(200)]))?;
        
        let results: Vec<SearchResult> = response.results.into_iter().take(num_results).map(|r| SearchResult {
            title: r.title,
            url: r.url,
            snippet: r.content.unwrap_or_default(),
        }).collect();
        
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
