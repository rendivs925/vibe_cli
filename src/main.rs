mod config;
mod model;
mod session;
mod safety;
mod runner;
mod prompt;
mod agent;
mod scriptgen;
mod clipboard;

use clap::{ArgAction, Parser};
use config::Config;
use session::ChatSession;
use anyhow::Result;

/// Qwen-powered ultra-safe CLI assistant using a local Ollama server.
#[derive(Parser, Debug)]
#[command(name = "qwen_cli_assistant")]
#[command(about = "Ultra-safe CLI assistant powered by Qwen via Ollama", long_about = None)]
struct Cli {
    /// Use interactive chat mode
    #[arg(long, action = ArgAction::SetTrue)]
    chat: bool,

    /// Use multi-step agent mode (plan several commands)
    #[arg(long, action = ArgAction::SetTrue)]
    agent: bool,

    /// Generate a bash script instead of running commands
    #[arg(long, action = ArgAction::SetTrue)]
    script: bool,

    /// Output file for --script mode
    #[arg(short = 'o', long)]
    output: Option<String>,

    /// Relax safety checks (still asks for confirmation)
    #[arg(long, action = ArgAction::SetTrue)]
    unsafe_mode: bool,

    /// Do not use or update cache
    #[arg(long, action = ArgAction::SetTrue)]
    no_cache: bool,

    /// Copy suggested command to clipboard
    #[arg(long, action = ArgAction::SetTrue)]
    copy: bool,

    /// Clear cache and retrain (start fresh)
    #[arg(long, action = ArgAction::SetTrue)]
    retrain: bool,

    /// Inline prompt for one-shot mode (if empty, will ask interactively)
    #[arg(value_parser, trailing_var_arg = true)]
    prompt: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let prompt_text = if !cli.prompt.is_empty() {
        cli.prompt.join(" ")
    } else if !cli.chat && !cli.agent && !cli.script {
        // Only ask interactively when not in chat/agent/script explicit modes
        prompt::ask_user_prompt()?
    } else {
        String::new()
    };

    let config = Config::new(!cli.unsafe_mode, !cli.no_cache, cli.copy);

    if cli.retrain {
        config.clear_cache()?;
        println!("Cache cleared. Starting fresh.");
        if cli.prompt.is_empty() && !cli.chat && !cli.agent && !cli.script {
            return Ok(());
        }
    }

    if cli.chat {
        run_chat_mode(&config).await?;
        return Ok(());
    }

    if cli.agent {
        agent::run_agent_mode(&config, &prompt_text).await?;
        return Ok(());
    }

    if cli.script {
        scriptgen::run_script_mode(&config, &prompt_text, cli.output.as_deref()).await?;
        return Ok(());
    }

    // Default: one-shot prompt -> single command
    run_one_shot(&config, &prompt_text).await?;

    Ok(())
}

async fn run_chat_mode(config: &Config) -> Result<()> {
    let mut session = ChatSession::new(config.safe_mode);

    loop {
        let user_input = prompt::ask_chat_turn()?;
        if user_input.trim().is_empty() {
            continue;
        }
        if user_input.trim().eq_ignore_ascii_case("exit")
            || user_input.trim().eq_ignore_ascii_case("quit")
        {
            break;
        }

        session.push_user(user_input.clone());

        eprintln!("Thinking...");
        let cmd = model::request_command(config, &session.messages).await?;
        session.push_assistant(cmd.clone());

        runner::confirm_and_run(&cmd, config)?;
    }

    Ok(())
}

async fn run_one_shot(config: &Config, prompt_text: &str) -> Result<()> {
    let mut session = ChatSession::new(config.safe_mode);
    session.push_user(prompt_text.to_string());

    eprintln!("Thinking...");
    let cmd = model::request_command(config, &session.messages).await?;
    session.push_assistant(cmd.clone());

    if config.cache_enabled {
        config.save_cached(prompt_text, &cmd)?;
    }

    runner::confirm_and_run(&cmd, config)?;

    Ok(())
}
