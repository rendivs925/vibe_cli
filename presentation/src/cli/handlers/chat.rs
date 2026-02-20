use super::CliHandlers;
use crate::cli::command_extraction::extract_command_from_response;
use shared::theme;
use shared::confirmation::ask_confirmation;
use shared::types::Result;
use std::process::Command;

impl CliHandlers {
    pub async fn handle_chat(&self) -> Result<()> {
        use dialoguer::{theme::ColorfulTheme, Input};
        println!("Command execution mode. Type 'exit' to quit.");
        loop {
            let input: String = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Query")
                .interact_text()?;
            if input.to_lowercase() == "exit" {
                break;
            }
            let client = infrastructure::ollama_client::OllamaClient::new()?;
            let prompt = format!(
                "You are on a system with: {}. Generate a bash command to: {}. Respond with only the exact command to run, without any formatting, backticks, quotes, or explanation. Ensure the command is complete, syntactically correct, and uses standard Unix tools. For size comparisons, use appropriate units like -BG for gigabytes in df.",
                self.system_info, input
            );
            let response = client.generate_response(&prompt).await?;
            let command = extract_command_from_response(&response);
            println!("{}", theme::success(&format!("Command: {}", command)));
            if ask_confirmation("Run this command?", false)? {
                let output = Command::new("bash").arg("-c").arg(&command).output()?;
                println!("{}", String::from_utf8_lossy(&output.stdout));
                if !output.status.success() && !output.stderr.is_empty() {
                    let message = format!(
                        "Command failed: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                    println!("{}", theme::error(&message));
                }
            } else {
                println!("{}", theme::warning("Command execution cancelled."));
            }
        }
        Ok(())
    }
}
