use shared::confirmation::{ask_command_confirmation, ask_selection};
use shared::types::Message;
use std::env;
use std::io::{self, Write};

use crate::cli::cache::CacheManager;
use crate::cli::cache::CommandCandidate;
use crate::cli::command_extraction::extract_commands;
use crate::cli::utils::*;
use anyhow::Context;
use futures_util::StreamExt;
use infrastructure::config::Config;
use serde::{Deserialize, Serialize};
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

fn confirm_and_run_cached_command(command: &str) -> anyhow::Result<Option<String>> {
    match ask_command_confirmation("Run this command?", true)? {
        Some(true) => Ok(Some(command.to_string())),
        Some(false) => {
            println!("Command cancelled.");
            Ok(None)
        }
        None => Err(anyhow::anyhow!("generate_new")), // Signal to generate new commands
    }
}

fn confirm_and_run_generated_command(command: &str) -> anyhow::Result<Option<String>> {
    match ask_command_confirmation("Run this command?", false)? {
        Some(true) => Ok(Some(command.to_string())),
        Some(false) => {
            println!("Command cancelled.");
            Ok(None)
        }
        None => Ok(None), // This shouldn't happen with allow_generate=false, but handle gracefully
    }
}

fn validate_command(command: &str) -> bool {
    if command.trim().is_empty() {
        return false;
    }

    // Check if command contains any dangerous patterns
    let dangerous_patterns = [
        "rm -rf",
        "rm -r",
        "dd if=",
        "mkfs",
        "format",
        "shred",
        "wipe",
        "fdisk",
        "sfdisk",
        "parted",
        "dd of=",
        "> /dev",
        "< /dev",
        "2> /dev",
    ];

    if dangerous_patterns.iter().any(|pattern| command.to_lowercase().contains(pattern)) {
        return false;
    }

    // Check for shell injection patterns
    let injection_patterns = [
        "; rm",
        "&& rm",
        "|| rm",
        "$(rm",
        "`rm`",
        "| rm",
        "> rm",
        "< rm",
    ];

    if injection_patterns.iter().any(|pattern| command.contains(pattern)) {
        return false;
    }

    // Extract the first word as the command
    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.is_empty() {
        return false;
    }

    let cmd_name = parts[0];
    
    // Skip validation for common built-in commands
    let builtins = ["echo", "cd", "pwd", "ls", "cat", "grep", "find", "which", "type"];
    if builtins.contains(&cmd_name) {
        return true;
    }

    // Check if command exists in PATH without executing it
    // Use `which` command to check availability without running the command
    match std::process::Command::new("which")
        .arg(cmd_name)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output() {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}

fn handle_cached_candidates(
    candidates: Vec<CommandCandidate>,
    user_query: &str,
) -> anyhow::Result<Option<String>> {
    // Filter out invalid commands
    let valid_candidates: Vec<CommandCandidate> = candidates
        .iter()
        .filter(|candidate| validate_command(&candidate.command))
        .cloned()
        .collect();

    if valid_candidates.is_empty() {
        println!("No valid cached commands found, generating new ones...");
        return Ok(None);
    }

    if valid_candidates.len() == 1 {
        let candidate = &valid_candidates[0];
        println!("Found cached command: {}", candidate.command);
        if let Some(label) = &candidate.label {
            println!("Label: {}", label);
        }
        return confirm_and_run_cached_command(&candidate.command);
    }

    println!("Found cached commands for: \"{}\"", user_query);
    println!();

    let options: Vec<String> = valid_candidates
        .iter()
        .map(|candidate| {
            let label_text = candidate
                .label
                .as_ref()
                .map(|l| format!(" ({})", l))
                .unwrap_or_default();
            format!("{}{}", candidate.command, label_text)
        })
        .collect();

    // Display options
    for (i, option) in options.iter().enumerate() {
        println!("  [{}] {}", i + 1, option);
    }
    println!();

    match ask_selection(&options, true) {
        Ok(Some(index)) => {
            let candidate = &candidates[index];
            confirm_and_run_cached_command(&candidate.command)
        }
        Ok(None) => Ok(None),
        Err(_) => Err(anyhow::anyhow!("generate_new")), // Signal to generate new commands
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
        return confirm_and_run_generated_command(&candidate.command);
    }

    println!("Generated command options:");
    println!();

    let options: Vec<String> = candidates
        .iter()
        .map(|candidate| {
            let label_text = candidate
                .label
                .as_ref()
                .map(|l| format!(" ({})", l))
                .unwrap_or_default();
            format!("{}{}", candidate.command, label_text)
        })
        .collect();

    // Display options
    for (i, option) in options.iter().enumerate() {
        println!("  [{}] {}", i + 1, option);
    }
    println!();

    match ask_selection(&options, false) {
        Ok(Some(index)) => {
            let candidate = &candidates[index];
            confirm_and_run_generated_command(&candidate.command)
        }
        Ok(None) => Ok(None),
        Err(_) => {
            println!("Invalid choice. Please try again.");
            handle_candidate_selection(candidates)
        }
    }
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
        std::path::PathBuf::from(env::var("HOME").unwrap_or_else(|_| ".".to_string()))
            .join(".local")
            .join("share")
            .join("vibe_cli")
            .join(format!("{}_cli_cache.bin", project_cache_suffix())),
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
        
STRICT OUTPUT CONTRACT:
- No JSON

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
    let all_candidates = extract_commands(&raw, user_query);

    // Filter to only valid shell commands - reject prose, descriptions, and natural language
    let valid_candidates: Vec<CommandCandidate> = all_candidates
        .into_iter()
        .filter(|c| {
            let cmd = &c.command;
            if cmd.is_empty() {
                return false;
            }

            let lower = cmd.to_ascii_lowercase();

            // Reject lines that are clearly prose/descriptions
            let prose_indicators = [
                "get the",
                "show the",
                "check the",
                "display the",
                "list the",
                "find the",
                "retrieve the",
                "execute the",
                "run the",
                "cpu",
                "disk",
                "memory",
                "hostname",
                "operating system",
                "platform",
                "kernel",
                "shell",
                "total ram",
                "free ram",
                "cpu type",
                "processor speed",
                "information:",
            ];
            if prose_indicators.iter().any(|p| lower.contains(p)) {
                return false;
            }

            // Reject lines with embedded colons followed by prose (like "CWD: value")
            if cmd.contains(": ") && !cmd.contains("|") && !cmd.contains(";") {
                return false;
            }

            // Reject lines that are just variable assignments or echo statements without actual commands
            if lower.starts_with("echo ") && cmd.split_whitespace().count() < 3 {
                return false;
            }

            // Accept if it contains shell operators, flags, or is a known command
            let has_shell_features = cmd.contains('|')
                || cmd.contains("&&")
                || cmd.contains(";")
                || cmd.contains(" -")
                || cmd.contains("--")
                || cmd.contains("$(")
                || cmd.starts_with("uname ")
                || cmd.starts_with("hostname ")
                || cmd.starts_with("free ")
                || cmd.starts_with("df ")
                || cmd.starts_with("ls ")
                || cmd.starts_with("ps ")
                || cmd.starts_with("top ")
                || cmd.starts_with("cat ")
                || cmd.starts_with("grep ")
                || cmd.starts_with("awk ")
                || cmd.starts_with("sed ")
                || cmd.starts_with("find ")
                || cmd.starts_with("lspci ")
                || cmd.starts_with("nvidia-smi");

            has_shell_features
        })
        .collect();

    // Only cache if we have valid commands
    if !valid_candidates.is_empty() {
        cache_manager.save_cached(user_query, valid_candidates.clone())?;
    }
    handle_candidate_selection(valid_candidates)
}
