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

    /// Initialize domain config directory
    #[arg(long)]
    pub neurosymbolic_init: bool,

    /// Install a domain package (URL or name)
    #[arg(long)]
    pub neurosymbolic_install: Option<String>,

    /// Remove a domain
    #[arg(long)]
    pub neurosymbolic_remove: Option<String>,

    /// Edit a domain config
    #[arg(long)]
    pub neurosymbolic_edit: Option<String>,

    /// Add a new domain from template
    #[arg(long)]
    pub neurosymbolic_add: Option<String>,

    /// List installed domains
    #[arg(long)]
    pub neurosymbolic_list: bool,

    /// Use AI to interpret command output (make it readable)
    #[arg(long)]
    pub ai_interpret: bool,

    /// Clear the command cache
    #[arg(long)]
    pub clear_cache: bool,

    /// Validate command syntax against man pages
    #[arg(long)]
    pub validate_syntax: bool,

    /// Disable learning/RAG features
    #[arg(long)]
    pub no_learning: bool,

    /// Output FQL (Formal Query Language) representation
    #[arg(long)]
    pub fql_output: bool,

    /// Show reasoning trace
    #[arg(long)]
    pub trace: bool,

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
            self.handlers
                .handle_neurosymbolic(&args_str, cli.ai_interpret)
                .await
        } else if cli.neurosymbolic_init {
            self.handlers.handle_neurosymbolic_init().await
        } else if let Some(domain) = cli.neurosymbolic_install {
            self.handlers.handle_neurosymbolic_install(&domain).await
        } else if let Some(domain) = cli.neurosymbolic_remove {
            self.handlers.handle_neurosymbolic_remove(&domain).await
        } else if let Some(domain) = cli.neurosymbolic_edit {
            self.handlers.handle_neurosymbolic_edit(&domain).await
        } else if let Some(domain) = cli.neurosymbolic_add {
            self.handlers.handle_neurosymbolic_add(&domain).await
        } else if cli.neurosymbolic_list {
            self.handlers.handle_neurosymbolic_list().await
        } else if cli.clear_cache {
            self.handlers.handle_clear_cache()
        } else {
            self.handlers
                .handle_query(&args_str, cli.ai_interpret, false)
                .await
        }
    }
}
