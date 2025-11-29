use crate::config::Config;
use crate::session::Message;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [Message],
    stream: bool,
}

#[derive(Deserialize, Debug)]
struct ChatResponse {
    message: Message,
}

/// Extract clean JSON from noisy model output
fn extract_last_json(raw: &str) -> Option<&str> {
    let trimmed = raw.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        return Some(trimmed);
    }
    let bytes = trimmed.as_bytes();
    let mut depth = 0;
    let mut start = None;
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'{' {
            if depth == 0 {
                start = Some(i);
            }
            depth += 1;
        } else if b == b'}' {
            depth -= 1;
            if depth == 0 {
                if let Some(s) = start {
                    return Some(&trimmed[s..=i]);
                }
            }
        }
    }
    None
}

/// Clean command output by removing markdown code blocks
fn clean_command_output(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.starts_with("```") && trimmed.ends_with("```") {
        // Remove the first and last lines if they are ``` or ```sh
        let lines: Vec<&str> = trimmed.lines().collect();
        if lines.len() >= 3 {
            if lines[0].trim().starts_with("```") && lines.last().unwrap().trim() == "```" {
                return lines[1..lines.len()-1].join("\n").trim().to_string();
            }
        }
    }
    trimmed.to_string()
}

/// Clean JSON content by removing comments and invalid parts
fn clean_json_content(content: &str) -> String {
    let mut result = String::new();
    let mut in_string = false;
    let mut escape_next = false;
    let mut comment_start = false;

    for (i, ch) in content.chars().enumerate() {
        if escape_next {
            result.push(ch);
            escape_next = false;
            continue;
        }

        match ch {
            '"' => {
                if !comment_start {
                    in_string = !in_string;
                    result.push(ch);
                }
            }
            '\\' => {
                if in_string {
                    escape_next = true;
                }
                result.push(ch);
            }
            '/' => {
                if !in_string && i + 1 < content.len() && content.chars().nth(i + 1) == Some('/') {
                    // Start of comment
                    comment_start = true;
                    // Skip until end of line
                    continue;
                } else if !comment_start {
                    result.push(ch);
                }
            }
            '\n' | '\r' => {
                if comment_start {
                    comment_start = false;
                } else {
                    result.push(ch);
                }
            }
            _ => {
                if !comment_start {
                    result.push(ch);
                }
            }
        }
    }

    result.trim().to_string()
}

/// Request a SINGLE command from Ollama
pub async fn request_command(config: &Config, messages: &[Message]) -> Result<String> {
    let client = reqwest::Client::new();

    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "/home/user".to_string());

    let mut adjusted = messages.to_vec();
    adjusted.push(Message {
        role: "user".into(),
        content: format!(
            "Convert the user's last request into ONE POSIX shell command. \
             Current working directory: {}. \
             Use actual paths and commands that will work in this environment. \
             Avoid placeholders like '/path/to/' - use real paths or relative paths. \
             Common patterns: 'disk space/free space' → df -h, 'folder sizes/largest folders' → du -sh */ | sort -hr. \
             Distinguish between filesystem space (df) and folder sizes (du). \
             Cache management: 'clear cache' uses --retrain flag, 'show cache' → cat ~/.config/qwen_cli_assistant/cache.json. \
             Output ONLY the command, no markdown, no explanation.",
            cwd
        ),
    });

    let req = ChatRequest {
        model: &config.model,
        messages: &adjusted,
        stream: false,
    };

    let resp = client
        .post(&config.endpoint)
        .json(&req)
        .send()
        .await
        .context("Failed contacting Ollama")?;

    let raw = resp.text().await?;

    // Handle streaming response (NDJSON)
    let lines: Vec<&str> = raw.lines().collect();
    for line in lines.into_iter().rev() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<ChatResponse>(line) {
            if v.message.role == "assistant" {
                return Ok(clean_command_output(&v.message.content));
            }
        }
    }

    // JSON parse first (non-streaming)
    if let Ok(v) = serde_json::from_str::<ChatResponse>(&raw) {
        return Ok(clean_command_output(&v.message.content));
    }

    // Try to extract JSON inside noisy output
    if let Some(json) = extract_last_json(&raw) {
        if let Ok(v) = serde_json::from_str::<ChatResponse>(json) {
            return Ok(clean_command_output(&v.message.content));
        }
    }

    // Fallback: use raw text
    Ok(clean_command_output(&raw))
}

/// Request multi-step agent plan: returns Vec<String>
pub async fn request_agent_plan(config: &Config, user_prompt: &str) -> Result<Vec<String>> {
    let client = reqwest::Client::new();

    let system = r#"Return a JSON array of POSIX shell commands only.
No text outside JSON.
No markdown.
Avoid destructive commands.
 "#;

    let msgs = vec![
        Message {
            role: "system".into(),
            content: system.into(),
        },
        Message {
            role: "user".into(),
            content: user_prompt.into(),
        },
    ];

    let req = ChatRequest {
        model: &config.model,
        messages: &msgs,
        stream: false,
    };

    let raw = client
        .post(&config.endpoint)
        .json(&req)
        .send()
        .await?
        .text()
        .await?;

    // Handle streaming response (NDJSON)
    let lines: Vec<&str> = raw.lines().collect();
    for line in lines.into_iter().rev() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<ChatResponse>(line) {
            if v.message.role == "assistant" {
                let content = clean_command_output(&v.message.content);
                // Try parsing the content as JSON array
                if let Ok(commands) = serde_json::from_str::<Vec<String>>(&content) {
                    return Ok(commands);
                }
                // Try extracting JSON from markdown
                if let Some(json) = extract_last_json(&content) {
                    if let Ok(commands) = serde_json::from_str::<Vec<String>>(json) {
                        return Ok(commands);
                    }
                }
            }
        }
    }

    // JSON parse first (non-streaming)
    if let Ok(v) = serde_json::from_str::<ChatResponse>(&raw) {
        let content = clean_command_output(&v.message.content);

        // Try parsing the content as JSON array
        if let Ok(commands) = serde_json::from_str::<Vec<String>>(&content) {
            return Ok(commands);
        } else {
            // Try to clean the JSON by removing comments and invalid parts
            let cleaned_json = clean_json_content(&content);
            if let Ok(commands) = serde_json::from_str::<Vec<String>>(&cleaned_json) {
                return Ok(commands);
            }
        }

        // Try extracting JSON from markdown
        if let Some(json) = extract_last_json(&content) {
            if let Ok(commands) = serde_json::from_str::<Vec<String>>(json) {
                return Ok(commands);
            }
        }
    }

    // Try to extract JSON inside noisy output
    if let Some(json) = extract_last_json(&raw) {
        if let Ok(v) = serde_json::from_str::<ChatResponse>(json) {
            let content = clean_command_output(&v.message.content);
            if let Ok(commands) = serde_json::from_str::<Vec<String>>(&content) {
                return Ok(commands);
            }
            // Try extracting JSON from markdown in content
            if let Some(inner_json) = extract_last_json(&content) {
                if let Ok(commands) = serde_json::from_str::<Vec<String>>(inner_json) {
                    return Ok(commands);
                }
            }
        }
    }

    // Fallback — split lines
    Ok(raw
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.trim().to_string())
        .collect())
}

/// Request a bash script (one string output)
pub async fn request_script(config: &Config, user_prompt: &str) -> Result<String> {
    let client = reqwest::Client::new();

    let system = r#"Generate a POSIX-compatible bash script only.
Return only the script text, no markdown."#;

    let msgs = vec![
        Message {
            role: "system".into(),
            content: system.into(),
        },
        Message {
            role: "user".into(),
            content: user_prompt.into(),
        },
    ];

    let req = ChatRequest {
        model: &config.model,
        messages: &msgs,
        stream: false,
    };

    let raw = client
        .post(&config.endpoint)
        .json(&req)
        .send()
        .await?
        .text()
        .await?;

    Ok(raw.trim().into())
}
