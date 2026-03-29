use serde::{Deserialize, Serialize};
use std::process::Command;
use tokio::time::Duration;
use crate::tools::web::html_to_text;
use readable_rs::{extract, ExtractOptions};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

pub struct WebSearchService {
    searxng_url: String,
}

const SEARXNG_CONTAINER_NAME: &str = "searxng";
const SEARXNG_HOST_PORT: u16 = 8080;

impl WebSearchService {
    pub fn new(searxng_url: String) -> Self {
        Self { searxng_url }
    }

    pub async fn search(
        &self,
        query: &str,
        num_results: usize,
    ) -> Result<Vec<SearchResult>, String> {
        let search_url = self
            .get_active_searxng_url()
            .unwrap_or_else(|| self.searxng_url.clone());

        match self
            .searxng_search_with_url(&search_url, query, num_results)
            .await
        {
            Ok(results) => Ok(results),
            Err(e) if Self::is_connection_error(&e) => Err(format!(
                "SearXNG is not running. Start it with: make searxng-up\nError: {}",
                e
            )),
            Err(e) => Err(e),
        }
    }

    fn is_connection_error(message: &str) -> bool {
        message.contains("Failed to connect")
            || message.contains("Connection refused")
            || message.contains("Empty reply")
            || message.contains("exit status: 7")
            || message.contains("exit status: 56")
    }

    fn get_active_searxng_url(&self) -> Option<String> {
        let output = Command::new("docker")
            .args([
                "ps",
                "--filter",
                &format!("name={}", SEARXNG_CONTAINER_NAME),
                "--format",
                "{{.Ports}}",
            ])
            .output()
            .ok()?;

        let ports = String::from_utf8_lossy(&output.stdout);

        for line in ports.lines() {
            if line.is_empty() {
                return Some(format!("http://localhost:{}", SEARXNG_HOST_PORT));
            }
            if let Some(host_port) = line.split("->").next() {
                if let Some(port) = host_port.trim().split(':').last() {
                    let port = port.trim_end_matches("/tcp").trim_end_matches("/udp");
                    return Some(format!("http://localhost:{}", port));
                }
            }
        }

        Some(format!("http://localhost:{}", SEARXNG_HOST_PORT))
    }

    async fn searxng_search_with_url(
        &self,
        base_url: &str,
        query: &str,
        num_results: usize,
    ) -> Result<Vec<SearchResult>, String> {
        let encoded_query = url_encode(query);

        let url = format!(
            "{}/search?q={}&format=json&engines=general&language=en&safesearch=1&count={}",
            base_url, encoded_query, num_results
        );

        let output = Command::new("curl")
            .args([
                "-sS",
                "--connect-timeout",
                "10",
                "-m",
                "15",
                "--noproxy",
                "localhost,127.0.0.1,::1",
                "-A",
                "Mozilla/5.0 (compatible; vibe_cli/1.0; +https://github.com)",
                "-H",
                "Accept: application/json",
                "-w",
                "\n__CURL_HTTP_STATUS__:%{http_code}",
                &url,
            ])
            .output()
            .map_err(|e| format!("Search request failed: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let detail = stderr.trim();
            if detail.is_empty() {
                return Err(format!("curl failed with status: {}", output.status));
            }
            return Err(format!(
                "curl failed with status: {} ({})",
                output.status, detail
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let (body, http_status) = match stdout.rsplit_once("__CURL_HTTP_STATUS__:") {
            Some((body, status)) => (body.trim_end(), status.trim()),
            None => (stdout.as_ref(), ""),
        };

        if !http_status.is_empty() && http_status != "200" {
            let snippet: String = body.chars().take(200).collect();
            return Err(format!(
                "SearXNG returned HTTP {}: {}",
                http_status,
                snippet.replace('\n', " ")
            ));
        }

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

        let response: SearxngResponse = serde_json::from_str(body).map_err(|e| {
            format!(
                "Parse error: {} - content: {}",
                e,
                &body[..body.len().min(200)]
            )
        })?;

        let results: Vec<SearchResult> = response
            .results
            .into_iter()
            .take(num_results)
            .map(|r| SearchResult {
                title: r.title,
                url: r.url,
                snippet: r.content.unwrap_or_default(),
            })
            .collect();

        Ok(results)
    }

    pub async fn fetch_page(&self, url: &str) -> Result<String, String> {
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (compatible; vibe_cli/1.0; +https://github.com)")
            .redirect(reqwest::redirect::Policy::limited(10))
            .timeout(Duration::from_secs(20))
            .build()
            .map_err(|e| format!("Fetch failed: {}", e))?;

        let resp = client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("Fetch failed: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!(
                "Fetch failed with status: {}",
                resp.status()
            ));
        }

        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| format!("Fetch failed: {}", e))?;

        extract_response_text(url, content_type.as_deref(), &bytes)
    }
}

fn extract_response_text(
    url: &str,
    content_type: Option<&str>,
    bytes: &[u8],
) -> Result<String, String> {
    let is_pdf = content_type
        .map(|ct| ct.contains("application/pdf"))
        .unwrap_or(false)
        || url.to_lowercase().ends_with(".pdf")
        || bytes.starts_with(b"%PDF");

    if is_pdf {
        return extract_pdf_text(bytes);
    }

    let is_html = content_type
        .map(|ct| ct.contains("text/html"))
        .unwrap_or(false)
        || bytes
            .get(0..512)
            .and_then(|chunk| std::str::from_utf8(chunk).ok())
            .map(|chunk| chunk.contains("<html") || chunk.contains("<body"))
            .unwrap_or(false);

    let text = if is_html {
        let html = String::from_utf8_lossy(bytes).to_string();
        if let Some(extracted) = extract_main_content(&html, url) {
            extracted
        } else {
            html_to_text(&html, 8000)
        }
    } else {
        let raw = String::from_utf8_lossy(bytes).to_string();
        truncate_text(&collapse_whitespace(&raw), 8000)
    };

    Ok(text)
}

fn extract_pdf_text(bytes: &[u8]) -> Result<String, String> {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("Fetch failed: {}", e))?
        .as_millis();

    let mut path = std::env::temp_dir();
    path.push(format!("vibe_cli_fetch_{}_{}.pdf", std::process::id(), now));

    fs::write(&path, bytes).map_err(|e| format!("Fetch failed: {}", e))?;
    let extracted = pdf_extract::extract_text(&path)
        .map_err(|e| format!("Fetch failed: {}", e))?;
    let _ = fs::remove_file(&path);

    Ok(truncate_text(&collapse_whitespace(&extracted), 8000))
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn extract_main_content(html: &str, url: &str) -> Option<String> {
    let product = extract(html, url, ExtractOptions::default());
    let content = product.content?;
    let article_html = content.to_string();
    let text = html_to_text(&article_html, 8000);
    if text.trim().len() < 200 {
        return None;
    }
    Some(text)
}

fn truncate_text(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        return text.to_string();
    }
    let mut out = String::new();
    let mut count = 0;
    for ch in text.chars() {
        if count >= max_len {
            break;
        }
        out.push(ch);
        count += 1;
    }
    out.push_str("...[truncated]");
    out
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
