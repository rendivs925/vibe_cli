use crate::config::Config;
use crate::session::Message;
use anyhow::{Context, Result};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_util::io::StreamReader;

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [Message],
    stream: bool,
}

#[derive(Deserialize, Debug)]
struct ChatResponse {
    message: Message,
    // If your Ollama returns `done`, keep it; otherwise remove.
    #[serde(default)]
    done: bool,
}

/// Stream NDJSON from Ollama, print assistant text as it arrives,
/// and return the final accumulated assistant content (raw).
async fn stream_assistant_content(
    client: &reqwest::Client,
    config: &Config,
    messages: &[Message],
) -> Result<String> {
    let req = ChatRequest {
        model: &config.model,
        messages,
        stream: true,
    };

    let resp = client
        .post(&config.endpoint)
        .json(&req)
        .send()
        .await
        .context("Failed contacting Ollama")?;

    // Convert the HTTP byte stream into an AsyncRead, then read line-by-line (NDJSON).
    let byte_stream = resp
        .bytes_stream()
        .map(|r| r.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)));
    let reader = StreamReader::new(byte_stream);
    let mut lines = BufReader::new(reader).lines();

    let mut full = String::new();

    while let Some(line) = lines.next_line().await? {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Each line should be a JSON object.
        if let Ok(v) = serde_json::from_str::<ChatResponse>(line) {
            if v.message.role == "assistant" && !v.message.content.is_empty() {
                // Print incrementally for “real-time” feel.
                print!("{}", v.message.content);
                io::stdout().flush().ok();

                full.push_str(&v.message.content);
            }
            if v.done {
                break;
            }
        }
    }

    // Ensure the terminal ends cleanly.
    if !full.ends_with('\n') {
        println!();
    }

    Ok(full)
}

/// Clean model output by removing markdown code fences.
fn clean_command_output(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.starts_with("```") && trimmed.ends_with("```") {
        let lines: Vec<&str> = trimmed.lines().collect();
        if lines.len() >= 3
            && lines[0].trim().starts_with("```")
            && lines.last().unwrap().trim() == "```"
        {
            return lines[1..lines.len() - 1].join("\n").trim().to_string();
        }
    }
    trimmed.to_string()
}

/// Ask user for confirmation (y/yes to proceed).
fn confirm(prompt: &str) -> Result<bool> {
    print!("{prompt} [y/N]: ");
    io::stdout().flush().ok();

    let mut s = String::new();
    io::stdin().read_line(&mut s)?;
    let ans = s.trim().to_ascii_lowercase();
    Ok(ans == "y" || ans == "yes")
}

/// Example: request a single command, stream raw assistant response,
/// then extract final command + ask confirmation.
pub async fn request_command_stream_then_confirm(
    config: &Config,
    messages: &[Message],
) -> Result<Option<String>> {
    let client = reqwest::Client::new();

    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "/home/user".to_string());
    let project_root = find_project_root().unwrap_or_else(|| cwd.clone());
    let platform = if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "unknown"
    };

    let instruction = format!(
        r#"You are a command generator. Convert the user's LAST request into EXACTLY ONE POSIX shell command.

Environment:
- platform: {platform}
- cwd: {cwd}
- project_root: {project_root}

Hard constraints:
1) Output ONLY the command text. No markdown, no explanation, no surrounding quotes.
2) Exactly one command line. (Pipes/&& allowed only if truly required.)
3) Non-destructive by default. Never delete/overwrite unless explicitly asked.
4) Do NOT assume files/tools/flags exist. If unsure, output a minimal discovery command instead of guessing.
5) Avoid placeholders like /path/to. Use real paths (absolute or relative to project_root).
6) No network access (curl/wget/git clone/package install) unless explicitly asked.
7) Disk questions: df for filesystem usage, du for directory sizes."#
    );

    let mut msgs = messages.to_vec();
    msgs.push(Message {
        role: "user".into(),
        content: instruction,
    });

    println!("--- Model (streaming) ---");
    let raw = stream_assistant_content(&client, config, &msgs).await?;

    let cmd = clean_command_output(&raw);

    println!("\n--- Proposed command ---\n{cmd}\n");

    if confirm("Run this command?")? {
        Ok(Some(cmd))
    } else {
        Ok(None)
    }
}

fn find_project_root() -> Option<String> {
    let mut current = std::env::current_dir().ok()?;
    let markers = [
        "Cargo.toml",
        "package.json",
        "requirements.txt",
        "Pipfile",
        "pyproject.toml",
        "setup.py",
        "Makefile",
        "CMakeLists.txt",
        "configure.ac",
        "go.mod",
        "Gemfile",
        "composer.json",
        ".git",
    ];

    loop {
        if markers.iter().any(|m| current.join(m).exists()) {
            return Some(current.display().to_string());
        }
        if !current.pop() {
            break;
        }
    }
    None
}
