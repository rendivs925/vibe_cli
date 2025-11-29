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
                return Ok(v.message.content.trim().into());
            }
        }
    }

    // JSON parse first (non-streaming)
    if let Ok(v) = serde_json::from_str::<ChatResponse>(&raw) {
        return Ok(v.message.content.trim().into());
    }

    // Try to extract JSON inside noisy output
    if let Some(json) = extract_last_json(&raw) {
        if let Ok(v) = serde_json::from_str::<ChatResponse>(json) {
            return Ok(v.message.content.trim().into());
        }
    }

    // Fallback: use raw text
    Ok(raw.trim().to_string())
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

    let resp = client
        .post(&config.endpoint)
        .json(&req)
        .send()
        .await?
        .text()
        .await?;

    // Try parsing pure JSON array
    if let Ok(v) = serde_json::from_str::<Vec<String>>(&resp) {
        return Ok(v);
    }

    // Try extract JSON array from noisy output
    if let Some(json) = extract_last_json(&resp) {
        if let Ok(v) = serde_json::from_str::<Vec<String>>(json) {
            return Ok(v);
        }
    }

    // Fallback — split lines
    Ok(resp
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
