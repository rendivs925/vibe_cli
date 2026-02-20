use super::CliHandlers;
use crate::cli::command_extraction::parse_agent_plan;
use application::services::test_time_scaling::{ScalingConfig, ScalingMethod};
use colored::Colorize;
use shared::theme;
use shared::confirmation::ask_confirmation;
use shared::types::Result;
use std::process::Command;

impl CliHandlers {
    pub async fn handle_agent(&self, task: &str, scaling_config: &ScalingConfig) -> Result<()> {
        let client = infrastructure::ollama_client::OllamaClient::new()?;

        let commands = if scaling_config.method != ScalingMethod::None {
            if let Some(best_cmd) = self.select_best_with_scaling(task, scaling_config).await {
                vec![best_cmd]
            } else {
                let prompt = format!(
                    "You are an assistant that turns a user's goal into a sequence of POSIX shell commands that can be run one-by-one with confirmation in between.\n\
                    Environment: {}.\n\
                    Constraints:\n\
                    - Respond ONLY with a JSON array of strings. Each element must be a complete shell command ready to run.\n\
                    - No prose, no markdown, no comments. If you cannot produce a valid JSON array, respond with [].\n\
                    - Prefer Debian/Ubuntu defaults (apt/apt-get, systemctl) unless otherwise implied.\n\
                    - Use real paths; avoid placeholders like /path/to.\n\
                    - Keep commands minimal and idempotent (check state before changing it).\n\n\
                    User request: {}",
                    self.system_info, task
                );
                let response = client.generate_response(&prompt).await?;
                parse_agent_plan(&response)
            }
        } else {
            let prompt = format!(
                "You are an assistant that turns a user's goal into a sequence of POSIX shell commands that can be run one-by-one with confirmation in between.\n\
Environment: {}.\n\
Constraints:\n\
- Respond ONLY with a JSON array of strings. Each element must be a complete shell command ready to run.\n\
- No prose, no markdown, no comments. If you cannot produce a valid JSON array, respond with [].\n\
- Prefer Debian/Ubuntu defaults (apt/apt-get, systemctl) unless otherwise implied.\n\
- Use real paths; avoid placeholders like /path/to.\n\
- Keep commands minimal and idempotent (check state before changing it).\n\n\
User request: {}",
                self.system_info, task
            );
            let response = client.generate_response(&prompt).await?;
            parse_agent_plan(&response)
        };

        if commands.is_empty() {
            println!(
                "{}",
                theme::error("Model did not return a runnable command list (expected JSON array).")
            );
            return Ok(());
        }

        println!("\n{}", theme::accent("Proposed plan:"));
        for (i, cmd) in commands.iter().enumerate() {
            println!("  {} {}", theme::info(&format!("[{}]", i + 1)), cmd);
        }

        for (i, cmd) in commands.iter().enumerate() {
            println!(
                "\n{} {}",
                theme::accent("Step").bold(),
                theme::accent(&format!("{}:", i + 1)).bold()
            );
            println!("{} {}", theme::accent("Suggested command:"), theme::warning(cmd));
            let accept = ask_confirmation("Run this command?", false)?;
            if !accept {
                println!("{}", theme::warning("Skipping this step."));
                continue;
            }
            let status = Command::new("bash").arg("-c").arg(cmd).status()?;
            if status.success() {
                println!("{}", theme::success("Command completed successfully."));
            } else {
                println!(
                    "{} (exit status: {:?})",
                    theme::error("Command failed."),
                    status.code()
                );
            }
        }
        Ok(())
    }
}
