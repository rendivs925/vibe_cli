use crate::tools::common::ensure_args_at_least;
use crate::tools::web::{fetch_url, html_to_text};
use domain::tools::{Tool, ToolError, ToolOutput};

pub struct WebFetchTool;

impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "web_fetch"
    }

    fn description(&self) -> &str {
        "Fetch URL content (HTML or text)"
    }

    fn usage(&self) -> &str {
        "web_fetch <url> [max_chars]"
    }

    fn examples(&self) -> Vec<&str> {
        vec!["web_fetch https://example.com", "web_fetch https://example.com 2000"]
    }

    fn requires_confirmation(&self) -> bool {
        false
    }

    fn execute(&self, args: &[&str]) -> Result<ToolOutput, ToolError> {
        ensure_args_at_least(args, 1, self.usage())?;
        let url = args[0];
        let max_chars = args.get(1).and_then(|v| v.parse::<usize>().ok()).unwrap_or(4000);

        let response = fetch_url(url)?;
        let is_html = response
            .content_type
            .as_deref()
            .map(|ct| ct.contains("text/html"))
            .unwrap_or_else(|| response.content.contains("<html"));

        let text = if is_html {
            html_to_text(&response.content, max_chars)
        } else if response.content.len() > max_chars {
            format!("{}...[truncated]", &response.content[..max_chars])
        } else {
            response.content
        };

        let mut out = ToolOutput::success(text);
        if let Some(ct) = response.content_type {
            out.metadata.insert("content_type".to_string(), ct);
        }
        Ok(out)
    }
}
