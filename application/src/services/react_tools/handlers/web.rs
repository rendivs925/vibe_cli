use anyhow::Result;
use async_trait::async_trait;
use domain::entities::react::{ReactTool, ToolCategory, ToolResult};
use std::sync::Arc;

use crate::services::react_context_retriever::RetrievedContext;
use crate::services::react_tools::ReactToolHandler;

pub struct WebSearchHandler;

#[async_trait]
impl ReactToolHandler for WebSearchHandler {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the web via SearXNG"
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Investigation
    }

    fn requires_output(&self) -> bool {
        false
    }

    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let query = context.goal.replace('"', "\\\"");
        Ok(ToolResult::new(ReactTool::WebSearch)
            .with_commands(vec![format!("web_search \"{}\"", query)])
            .with_next_tool(ReactTool::WebFetch))
    }

    fn get_prompt(&self, _context: &RetrievedContext) -> String {
        "Formulate a web search query to gather authoritative info.".to_string()
    }
}

pub struct WebFetchHandler;

#[async_trait]
impl ReactToolHandler for WebFetchHandler {
    fn name(&self) -> &str {
        "web_fetch"
    }

    fn description(&self) -> &str {
        "Fetch content from a URL"
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Investigation
    }

    fn requires_output(&self) -> bool {
        false
    }

    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let url = find_url(context).unwrap_or_else(|| "https://example.com".to_string());
        Ok(ToolResult::new(ReactTool::WebFetch)
            .with_commands(vec![format!("web_fetch {}", url)])
            .with_next_tool(ReactTool::WebSummarize))
    }

    fn get_prompt(&self, _context: &RetrievedContext) -> String {
        "Select the most relevant URL and fetch its content.".to_string()
    }
}

pub struct WebSummarizeHandler;

#[async_trait]
impl ReactToolHandler for WebSummarizeHandler {
    fn name(&self) -> &str {
        "web_summarize"
    }

    fn description(&self) -> &str {
        "Summarize a web page"
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Analysis
    }

    fn requires_output(&self) -> bool {
        false
    }

    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let url = find_url(context).unwrap_or_else(|| "https://example.com".to_string());
        Ok(ToolResult::new(ReactTool::WebSummarize)
            .with_commands(vec![format!("web_summarize {}", url)])
            .with_next_tool(ReactTool::Summarize))
    }

    fn get_prompt(&self, _context: &RetrievedContext) -> String {
        "Summarize the most relevant web page.".to_string()
    }
}

pub struct WebExtractHandler;

#[async_trait]
impl ReactToolHandler for WebExtractHandler {
    fn name(&self) -> &str {
        "web_extract"
    }

    fn description(&self) -> &str {
        "Extract structured data from a web page"
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Analysis
    }

    fn requires_output(&self) -> bool {
        false
    }

    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let url = find_url(context).unwrap_or_else(|| "https://example.com".to_string());
        Ok(ToolResult::new(ReactTool::WebExtract)
            .with_commands(vec![format!("web_extract {}", url)])
            .with_next_tool(ReactTool::Summarize))
    }

    fn get_prompt(&self, _context: &RetrievedContext) -> String {
        "Extract structured information from the web page.".to_string()
    }
}

pub fn build_web_handlers() -> Vec<(ReactTool, Arc<dyn ReactToolHandler>)> {
    vec![
        (ReactTool::WebSearch, Arc::new(WebSearchHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::WebFetch, Arc::new(WebFetchHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::WebSummarize, Arc::new(WebSummarizeHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::WebExtract, Arc::new(WebExtractHandler) as Arc<dyn ReactToolHandler>),
    ]
}

fn find_url(context: &RetrievedContext) -> Option<String> {
    for haystack in [&context.latest_output, &context.session_history, &context.goal] {
        if let Some(url) = extract_first_url(haystack) {
            return Some(url);
        }
    }
    None
}

fn extract_first_url(text: &str) -> Option<String> {
    for token in text.split_whitespace() {
        if token.starts_with("http://") || token.starts_with("https://") {
            let cleaned = token.trim_matches(|c: char| c == ')' || c == ']' || c == '>' || c == ',' || c == '.');
            return Some(cleaned.to_string());
        }
    }
    None
}
