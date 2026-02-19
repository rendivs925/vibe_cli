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

use application::services::test_time_scaling::{ScalingConfig, ScalingMethod};
use clap::Parser;
use infrastructure::config::Config;
use shared::types::Result;

#[derive(Clone, Copy, Debug, Default, clap::ValueEnum)]
pub enum ScalingMethodArg {
    #[default]
    None,
    Knockout,
    League,
}

impl From<ScalingMethodArg> for ScalingMethod {
    fn from(arg: ScalingMethodArg) -> Self {
        match arg {
            ScalingMethodArg::None => ScalingMethod::None,
            ScalingMethodArg::Knockout => ScalingMethod::Knockout,
            ScalingMethodArg::League => ScalingMethod::League,
        }
    }
}

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

    /// Use ReAct iterative reasoning mode
    #[arg(long)]
    pub react: bool,

    /// Explain a file
    #[arg(long)]
    pub explain: bool,

    /// Query with RAG context
    #[arg(long)]
    pub rag: bool,

    /// Load context from path
    #[arg(long)]
    pub context: bool,

    /// Use RAG context to constrain neurosymbolic command generation
    #[arg(long)]
    pub neurosymbolic_rag: bool,

    /// Initialize domain config directory
    #[arg(long)]
    pub neurosymbolic_init: bool,

    /// Use neurosymbolic command generation (config-driven)
    #[arg(long)]
    pub neurosymbolic: bool,

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

    /// Clear the RAG answer cache only
    #[arg(long)]
    pub clear_rag_cache: bool,

    /// Clear the RAG embeddings index for this project
    #[arg(long)]
    pub clear_embeddings: bool,

    /// Validate command syntax against man pages
    #[arg(long)]
    pub validate_syntax: bool,

    /// Disable learning/RAG features
    #[arg(long)]
    pub no_learning: bool,

    /// Show reasoning trace
    #[arg(long)]
    pub trace: bool,

    /// Test-time compute scaling method: knockout, league, or none (default: knockout)
    #[arg(long, value_enum, default_value = "knockout")]
    pub scaling_method: ScalingMethodArg,

    /// Number of candidate samples for test-time compute (default: 6)
    #[arg(long)]
    pub samples: Option<usize>,

    /// Comparisons per pair for knockout tournament (default: 3)
    #[arg(long)]
    pub comparisons: Option<usize>,

    /// Random opponents per candidate for league (default: 5)
    #[arg(long)]
    pub opponents: Option<usize>,

    /// Enable early stopping when confidence is high
    #[arg(long)]
    pub early_stop: Option<bool>,

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

        let scaling_config = ScalingConfig {
            method: cli.scaling_method.into(),
            num_samples: cli.samples.unwrap_or(6),
            comparisons_per_pair: cli.comparisons.unwrap_or(3),
            opponents_per_candidate: cli.opponents.unwrap_or(5),
            early_stopping: cli.early_stop.unwrap_or(true),
            confidence_threshold: 0.85,
        };

        if cli.chat {
            return self.handlers.handle_chat().await;
        }
        if cli.agent {
            return self.handlers.handle_agent(&args_str, &scaling_config).await;
        }
        if cli.react {
            return self
                .handlers
                .handle_react(&args_str, cli.neurosymbolic, &scaling_config)
                .await;
        }
        if cli.explain {
            return self
                .handlers
                .handle_explain(&args_str, &scaling_config)
                .await;
        }
        if cli.rag {
            return self
                .handlers
                .handle_rag(&args_str, &scaling_config)
                .await;
        }
        if cli.context {
            return self.handlers.handle_context(&args_str).await;
        }
        if cli.neurosymbolic_init {
            return self.handlers.handle_neurosymbolic_init().await;
        }
        if let Some(domain) = cli.neurosymbolic_install {
            return self.handlers.handle_neurosymbolic_install(&domain).await;
        }
        if let Some(domain) = cli.neurosymbolic_remove {
            return self.handlers.handle_neurosymbolic_remove(&domain).await;
        }
        if let Some(domain) = cli.neurosymbolic_edit {
            return self.handlers.handle_neurosymbolic_edit(&domain).await;
        }
        if let Some(domain) = cli.neurosymbolic_add {
            return self.handlers.handle_neurosymbolic_add(&domain).await;
        }
        if cli.neurosymbolic_list {
            return self.handlers.handle_neurosymbolic_list().await;
        }
        if cli.clear_cache {
            return self.handlers.handle_clear_cache();
        }
        if cli.clear_rag_cache {
            return self.handlers.handle_clear_rag_cache();
        }
        if cli.clear_embeddings {
            return self.handlers.handle_clear_embeddings();
        }

        // If --neurosymbolic flag is set, use neurosymbolic mode with scaling
        if cli.neurosymbolic {
            return self
                .handlers
                .handle_neurosymbolic(
                    &args_str,
                    cli.ai_interpret,
                    cli.neurosymbolic_rag,
                    &scaling_config,
                )
                .await;
        }

        // Default: use standard LLM query mode with optional scaling
        self.handlers
            .handle_query_with_scaling(&args_str, cli.ai_interpret, false, &scaling_config)
            .await
    }
}
