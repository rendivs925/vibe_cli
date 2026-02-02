pub use crate::cli::command_extraction::{
    extract_command, extract_command_from_response, extract_commands,
};
pub use crate::cli::handlers::CliHandlers;
pub use crate::cli::streaming::{
    restore_cursor_and_clear_to_end, save_cursor, stream_assistant_content,
};
pub use crate::cli::utils::{
    detect_system_info, find_project_root, floor_char_boundary, project_cache_suffix,
};

use crate::cli::cache::{CacheManager, CommandCandidate};
use clap::Parser;
use infrastructure::config::Config;
use serde::{Deserialize, Serialize};
use shared::types::Result;
use std::io::{self, Write};

#[derive(Parser)]
#[command(name = "vibe_cli")]
#[command(about = "Vibe CLI assistant with RAG and neurosymbolic capabilities")]
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

    /// Use neurosymbolic reasoning with domain configs
    #[arg(long)]
    pub neurosymbolic: bool,

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
        } else if cli.neurosymbolic {
            self.handlers.handle_neurosymbolic(&args_str).await
        } else {
            self.handlers.handle_query(&args_str).await
        }
    }
}
