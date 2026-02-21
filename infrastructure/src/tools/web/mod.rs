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
    use scraper::{Html, Selector};

    let document = Html::parse_document(html);
    let main_selector = Selector::parse("article, main")
        .unwrap_or_else(|_| Selector::parse("article").unwrap());
    let body_selector = Selector::parse("body").unwrap_or_else(|_| Selector::parse("html").unwrap());

    let mut nodes: Vec<scraper::ElementRef> = document.select(&main_selector).collect();
    if nodes.is_empty() {
        nodes = document.select(&body_selector).collect();
    }

    let mut text = String::new();
    for node in nodes {
        collect_text(node, &mut text, false);
    }

    let cleaned = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.len() > max_len {
        format!("{}...[truncated]", &cleaned[..max_len])
    } else {
        cleaned
    }
}

fn collect_text(node: scraper::ElementRef, out: &mut String, skip: bool) {
    use scraper::node::Node;

    let mut stack = Vec::new();
    stack.push((node, skip));

    while let Some((current, skip_parent)) = stack.pop() {
        let name = current.value().name();
        let skip_here = skip_parent || should_skip_tag(name);

        for child in current.children() {
            match child.value() {
                Node::Text(t) => {
                    if !skip_here {
                        let trimmed = t.trim();
                        if !trimmed.is_empty() {
                            out.push_str(trimmed);
                            out.push(' ');
                        }
                    }
                }
                Node::Element(_) => {
                    if let Some(elem) = scraper::ElementRef::wrap(child) {
                        stack.push((elem, skip_here));
                    }
                }
                _ => {}
            }
        }
    }
}

fn should_skip_tag(name: &str) -> bool {
    matches!(
        name,
        "script"
            | "style"
            | "noscript"
            | "svg"
            | "canvas"
            | "iframe"
            | "nav"
            | "header"
            | "footer"
            | "aside"
            | "form"
            | "button"
            | "input"
            | "textarea"
            | "select"
    )
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
