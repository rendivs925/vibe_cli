mod cache;
mod command_extraction;
mod handlers;
mod streaming;
mod utils;

pub use command_extraction::{extract_command, extract_command_from_response, extract_commands};
pub use handlers::CliHandlers;
pub use streaming::{restore_cursor_and_clear_to_end, save_cursor, stream_assistant_content};
pub use utils::{detect_system_info, find_project_root, floor_char_boundary, project_cache_suffix};

use cache::{CacheManager, CommandCandidate};
use clap::Parser;
use infrastructure::config::Config;
use serde::{Deserialize, Serialize};
use shared::types::Result;
use std::io::{self, Write};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

pub async fn request_command_stream_then_confirm(
    config: &Config,
    messages: &[Message],
) -> Result<Option<String>> {
    let client = reqwest::Client::new();

    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "/home/user".to_string());
    let project_root = utils::find_project_root().unwrap_or_else(|| cwd.clone());

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

Output style:
- Start with 1–2 short plain-text sentences explaining the approach.
- Then list the command(s), one per line, with no prefixes.
- No markdown, no code fences, no bullets, no numbering.

Rules:
- Do NOT include destructive commands unless explicitly asked.
- No installs, no sudo, no network unless explicitly asked.
- Prefer read-only inspection commands.
- Keep explanations concise and practical.

Example output:

Shows current memory usage in a human-readable format.
free -h

Another example:

Displays total physical RAM available on the system.
cat /proc/meminfo | grep MemTotal
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

fn handle_cached_candidates(
    candidates: Vec<CommandCandidate>,
    user_query: &str,
) -> Result<Option<String>> {
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

fn handle_candidate_selection(candidates: Vec<CommandCandidate>) -> Result<Option<String>> {
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

fn confirm_and_run_command(command: &str) -> Result<Option<String>> {
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

#[derive(Parser)]
#[command(name = "vibe_cli")]
#[command(about = "Vibe CLI assistant with RAG capabilities")]
pub struct Cli {
    /// Enter interactive chat mode
    #[arg(long)]
    pub chat: bool,

    /// Use multi-step agent mode
    #[arg(long)]
    pub agent: bool,

    /// Explain a file
    #[arg(long)]
    pub explain: bool,

    /// Query with RAG context
    #[arg(long)]
    pub rag: bool,

    /// Load context from path
    #[arg(long)]
    pub context: bool,

    /// The query or file path to process
    #[arg(trailing_var_arg = true)]
    pub args: Vec<String>,
}

pub struct CliApp {
    handlers: CliHandlers,
}

impl CliApp {
    pub fn new() -> Self {
        let config = Config::load();
        let handlers = CliHandlers::new(config);
        Self { handlers }
    }

    pub async fn run(&mut self, cli: Cli) -> Result<()> {
        let args_str = cli.args.join(" ");
        if cli.chat {
            if args_str.trim().is_empty() {
                self.handlers.handle_chat().await
            } else {
                self.handlers.handle_chat().await
            }
        } else if cli.agent {
            self.handlers.handle_agent(&args_str).await
        } else if cli.explain {
            self.handlers.handle_explain(&args_str).await
        } else if cli.rag {
            self.handlers.handle_rag(&args_str).await
        } else if cli.context {
            self.handlers.handle_context(&args_str).await
        } else {
            self.handlers.handle_query(&args_str).await
        }
    }
}

