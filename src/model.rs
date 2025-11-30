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

/// Extract JSON array from text that may contain other content
fn extract_json_array(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    let mut depth = 0;
    let mut start = None;
    let mut in_string = false;
    let mut escape_next = false;

    for (i, &b) in bytes.iter().enumerate() {
        if escape_next {
            escape_next = false;
            continue;
        }

        match b {
            b'"' => {
                if !in_string {
                    in_string = true;
                } else {
                    in_string = false;
                }
            }
            b'\\' => {
                if in_string {
                    escape_next = true;
                }
            }
            b'[' => {
                if !in_string && depth == 0 {
                    start = Some(i);
                }
                if !in_string {
                    depth += 1;
                }
            }
            b']' => {
                if !in_string {
                    depth -= 1;
                    if depth == 0 {
                        if let Some(s) = start {
                            return Some(&text[s..=i]);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    None
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

    let system = r#"You are an AI assistant that generates plans as JSON arrays of POSIX shell commands.

IMPORTANT: Your response must be ONLY a valid JSON array of strings. Each string is one POSIX shell command.
Do not include any text before or after the JSON array.
Do not include markdown code blocks.
Do not include explanations or comments.
Do not generate code in other languages.
Avoid destructive commands that modify the system.

Example response format:
["df -h", "free -h", "top -b -n1 | head -20"]

Generate the plan based on the user's request.
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

    // First try: parse the entire raw response directly as JSON array (in case model returns just the array)
    if let Ok(commands) = serde_json::from_str::<Vec<String>>(&raw) {
        return Ok(commands);
    }

    // Second try: clean the raw response and parse as JSON array
    let cleaned_raw = clean_command_output(&raw);
    if let Ok(commands) = serde_json::from_str::<Vec<String>>(&cleaned_raw) {
        return Ok(commands);
    }

    // Handle streaming response (NDJSON) - try each line
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
                // Try to clean the JSON by removing comments and invalid parts
                let cleaned_json = clean_json_content(&content);
                if let Ok(commands) = serde_json::from_str::<Vec<String>>(&cleaned_json) {
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

    // JSON parse first (non-streaming) - try the entire raw response
    if let Ok(v) = serde_json::from_str::<ChatResponse>(&raw) {
        let content = clean_command_output(&v.message.content);

        // Try parsing the content as JSON array
        if let Ok(commands) = serde_json::from_str::<Vec<String>>(&content) {
            return Ok(commands);
        }
        // Try to clean the JSON by removing comments and invalid parts
        let cleaned_json = clean_json_content(&content);
        if let Ok(commands) = serde_json::from_str::<Vec<String>>(&cleaned_json) {
            return Ok(commands);
        }
        // Try extracting JSON from markdown
        if let Some(json) = extract_last_json(&content) {
            if let Ok(commands) = serde_json::from_str::<Vec<String>>(json) {
                return Ok(commands);
            }
        }
    }

    // Try to extract JSON arrays directly from the raw response (in case model returns just the array)
    if let Some(json_array) = extract_json_array(&raw) {
        if let Ok(commands) = serde_json::from_str::<Vec<String>>(json_array) {
            return Ok(commands);
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
        // Also try parsing the extracted JSON directly as an array
        if let Ok(commands) = serde_json::from_str::<Vec<String>>(json) {
            return Ok(commands);
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
