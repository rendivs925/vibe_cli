use shared::types::Message;

use crate::cli::cache::CacheManager;
use crate::cli::cache::CommandCandidate;
use crate::cli::command_extraction::extract_commands;
use crate::cli::utils::*;
use anyhow::Context;
use futures_util::StreamExt;
use infrastructure::config::Config;
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use tokio::io::{AsyncBufReadExt, BufReader as AsyncBufReader};
use tokio_util::io::StreamReader;

#[derive(Serialize)]
pub struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [Message],
    stream: bool,
}

#[derive(Deserialize, Debug)]
pub struct ChatResponse {
    message: Message,
    #[serde(default)]
    done: bool,
}

fn confirm_and_run_command(command: &str) -> anyhow::Result<Option<String>> {
    print!("Run this command? [y/N]: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim().to_lowercase();

    if input == "y" || input == "yes" {
        Ok(Some(command.to_string()))
    } else {
        println!("Command cancelled.");
        Ok(None)
    }
}

fn handle_cached_candidates(
    candidates: Vec<CommandCandidate>,
    user_query: &str,
) -> anyhow::Result<Option<String>> {
    if candidates.len() == 1 {
        let candidate = &candidates[0];
        println!("Found cached command: {}", candidate.command);
        if let Some(label) = &candidate.label {
            println!("Label: {}", label);
        }
        return confirm_and_run_command(&candidate.command);
    }

    println!("Found cached commands for: \"{}\"", user_query);
    println!();

    for (i, candidate) in candidates.iter().enumerate() {
        let label_text = candidate
            .label
            .as_ref()
            .map(|l| format!(" ({})", l))
            .unwrap_or_default();
        println!("  [{}] {}{}", i + 1, candidate.command, label_text);
    }
    println!();

    loop {
        print!("Choose [1-{}], (g)enerate new, (q)uit: ", candidates.len());
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_lowercase();

        if input == "q" {
            return Ok(None);
        } else if input == "g" {
            return Err(anyhow::anyhow!("generate_new")); // Signal to generate new commands
        } else if let Ok(choice) = input.parse::<usize>() {
            if choice >= 1 && choice <= candidates.len() {
                let candidate = &candidates[choice - 1];
                println!("Selected: {}", candidate.command);
                return confirm_and_run_command(&candidate.command);
            }
        }

        println!("Invalid choice. Please try again.");
    }
}

fn handle_candidate_selection(candidates: Vec<CommandCandidate>) -> anyhow::Result<Option<String>> {
    if candidates.is_empty() {
        return Ok(None);
    }

    if candidates.len() == 1 {
        let candidate = &candidates[0];
        println!("Generated command: {}", candidate.command);
        if let Some(label) = &candidate.label {
            println!("Label: {}", label);
        }
        return confirm_and_run_command(&candidate.command);
    }

    println!("Generated command options:");
    println!();

    for (i, candidate) in candidates.iter().enumerate() {
        let label_text = candidate
            .label
            .as_ref()
            .map(|l| format!(" ({})", l))
            .unwrap_or_default();
        println!("  [{}] {}{}", i + 1, candidate.command, label_text);
    }
    println!();

    print!("Choose [1-{}], (q)uit: ", candidates.len());
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim().to_lowercase();

    if input == "q" {
        return Ok(None);
    } else if let Ok(choice) = input.parse::<usize>() {
        if choice >= 1 && choice <= candidates.len() {
            let candidate = &candidates[choice - 1];
            println!("Selected: {}", candidate.command);
            return confirm_and_run_command(&candidate.command);
        }
    }

    println!("Invalid choice. Please try again.");
    handle_candidate_selection(candidates)
}

pub fn normalize_ollama_url(base: &str) -> String {
    let b = base.trim_end_matches('/');
    if b.ends_with("/api/chat") || b.ends_with("/api/generate") {
        b.to_string()
    } else {
        format!("{}/api/chat", b)
    }
}

pub fn clear_last_lines(lines: usize) {
    if lines == 0 {
        return;
    }
    for _ in 0..lines {
        print!("\x1b[1A");
        print!("\x1b[2K");
    }
    io::stdout().flush().ok();
}

pub fn save_cursor() {
    print!("\x1b7");
    io::stdout().flush().ok();
}

pub fn restore_cursor_and_clear_to_end() {
    print!("\x1b8\x1b[J");
    io::stdout().flush().ok();
}

pub async fn stream_assistant_content(
    client: &reqwest::Client,
    config: &Config,
    messages: &[Message],
) -> anyhow::Result<(String, bool)> {
    let req = ChatRequest {
        model: &config.ollama_model,
        messages,
        stream: true,
    };

    let url = normalize_ollama_url(&config.ollama_base_url);

    let resp = client
        .post(url)
        .json(&req)
        .send()
        .await
        .context("Failed contacting Ollama")?
        .error_for_status()
        .context("Ollama returned non-2xx status")?;

    let byte_stream = resp
        .bytes_stream()
        .map(|r| r.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)));
    let reader = StreamReader::new(byte_stream);
    let mut lines = AsyncBufReader::new(reader).lines();

    let mut full = String::new();
    let mut printed_anything = false;

    while let Some(line) = lines.next_line().await? {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Ollama streams JSON lines
        let Ok(v) = serde_json::from_str::<ChatResponse>(line) else {
            continue;
        };

        if v.message.role == "assistant" && !v.message.content.is_empty() {
            let chunk = &v.message.content;

            full.push_str(chunk);

            printed_anything = true;
            print!("{chunk}");
            io::stdout().flush().ok();
        }

        if v.done {
            break;
        }
    }

    Ok((full, printed_anything))
}

pub async fn request_command_stream_then_confirm(
    config: &Config,
    messages: &[Message],
) -> anyhow::Result<Option<String>> {
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

    let user_query = messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.content.as_str())
        .unwrap_or("");

    let cache_manager = CacheManager::new(
        std::path::PathBuf::from(".cache")
            .join("vibe_cli")
            .join("commands.json"),
    );

    // Check cache first
    if let Some(cached_candidates) = cache_manager.load_cached(user_query)? {
        match handle_cached_candidates(cached_candidates, user_query) {
            Ok(Some(cmd)) => return Ok(Some(cmd)),
            Ok(None) => return Ok(None), // User chose to quit
            Err(_) => {
                // User chose "g" to generate new, fall through to generation
                println!("Generating new commands...");
            }
        }
    }

    let instruction = format!(
        r#"You are a CLI assistant.

Goal:
- Help the user by suggesting 1–3 safe, relevant shell commands (prefer 1 if possible).
- Briefly explain what the command does and why it is useful.

Environment:
- platform: {platform}
- cwd: {cwd}
- project_root: {project_root}
"#
    );

    // Build messages to send: inject as system message for this request.
    let mut req_messages: Vec<Message> = Vec::with_capacity(messages.len() + 1);
    req_messages.push(Message {
        role: "system".to_string(),
        content: instruction,
    });
    req_messages.extend_from_slice(messages);

    // Optional: keep your cursor UX (no retries, but still cleanly prints one attempt)
    save_cursor();

    let (raw, printed_anything) = stream_assistant_content(&client, config, &req_messages).await?;

    if printed_anything {
        println!();
    }

    // Extract command candidates flexibly
    let candidates = extract_commands(&raw, user_query);

    if candidates.is_empty() {
        // No retry; just return a useful error that includes raw output for debugging.
        // (You could also return Ok(None) if you want “no command found” to be non-fatal.)
        anyhow::bail!("No valid command candidates found in model output.\nRaw output:\n{raw}");
    }

    // Cache and prompt user to choose
    cache_manager.save_cached(user_query, candidates.clone())?;
    handle_candidate_selection(candidates)
}
