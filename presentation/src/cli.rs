mod cache;
mod command_extraction;
mod handlers;
mod streaming;
mod utils;

pub use command_extraction::{extract_command, extract_command_from_response};
pub use handlers::CliHandlers;
pub use streaming::{restore_cursor_and_clear_to_end, save_cursor, stream_assistant_content};
pub use utils::{detect_system_info, find_project_root, floor_char_boundary, project_cache_suffix};

use clap::Parser;
use infrastructure::config::Config;
use serde::{Deserialize, Serialize};
use shared::types::Result;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

pub async fn request_command_stream_then_confirm(
    config: &Config,
    messages: &[Message],
) -> Result<Option<String>> {
    const MAX_TRIES: usize = 3;

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

    let _base_instruction = format!(
        r#"You are a CLI assistant that returns ONE safe shell command.

Environment:
- platform: {platform}
- cwd: {cwd}
- project_root: {project_root}

OUTPUT FORMAT (STRICT):
- First: 1–3 short plain-text sentences (no lists).
- Last line: exactly `COMMAND: <one command>`
- After the `COMMAND:` line, output NOTHING else.

ABSOLUTE RULES:
- No Markdown/backticks/code fences/bullets/numbered lists.
- Do NOT output `Command:` or any other prefix. Only `COMMAND:`.
- The LAST line must start with `COMMAND:` and contain exactly one command.
- If you cannot comply, output ONLY:
  COMMAND: echo "ERROR: no safe command"

SAFETY:
- Non-destructive by default. Never delete/overwrite/move/edit unless explicitly asked.
- No network access or installs unless explicitly asked.
- Do not use sudo unless explicitly asked.

GUIDANCE (Linux):
- Battery: `acpi -b` (if available) else read `/sys/class/power_supply`.
- GPU: `lspci -k | grep -A3 -Ei 'vga|3d|display'`.
- RAM: `free -h`.
- CPU: `lscpu`.
- Disk: filesystem -> `df -h`; directory -> `du -sh <path>`.

Now follow OUTPUT FORMAT (STRICT)."#
    );

    let mut last_raw = String::new();
    let _last_printed_lines: usize = 0;

    for attempt in 1..=MAX_TRIES {
        if attempt > 1 {
            restore_cursor_and_clear_to_end();
        }

        save_cursor();

        println!("--- [attempt {attempt}/{MAX_TRIES}] ---");

        let (raw, printed_anything) = stream_assistant_content(&client, config, &messages).await?;
        last_raw = raw.clone();

        if printed_anything {
            println!();
        }

        let user_query = messages
            .iter()
            .rev()
            .find(|m| m.role == "user")
            .map(|m| m.content.as_str())
            .unwrap_or("");

        if let Some(cmd) = extract_command(&raw, user_query) {
            return Ok(Some(cmd));
        }

        eprintln!("(Format invalid, retrying...)");
    }

    anyhow::bail!(
        "Model did not produce a COMMAND: line with a command after {MAX_TRIES} attempts.\nLast output:\n{last_raw}"
    );
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
