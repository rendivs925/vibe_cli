pub mod search;
pub mod fetch;
pub mod summarize;
pub mod extract;

use domain::tools::ToolError;

pub(crate) struct FetchResponse {
    pub content: String,
    pub content_type: Option<String>,
}

pub(crate) fn fetch_url(url: &str) -> Result<FetchResponse, ToolError> {
    let url = url.trim();
    if url.is_empty() {
        return Err(ToolError::InvalidArguments("missing url".to_string()));
    }

    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

    let response = rt
        .block_on(async {
            let client = reqwest::Client::new();
            let resp = client
                .get(url)
                .header("User-Agent", "vibe-cli/1.0")
                .send()
                .await
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
            let content_type = resp
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            let text = resp
                .text()
                .await
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
            Ok::<_, ToolError>(FetchResponse { content: text, content_type })
        })?;

    Ok(response)
}

pub(crate) fn html_to_text(html: &str, max_len: usize) -> String {
    let document = scraper::Html::parse_document(html);
    let selector = scraper::Selector::parse("body").unwrap_or_else(|_| scraper::Selector::parse("html").unwrap());
    let mut text = String::new();

    for node in document.select(&selector) {
        for line in node.text() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                text.push_str(trimmed);
                text.push(' ');
            }
        }
    }

    let cleaned = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.len() > max_len {
        format!("{}...[truncated]", &cleaned[..max_len])
    } else {
        cleaned
    }
}

pub(crate) fn summarize_text(text: &str, max_sentences: usize) -> String {
    let mut sentences = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        current.push(ch);
        if matches!(ch, '.' | '!' | '?') {
            if current.trim().len() > 20 {
                sentences.push(current.trim().to_string());
            }
            current.clear();
        }
        if sentences.len() >= max_sentences {
            break;
        }
    }

    if sentences.is_empty() {
        text.lines().take(max_sentences).collect::<Vec<_>>().join(" ")
    } else {
        sentences.join(" ")
    }
}
