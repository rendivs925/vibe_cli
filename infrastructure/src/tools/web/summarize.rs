use crate::tools::common::ensure_args_at_least;
use crate::tools::web::{fetch_url, html_to_text, summarize_text};
use domain::tools::{Tool, ToolError, ToolOutput};

pub struct WebSummarizeTool;

impl Tool for WebSummarizeTool {
    fn name(&self) -> &str {
        "web_summarize"
    }

    fn description(&self) -> &str {
        "Summarize a web page"
    }

    fn usage(&self) -> &str {
        "web_summarize <url>"
    }

    fn examples(&self) -> Vec<&str> {
        vec!["web_summarize https://example.com"]
    }

    fn requires_confirmation(&self) -> bool {
        false
    }

    fn execute(&self, args: &[&str]) -> Result<ToolOutput, ToolError> {
        ensure_args_at_least(args, 1, self.usage())?;
        let url = args[0];

        let response = fetch_url(url)?;
        let text = if response
            .content_type
            .as_deref()
            .map(|ct| ct.contains("text/html"))
            .unwrap_or_else(|| response.content.contains("<html"))
        {
            html_to_text(&response.content, 8000)
        } else {
            response.content
        };

        let summary = summarize_text(&text, 5);
        Ok(ToolOutput::success(summary))
    }
}
