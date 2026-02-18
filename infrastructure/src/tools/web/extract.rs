use crate::tools::common::ensure_args_at_least;
use crate::tools::web::fetch_url;
use domain::tools::{Tool, ToolError, ToolOutput};
use scraper::{Html, Selector};

pub struct WebExtractTool;

impl Tool for WebExtractTool {
    fn name(&self) -> &str {
        "web_extract"
    }

    fn description(&self) -> &str {
        "Extract structured data from a web page"
    }

    fn usage(&self) -> &str {
        "web_extract <url>"
    }

    fn examples(&self) -> Vec<&str> {
        vec!["web_extract https://example.com"]
    }

    fn requires_confirmation(&self) -> bool {
        false
    }

    fn execute(&self, args: &[&str]) -> Result<ToolOutput, ToolError> {
        ensure_args_at_least(args, 1, self.usage())?;
        let url = args[0];
        let response = fetch_url(url)?;
        let html = response.content;

        let document = Html::parse_document(&html);
        let title_selector = Selector::parse("title").unwrap();
        let h1_selector = Selector::parse("h1").unwrap();
        let h2_selector = Selector::parse("h2").unwrap();
        let link_selector = Selector::parse("a").unwrap();

        let title = document
            .select(&title_selector)
            .next()
            .map(|t| t.text().collect::<Vec<_>>().join(" "))
            .unwrap_or_else(|| "(no title)".to_string());

        let h1s: Vec<String> = document
            .select(&h1_selector)
            .take(5)
            .map(|h| h.text().collect::<Vec<_>>().join(" "))
            .collect();

        let h2s: Vec<String> = document
            .select(&h2_selector)
            .take(5)
            .map(|h| h.text().collect::<Vec<_>>().join(" "))
            .collect();

        let links: Vec<String> = document
            .select(&link_selector)
            .filter_map(|a| a.value().attr("href"))
            .take(8)
            .map(|href| href.to_string())
            .collect();

        let mut output = String::new();
        output.push_str(&format!("Title: {}\n", title.trim()));
        if !h1s.is_empty() {
            output.push_str("H1:\n");
            for h in h1s {
                output.push_str(&format!("- {}\n", h.trim()));
            }
        }
        if !h2s.is_empty() {
            output.push_str("H2:\n");
            for h in h2s {
                output.push_str(&format!("- {}\n", h.trim()));
            }
        }
        if !links.is_empty() {
            output.push_str("Links:\n");
            for l in links {
                output.push_str(&format!("- {}\n", l));
            }
        }

        Ok(ToolOutput::success(output.trim().to_string()))
    }
}
