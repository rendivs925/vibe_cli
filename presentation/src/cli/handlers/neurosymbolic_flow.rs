use super::CliHandlers;
use application::services::test_time_scaling::ScalingConfig;
use shared::theme;
use infrastructure::cache::CommandCandidate;
use infrastructure::storage::experience_buffer::FailureType;
use shared::confirmation::ask_confirmation;
use shared::types::{Message, Result};
use std::sync::mpsc;

impl CliHandlers {
    pub async fn handle_neurosymbolic(
        &mut self,
        query: &str,
        ai_interpret: bool,
        use_rag_constraints: bool,
        scaling_config: &ScalingConfig,
    ) -> Result<()> {
        use application::services::test_time_scaling::ScalingMethod;

        self.ensure_neurosymbolic_service();
        if let Some(answer) = self.direct_answer(query) {
            println!("{}", answer);
            return Ok(());
        }

        if scaling_config.method != ScalingMethod::None {
            if let Some(best_cmd) = self.select_best_with_scaling(query, scaling_config).await {
                println!(
                    "Selected best command via {}: {}",
                    match scaling_config.method {
                        ScalingMethod::Knockout => "knockout tournament",
                        ScalingMethod::League => "league competition",
                        ScalingMethod::None => "none",
                    },
                    best_cmd
                );

                if ask_confirmation("Run this command?", true)? {
                    let mut summary = String::new();
                    let output = if ai_interpret {
                        let (tx, rx) = mpsc::channel();
                        let (ack_tx, ack_rx) = mpsc::channel();
                        let handle = self.spawn_incremental_interpreter(query, rx, ack_tx);
                        let sink = super::OutputSink { tx, ack: ack_rx };
                        let result =
                            self.run_shell_command_streaming_with_sink(&best_cmd, Some(sink))?;
                        summary = handle.join().unwrap_or_default();
                        result
                    } else {
                        self.run_shell_command_streaming(&best_cmd)?
                    };

                    if output.status.success() {
                        let candidate = CommandCandidate::new(best_cmd.clone());
                        let _ = self
                            .cache_manager
                            .save_command_cached(query, vec![candidate]);
                    }

                    if ai_interpret {
                        self.interpret_output_final(query, &output.full_output, &summary)
                            .await?;
                    }
                }
                return Ok(());
            }
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
            let (_, candidates) = crate::cli::streaming::request_command_candidates_from_llm(
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
                if let Some(service) = self.neurosymbolic_service.as_ref() {
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

        // Check cache first for successful commands
        if !from_fallback {
            if let Ok(Some(cached_commands)) = self.cache_manager.load_command_cached(query) {
                if let Some(first_candidate) = cached_commands.first() {
                    println!("Found cached command for: {}", query);
                    println!("Using: {}", first_candidate.command);

                    if ask_confirmation("Use cached command?", true)? {
                        let cmd = &first_candidate.command;
                        let mut summary = String::new();
                        let output = if ai_interpret {
                            let (tx, rx) = mpsc::channel();
                            let (ack_tx, ack_rx) = mpsc::channel();
                            let handle = self.spawn_incremental_interpreter(query, rx, ack_tx);
                            let sink = super::OutputSink { tx, ack: ack_rx };
                            let result =
                                self.run_shell_command_streaming_with_sink(cmd, Some(sink))?;
                            summary = handle.join().unwrap_or_default();
                            result
                        } else {
                            self.run_shell_command_streaming(cmd)?
                        };

                        if output.status.success() {
                        } else {
                            println!(
                                "{}",
                                theme::error(&format!(
                                    "Command failed with exit code: {:?}",
                                    output.status.code()
                                ))
                            );
                            if !output.stderr.is_empty() {
                                println!("{}", theme::error(output.stderr.trim_end()));
                            }
                        }

                        if ai_interpret {
                            self.interpret_output_final(query, &output.full_output, &summary)
                                .await?;
                        }

                        return Ok(());
                    }
                }
            }
        }

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

        let mut summary = String::new();
        let output = if ai_interpret {
            let (tx, rx) = mpsc::channel();
            let (ack_tx, ack_rx) = mpsc::channel();
            let handle = self.spawn_incremental_interpreter(query, rx, ack_tx);
            let sink = super::OutputSink { tx, ack: ack_rx };
            let result = self.run_shell_command_streaming_with_sink(&cmd, Some(sink))?;
            summary = handle.join().unwrap_or_default();
            result
        } else {
            self.run_shell_command_streaming(&cmd)?
        };
        let _ = output;

        if !output.status.success() {
            println!(
                "{}",
                theme::error(&format!(
                    "Command failed with exit code: {:?}",
                    output.status.code()
                ))
            );
            if !output.stderr.is_empty() {
                println!("{}", theme::error(output.stderr.trim_end()));
            }
        } else {
            last_successful_command = cmd.to_string();
            last_successful_query = query.to_string();

            // Cache successful command
            let candidate = CommandCandidate::new(last_successful_command.clone());
            let _ = self
                .cache_manager
                .save_command_cached(query, vec![candidate]);
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

        if ai_interpret {
            self.interpret_output_final(query, &output.full_output, &summary)
                .await?;
        }

        Ok(())
    }

    async fn execute_or_interpret(&self, query: &str, cmd: &str, ai_interpret: bool) -> Result<()> {
        let mut summary = String::new();
        let output = if ai_interpret {
            let (tx, rx) = mpsc::channel();
            let (ack_tx, ack_rx) = mpsc::channel();
            let handle = self.spawn_incremental_interpreter(query, rx, ack_tx);
            let sink = super::OutputSink { tx, ack: ack_rx };
            let result = self.run_shell_command_streaming_with_sink(cmd, Some(sink))?;
            summary = handle.join().unwrap_or_default();
            result
        } else {
            self.run_shell_command_streaming(cmd)?
        };
        let _ = output;

        if !output.status.success() {
            println!(
                "{}",
                theme::error(&format!(
                    "Command failed with exit code: {:?}",
                    output.status.code()
                ))
            );
            if !output.stderr.is_empty() {
                println!("{}", theme::error(output.stderr.trim_end()));
            }
            if let Some(service) = self.neurosymbolic_service.as_ref() {
                let _ = service.record_failure(
                    query,
                    cmd,
                    FailureType::ExecutionFailed,
                    Some(output.stderr.trim()),
                );
            }
        } else if let Some(service) = self.neurosymbolic_service.as_ref() {
            let _ = service.record_success(query, cmd);
        }

        if ai_interpret {
            self.interpret_output_final(query, &output.full_output, &summary)
                .await?;
        }

        Ok(())
    }

    pub async fn handle_query_with_scaling(
        &mut self,
        query: &str,
        ai_interpret: bool,
        from_fallback: bool,
        scaling_config: &ScalingConfig,
    ) -> Result<()> {
        use application::services::test_time_scaling::ScalingMethod;

        if scaling_config.method != ScalingMethod::None {
            if let Some(best_cmd) = self.select_best_with_scaling(query, scaling_config).await {
                println!(
                    "Selected best command via {}: {}",
                    match scaling_config.method {
                        ScalingMethod::Knockout => "knockout tournament",
                        ScalingMethod::League => "league competition",
                        ScalingMethod::None => "none",
                    },
                    best_cmd
                );

                if ask_confirmation("Run this command?", true)? {
                    let mut summary = String::new();
                    let output = if ai_interpret {
                        let (tx, rx) = mpsc::channel();
                        let (ack_tx, ack_rx) = mpsc::channel();
                        let handle = self.spawn_incremental_interpreter(query, rx, ack_tx);
                        let sink = super::OutputSink { tx, ack: ack_rx };
                        let result =
                            self.run_shell_command_streaming_with_sink(&best_cmd, Some(sink))?;
                        summary = handle.join().unwrap_or_default();
                        result
                    } else {
                        self.run_shell_command_streaming(&best_cmd)?
                    };

                    if output.status.success() {
                        let candidate = CommandCandidate::new(best_cmd.clone());
                        let _ = self
                            .cache_manager
                            .save_command_cached(query, vec![candidate]);
                    }

                    if ai_interpret {
                        self.interpret_output_final(query, &output.full_output, &summary)
                            .await?;
                    }
                }
                return Ok(());
            }
        }

        self.handle_query(query, ai_interpret, from_fallback).await
    }
}
