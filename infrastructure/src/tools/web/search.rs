use crate::tools::common::ensure_args_at_least;
use crate::tools::web::fetch_url;
use domain::tools::{Tool, ToolError, ToolOutput, OutputFormat};
use serde::Deserialize;

#[derive(Deserialize)]
struct SearxResponse {
    results: Vec<SearxResult>,
}

#[derive(Deserialize)]
struct SearxResult {
    title: Option<String>,
    url: Option<String>,
    content: Option<String>,
}

pub struct WebSearchTool;

impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the web via SearXNG"
    }

    fn usage(&self) -> &str {
        "web_search <query> [limit]"
    }

    fn examples(&self) -> Vec<&str> {
        vec!["web_search \"rust async http client\"", "web_search \"nginx reload\" 5"]
    }

    fn requires_confirmation(&self) -> bool {
        false
    }

    fn execute(&self, args: &[&str]) -> Result<ToolOutput, ToolError> {
        ensure_args_at_least(args, 1, self.usage())?;
        let (query, limit) = parse_query_limit(args);
        let base = std::env::var("SEARXNG_URL")
            .map_err(|_| ToolError::InvalidArguments("SEARXNG_URL not set".to_string()))?;

        let url = build_searx_url(&base, &query)?;
        let response = fetch_url(&url)?;
        let parsed: SearxResponse = serde_json::from_str(&response.content)
            .map_err(|e| ToolError::ExecutionFailed(format!("invalid JSON response: {e}")))?;

        let mut lines = Vec::new();
        for (idx, item) in parsed.results.iter().take(limit).enumerate() {
            let title = item.title.as_deref().unwrap_or("(untitled)");
            let url = item.url.as_deref().unwrap_or("(no url)");
            let snippet = item.content.as_deref().unwrap_or("");
            lines.push(format!("{}. {}\n   {}\n   {}", idx + 1, title, url, snippet.trim()));
        }

        if lines.is_empty() {
            return Ok(ToolOutput::success("No results returned.".to_string()));
        }

        let mut out = ToolOutput::success(lines.join("\n"));
        out.format = OutputFormat::Text;
        Ok(out)
    }
}

fn parse_query_limit(args: &[&str]) -> (String, usize) {
    let mut limit = 5_usize;
    let mut parts = args.to_vec();
    if let Some(last) = args.last().and_then(|v| v.parse::<usize>().ok()) {
        limit = last.max(1).min(10);
        parts.pop();
    }
    (parts.join(" "), limit)
}

fn build_searx_url(base: &str, query: &str) -> Result<String, ToolError> {
    let base = base.trim_end_matches('/');
    let url = format!("{}/search", base);
    let mut url = reqwest::Url::parse(&url)
        .map_err(|e| ToolError::ExecutionFailed(format!("invalid SEARXNG_URL: {e}")))?;
    url.query_pairs_mut()
        .append_pair("q", query)
        .append_pair("format", "json");
    Ok(url.to_string())
}
