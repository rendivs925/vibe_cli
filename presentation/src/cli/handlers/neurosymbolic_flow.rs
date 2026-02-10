use super::CliHandlers;
use colored::Colorize;
use infrastructure::storage::experience_buffer::FailureType;
use shared::confirmation::ask_confirmation;
use shared::types::{Message, Result};

impl CliHandlers {
    pub async fn handle_neurosymbolic(
        &mut self,
        query: &str,
        ai_interpret: bool,
        use_rag_constraints: bool,
    ) -> Result<()> {
        self.ensure_integrated_service();
        if let Some(answer) = self.direct_answer(query) {
            println!("{}", answer);
            return Ok(());
        }

        let mut attempts = 0;
        let max_attempts = 3;
        let mut critique_feedback: Option<String> = None;
        let allowed_commands = if use_rag_constraints {
            self.allowed_commands_from_rag(query).await?
        } else {
            None
        };

        loop {
            attempts += 1;
            let mut messages: Vec<Message> = Vec::new();
            if critique_feedback.is_none() {
                if let Some(ctx) = self.build_learning_context_message(query) {
                    messages.push(ctx);
                }
            }

            messages.push(Self::user_message(query, critique_feedback.as_deref()));
            let (_, candidates) =
                crate::cli::streaming::request_command_candidates_from_llm(
                    &self.config,
                    &messages,
                    Some(query),
                )
                .await?;

            if candidates.is_empty() {
                return Ok(());
            }

            let (valid_candidates, has_symbolic, validation, suggestion) =
                self.filter_candidates_by_domain(query, candidates, allowed_commands.as_ref());

            if !valid_candidates.is_empty() {
                if let Some(cmd) =
                    crate::cli::streaming::select_command_from_candidates(valid_candidates, query)?
                {
                    self.execute_or_interpret(query, &cmd, ai_interpret).await?;
                }
                return Ok(());
            }

            if attempts == 1 && has_symbolic {
                if let Some(symbolic) = suggestion.as_ref() {
                    let chosen = self.select_symbolic_command(symbolic, query)?;
                    if let Some(cmd) = chosen {
                        self.execute_or_interpret(query, &cmd, ai_interpret).await?;
                    }
                    return Ok(());
                }
            }

            let Some(validation) = validation else {
                eprintln!("No symbolic match available; falling back to standard query...");
                return self.handle_query(query, ai_interpret, true).await;
            };

            if attempts >= max_attempts {
                let reason = validation
                    .reason
                    .as_deref()
                    .unwrap_or("symbolic validation failed");
                eprintln!("Symbolic validation failed: {}", reason);
                if let Some(service) = self.integrated_service.as_ref() {
                    let _ = service.record_failure(query, "", FailureType::Other, Some(reason));
                }
                eprintln!("Falling back to standard query...");
                return self.handle_query(query, ai_interpret, true).await;
            }

            critique_feedback = Some(self.build_domain_critique_prompt(query, &validation));
        }
    }

    pub async fn handle_query(
        &mut self,
        query: &str,
        ai_interpret: bool,
        from_fallback: bool,
    ) -> Result<()> {
        let mut last_successful_command = String::new();
        let mut last_successful_query = String::new();

        let messages = vec![Message {
            role: "user".to_string(),
            content: query.to_string(),
        }];

        let command =
            crate::cli::streaming::request_command_stream_then_confirm(&self.config, &messages)
                .await?;
        let Some(cmd) = command else {
            return Ok(());
        };

        let output = self.run_shell_command(&cmd)?;
        if ai_interpret {
            self.interpret_output(query, &output.full_output).await?;
        } else {
            println!("{}", output.stdout);
        }

        if !output.status.success() {
            println!(
                "{}",
                format!("Command failed with exit code: {:?}", output.status.code()).red()
            );
            if !output.stderr.is_empty() {
                println!("{}", output.stderr.red());
            }
        } else {
            last_successful_command = cmd;
            last_successful_query = query.to_string();
        }

        if from_fallback && !last_successful_command.is_empty() {
            if !self.is_known_operation(&last_successful_query, &last_successful_command) {
                if ask_confirmation(
                    "\nCommand succeeded! Learn this for future neurosymbolic queries?",
                    false,
                )? {
                    self.learn_command(&last_successful_query, &last_successful_command)
                        .await?;
                }
            }
        }

        Ok(())
    }

    async fn execute_or_interpret(
        &self,
        query: &str,
        cmd: &str,
        ai_interpret: bool,
    ) -> Result<()> {
        let output = self.run_shell_command(cmd)?;
        if ai_interpret {
            self.interpret_output(query, &output.full_output).await?;
        } else {
            println!("{}", output.stdout);
        }

        if !output.status.success() {
            println!(
                "{}",
                format!("Command failed with exit code: {:?}", output.status.code()).red()
            );
            if !output.stderr.is_empty() {
                println!("{}", output.stderr.red());
            }
            if let Some(service) = self.integrated_service.as_ref() {
                let _ = service.record_failure(
                    query,
                    cmd,
                    FailureType::ExecutionFailed,
                    Some(output.stderr.trim()),
                );
            }
        } else if let Some(service) = self.integrated_service.as_ref() {
            let _ = service.record_success(query, cmd);
        }

        Ok(())
    }
}
