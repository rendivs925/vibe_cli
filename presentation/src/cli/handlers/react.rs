use super::CliHandlers;
use crate::cli::cache::CommandCandidate;
use crate::cli::command_review::review_candidates;
use anyhow::anyhow;
use application::services::rag_service::RagService;
use application::services::react_agent_service::ReactAgentService;
use application::services::test_time_scaling::{ScalingConfig, ScalingMethod};
use application::services::tool_executor::ToolExecutor;
use crossterm::event::{read, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use domain::entities::react::{
    CommandSafety, ProposedCommand, ReactSession, ReactStatus, ReactStep, ReactStepType, ReactTool,
};
use domain::repositories::react_repository::{ReactCommandRepository, ReactRepository};
use infrastructure::memory::{default_memory_path, lifelong::LifelongMemoryStore};
use infrastructure::ollama_client::OllamaClient;
use infrastructure::react_persistent_storage::SqliteReactStorage;
use infrastructure::react_storage::InMemoryReactStorage;
use infrastructure::session_indexing_service::SessionIndexingService;
use infrastructure::syntax_grammar_validator::SyntaxGrammarValidator;
use infrastructure::tools;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use shared::types::Result;
use std::env;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use termimad::{MadSkin, FmtText};

mod planning;

#[derive(Clone, Copy)]
struct ReactRunOptions {
    allow_next_prompt: bool,
    show_reasoning: bool,
    context_scope: &'static str,
    show_query: bool,
    show_validation: bool,
    show_command: bool,
    auto_run_readonly: bool,
    summary_only: bool,
    auto_summary: bool,
}

impl ReactRunOptions {
    fn single_query(auto_summary: bool) -> Self {
        Self {
            allow_next_prompt: true,
            show_reasoning: true,
            context_scope: "default",
            show_query: true,
            show_validation: true,
            show_command: true,
            auto_run_readonly: false,
            summary_only: false,
            auto_summary,
        }
    }

    fn interactive(auto_summary: bool) -> Self {
        Self {
            allow_next_prompt: false,
            show_reasoning: false,
            context_scope: "default",
            show_query: false,
            show_validation: false,
            show_command: false,
            auto_run_readonly: true,
            summary_only: true,
            auto_summary,
        }
    }
}

struct ReactSeedOutput {
    command: Option<String>,
    output: String,
}

struct ExecutionResult {
    output: String,
    summary: Option<String>,
    output_printed: bool,
}

#[derive(Default, Clone)]
struct ReactSessionCarry {
    last_output: Option<String>,
    last_command: Option<String>,
}

struct LineEditor {
    editor: DefaultEditor,
    history_path: PathBuf,
}

impl LineEditor {
    fn new() -> Result<Self> {
        let mut editor = DefaultEditor::new().map_err(|e| anyhow!(e.to_string()))?;
        let history_path = react_history_path();
        if let Some(parent) = history_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = editor.load_history(&history_path);
        Ok(Self {
            editor,
            history_path,
        })
    }

    fn read_line(&mut self) -> Result<Option<String>> {
        self.read_line_with_prompt(Some("> "))
    }

    fn read_line_with_prompt(&mut self, prompt_msg: Option<&str>) -> Result<Option<String>> {
        let prompt = prompt_msg.unwrap_or("> ");
        match self.editor.readline(prompt) {
            Ok(line) => {
                let trimmed = line.trim_end().to_string();
                if !trimmed.is_empty() {
                    let _ = self.editor.add_history_entry(trimmed.as_str());
                }
                Ok(Some(trimmed))
            }
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => Ok(None),
            Err(err) => Err(anyhow!(err.to_string())),
        }
    }
}

impl Drop for LineEditor {
    fn drop(&mut self) {
        let _ = self.editor.save_history(&self.history_path);
    }
}

impl CliHandlers {
    pub async fn handle_react(
        &mut self,
        query: &str,
        neurosymbolic: bool,
        _ai_interpret: bool,
        scaling_config: &ScalingConfig,
    ) -> Result<()> {
        if query.trim().is_empty() {
            return self
                .handle_react_shell(neurosymbolic, scaling_config)
                .await;
        }

        let interrupted = Arc::new(AtomicBool::new(false));
        install_ctrlc_handler(interrupted.clone())?;
        let mut repl: Option<LineEditor> = None;
        let _ = self
            .run_react_session(
                query,
                neurosymbolic,
                scaling_config,
                ReactRunOptions::single_query(true),
                None,
                &mut repl,
                interrupted,
            )
            .await?;
        Ok(())
    }

    async fn handle_react_shell(
        &mut self,
        neurosymbolic: bool,
        scaling_config: &ScalingConfig,
    ) -> Result<()> {
        let interrupted = Arc::new(AtomicBool::new(false));
        install_ctrlc_handler(interrupted.clone())?;
        let mut carry = ReactSessionCarry::default();
        let mut repl = match LineEditor::new() {
            Ok(repl) => Some(repl),
            Err(err) => {
                eprintln!("[warn] Failed to start line editor: {err}");
                None
            }
        };

        loop {
            interrupted.store(false, Ordering::SeqCst);
            let input = match read_shell_line(&mut repl, &interrupted)? {
                Some(value) => value,
                None => {
                    println!();
                    break;
                }
            };
            let trimmed = input.trim();
            if trimmed.is_empty() {
                continue;
            }
            match trimmed {
                "/exit" | "/quit" => break,
                "/help" => {
                    println!("Commands: /help, /exit, /quit");
                    continue;
                }
                _ => {}
            }

            let follow_up = is_follow_up_request(trimmed) && carry.last_output.is_some();
            let seed = if follow_up {
                Some(ReactSeedOutput {
                    command: carry.last_command.clone(),
                    output: carry.last_output.clone().unwrap_or_default(),
                })
            } else {
                None
            };

            interrupted.store(false, Ordering::SeqCst);
            carry = self
                .run_react_session(
                    trimmed,
                    neurosymbolic,
                    scaling_config,
                    ReactRunOptions::interactive(true),
                    seed,
                    &mut repl,
                    interrupted.clone(),
                )
                .await?;
        }

        Ok(())
    }

    async fn run_react_session(
        &mut self,
        query: &str,
        neurosymbolic: bool,
        scaling_config: &ScalingConfig,
        options: ReactRunOptions,
        seed: Option<ReactSeedOutput>,
        repl: &mut Option<LineEditor>,
        interrupted: Arc<AtomicBool>,
    ) -> Result<ReactSessionCarry> {
        let (react_repo, cmd_repo) = init_react_storage();
        let react_repo_for_cli = react_repo.clone();
        let neurosymbolic_service = if neurosymbolic {
            Some(Arc::new(
                application::services::neurosymbolic_service::NeurosymbolicService::new()?,
            ))
        } else {
            None
        };

        let service = ReactAgentService::new(neurosymbolic_service, react_repo, cmd_repo)?;

        // Enable semantic indexing for cross-session learning
        let service = match SessionIndexingService::new().await {
            Ok(indexing) => service.with_indexing_service(Arc::new(indexing)),
            Err(e) => {
                eprintln!("[warn] Failed to initialize semantic indexing: {}", e);
                service
            }
        };

        let mut session = service
            .start_session(query.to_string(), neurosymbolic)
            .await?;
        session.context.insert(
            "context_scope".to_string(),
            options.context_scope.to_string(),
        );

        if let Some(seed_output) = seed {
            let content = if let Some(cmd) = seed_output.command.as_ref() {
                format!("Command: {}\nOutput:\n{}", cmd.trim(), seed_output.output)
            } else {
                format!("Output:\n{}", seed_output.output)
            };
            let step_index = session.steps.len();
            save_step(&service, &mut session, ReactStepType::Observation, content).await?;
            if let Some(cmd) = seed_output.command.as_ref() {
                service.ingest_observation(&mut session, cmd, &seed_output.output, step_index);
            }
        }

        let mut validator = SyntaxGrammarValidator::new();
        let tools = build_default_tool_executor();

        if options.show_query {
            println!("→ {}", session.query);
        }

        let mut pending_command_override: Option<String> = None;
        let mut requested_primary = extract_user_command_override(&session.query)
            .and_then(|cmd| primary_command_binary(&cmd));
        let mut carry = ReactSessionCarry::default();

        while matches!(session.status, ReactStatus::Running) {
            if repl.is_none() && interrupted.load(Ordering::SeqCst) {
                session.abort();
                service.save_session(&session).await?;
                println!("\n[Session auto-saved]");
                return Ok(carry);
            }

            let reasoning = service.generate_reasoning(&session).await?;
            service.ingest_reasoning(&mut session, &reasoning);
            if options.show_reasoning {
                print_section("ANALYZE", &reasoning);
            }
            save_step(
                &service,
                &mut session,
                ReactStepType::Thought,
                reasoning.clone(),
            )
            .await?;

            // Dynamic Tool Selection
            let mut tool_name = "unknown".to_string();
            let mut tool_justification = "Tool selection failed".to_string();

            let tool_result = match service.select_tool(&session, &reasoning).await {
                Ok(tool_decision) => {
                    tool_name = tool_decision.tool.name().to_string();
                    tool_justification = tool_decision.justification.clone();

                    // Execute the selected tool
                    match service
                        .execute_tool(tool_decision.tool, &session, &reasoning)
                        .await
                    {
                        Ok(result) => {
                            if !options.summary_only && !result.output.is_empty() {
                                print_with_pager(&result.output);
                            }
                            Some(result)
                        }
                        Err(e) => {
                            eprintln!("[warn] Tool execution failed: {}", e);
                            None
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[warn] Tool selection failed: {}", e);
                    None
                }
            };

            // If tool failed, collect user direction instead of falling back to command generation
            let tool_result = match tool_result {
                Some(r) => r,
                None => {
                    eprintln!("[warn] No tool result available. Provide direction or /abort.");
                    let input = match read_user_line("> ", &interrupted, repl)? {
                        Some(value) => value,
                        None => {
                            session.abort();
                            service.save_session(&session).await?;
                            println!("\n[Session auto-saved]");
                            return Ok(carry);
                        }
                    };
                    if input.trim().is_empty() {
                        continue;
                    }
                    service.ingest_user_input(&mut session, &input);
                    save_step(&service, &mut session, ReactStepType::Observation, input).await?;
                    continue;
                }
            };

            if tool_result.should_ask_user {
                let input = match read_user_line("> ", &interrupted, repl)? {
                    Some(value) => value,
                    None => {
                        session.abort();
                        service.save_session(&session).await?;
                        println!("\n[Session auto-saved]");
                        return Ok(carry);
                    }
                };
                if input.trim().is_empty() {
                    continue;
                }
                service.ingest_user_input(&mut session, &input);
                save_step(&service, &mut session, ReactStepType::Observation, input).await?;
                continue;
            }

            if !tool_result.should_continue {
                if !tool_result.output.trim().is_empty() {
                    let summary = summarize_output_for_observation(&tool_result.output);
                    let step_index = session.steps.len();
                    service.ingest_observation(
                        &mut session,
                        tool_result.tool.name(),
                        &tool_result.output,
                        step_index,
                    );
                    save_step(&service, &mut session, ReactStepType::Observation, summary).await?;
                    if options.auto_summary && !tool_result.output.trim().is_empty() {
                        let previous = session.compacted_summary.as_deref().unwrap_or("");
                        let _ = self
                            .interpret_output_final(&session.query, &tool_result.output, previous)
                            .await;
                    }
                }
                match tool_result.tool {
                    ReactTool::ConcludeFail | ReactTool::Escalate | ReactTool::Defer => {
                        session.fail();
                    }
                    _ => session.complete(),
                }
                service.save_session(&session).await?;
                break;
            }

            if tool_result.commands.is_empty() && !tool_result.output.trim().is_empty() {
                let summary = summarize_output_for_observation(&tool_result.output);
                let step_index = session.steps.len();
                service.ingest_observation(
                    &mut session,
                    tool_result.tool.name(),
                    &tool_result.output,
                    step_index,
                );
                save_step(&service, &mut session, ReactStepType::Observation, summary).await?;
                if options.auto_summary && !tool_result.output.trim().is_empty() {
                    let previous = session.compacted_summary.as_deref().unwrap_or("");
                    let _ = self
                        .interpret_output_final(&session.query, &tool_result.output, previous)
                        .await;
                    session.complete();
                    service.save_session(&session).await?;
                    break;
                }
                service.save_session(&session).await?;
                continue;
            }

            // Get commands from tool result
            let mut commands = if let Some(command_override) = pending_command_override.take() {
                vec![ProposedCommand::new(
                    command_override,
                    "User-directed command".to_string(),
                    "User asked to run a specific command".to_string(),
                )]
            } else if should_start_with_structure_discovery(&session.query, &session) {
                vec![ProposedCommand::new(
                    build_exploration_seed_command(&session.query),
                    "Project structure discovery".to_string(),
                    "Prefer RAG and AST for codebase exploration".to_string(),
                )]
            } else if !tool_result.commands.is_empty() {
                // Use commands from tool result
                tool_result
                    .commands
                    .into_iter()
                    .map(|cmd| {
                        ProposedCommand::new(
                            cmd,
                            format!("Tool proposed: {}", tool_name),
                            tool_justification.clone(),
                        )
                    })
                    .collect()
            } else {
                service.propose_commands(&reasoning, &session).await?
            };
            for command in &mut commands {
                let adjusted = adjust_command_for_query(&command.command, &session.query);
                if adjusted != command.command {
                    command.command = adjusted;
                }
            }
            if commands.is_empty() {
                println!("No command suggestion generated.");
                break;
            }

            if scaling_config.method != ScalingMethod::None && commands.len() > 1 {
                if let Some(best_cmd) = self
                    .select_best_with_scaling(&session.query, scaling_config)
                    .await
                {
                    println!("[Scaling selected: {}]", best_cmd);
                    commands = vec![ProposedCommand::new(
                        best_cmd,
                        "Scaling selected command".to_string(),
                        "test-time scaling".to_string(),
                    )];
                }
            }

            let validation = validate_command_candidates(commands, &tools);
            print_validation_report(&validation, options.show_validation);
            let mut suggested = if let Some(valid) = validation.valid.into_iter().next() {
                valid
            } else {
                if options.show_validation {
                    println!("No valid commands found.");
                }
                break;
            };
            if options.show_command
                || (!options.auto_run_readonly
                    || !matches!(suggested.safety, CommandSafety::ReadOnly))
            {
                println!("→ {}", suggested.command);
            }

            let mut action_step = ReactStep::new(
                session.id.clone(),
                ReactStepType::Action,
                suggested.command.clone(),
            );
            action_step.add_command(suggested.clone());
            action_step.start();
            action_step.complete();
            service.save_step(&action_step).await?;
            session.add_step(action_step);
            service.save_command(&suggested).await?;
            service.save_session(&session).await?;

            let decision = if options.auto_run_readonly
                && matches!(suggested.safety, CommandSafety::ReadOnly)
            {
                AllowDecision::Execute
            } else {
                loop {
                    let prompt = safety_prompt(&suggested.safety);
                    let input = match read_user_line(prompt, &interrupted, repl)? {
                        Some(value) => value,
                        None => {
                            session.abort();
                            service.save_session(&session).await?;
                            println!("\n[Session auto-saved]");
                            return Ok(carry);
                        }
                    };
                    match parse_allow_input(&input, suggested.safety)? {
                        AllowDecision::PromptForDirection => {
                            let extra = match read_user_line("> ", &interrupted, repl)? {
                                Some(value) => value,
                                None => {
                                    session.abort();
                                    service.save_session(&session).await?;
                                    println!("\n[Session auto-saved]");
                                    return Ok(carry);
                                }
                            };
                            if extra.trim().is_empty() {
                                continue;
                            }
                            break AllowDecision::Direction(extra);
                        }
                        other => break other,
                    }
                }
            };

            match decision {
                AllowDecision::Execute => {
                    let execution = execute_suggestion(
                        self,
                        &tools,
                        &mut validator,
                        &mut suggested,
                        &session.query,
                        !options.summary_only || options.auto_summary,
                        options.auto_summary,
                    )
                    .await?;
                    let output = execution.output.clone();
                    carry.last_output = Some(output.clone());
                    carry.last_command = Some(suggested.command.clone());
                    let step_index = session.steps.len();
                    service.ingest_observation(
                        &mut session,
                        &suggested.command,
                        &output,
                        step_index,
                    );
                    save_step(
                        &service,
                        &mut session,
                        ReactStepType::Observation,
                        summarize_output_for_observation(&output),
                    )
                    .await?;
                    service.update_command(&suggested).await.ok();
                    service.record_command_outcome(&session.query, &suggested);
                    // Index command for cross-session learning
                    let _ = service
                        .index_command_execution(&suggested, &session.id)
                        .await;

                    if !execution.output_printed && !output.trim().is_empty() {
                        print_with_pager(&output);
                    }

                    if options.auto_summary && !output.trim().is_empty() {
                        let previous = execution
                            .summary
                            .as_deref()
                            .or(session.compacted_summary.as_deref())
                            .unwrap_or("");
                        let _ = self
                            .interpret_output_final(&session.query, &output, previous)
                            .await;
                    }

                    if options.summary_only {
                        session.complete();
                        service.save_session(&session).await?;
                        break;
                    }

                    if suggested.exit_code == Some(0) && !output.trim().is_empty() {
                        let mut achieved = false;
                        if let Some(requested) = requested_primary.as_ref() {
                            if primary_command_binary(&suggested.command)
                                .as_ref()
                                .map(|cmd| cmd == requested)
                                .unwrap_or(false)
                            {
                                achieved = true;
                            }
                        }

                        if !achieved {
                            match service.is_goal_achieved(&session).await {
                                Ok(true) => achieved = true,
                                Ok(false) => {}
                                Err(e) => {
                                    eprintln!("[warn] Goal check failed: {}", e);
                                }
                            }
                        }

                        if achieved {
                            if options.allow_next_prompt {
                                let next = prompt_next_goal(&interrupted)?;
                                session.complete();
                                service.save_session(&session).await?;
                                if let Some(next_query) = next {
                                    let follow_up = is_follow_up_request(&next_query)
                                        && carry.last_output.is_some();
                                    session =
                                        service.start_session(next_query, neurosymbolic).await?;
                                    session.context.insert(
                                        "context_scope".to_string(),
                                        options.context_scope.to_string(),
                                    );
                                    requested_primary =
                                        extract_user_command_override(&session.query)
                                            .and_then(|cmd| primary_command_binary(&cmd));
                                    if follow_up {
                                        if let Some(output) = carry.last_output.as_ref() {
                                            let seed =
                                                if let Some(cmd) = carry.last_command.as_ref() {
                                                    format!(
                                                        "Command: {}\nOutput:\n{}",
                                                        cmd.trim(),
                                                        output
                                                    )
                                                } else {
                                                    format!("Output:\n{}", output)
                                                };
                                            let step_index = session.steps.len();
                                            save_step(
                                                &service,
                                                &mut session,
                                                ReactStepType::Observation,
                                                seed,
                                            )
                                            .await?;
                                            if let Some(cmd) = carry.last_command.as_ref() {
                                                service.ingest_observation(
                                                    &mut session,
                                                    cmd,
                                                    output,
                                                    step_index,
                                                );
                                            }
                                        }
                                    }
                                    pending_command_override = None;
                                    println!("→ {}", session.query);
                                    continue;
                                } else {
                                    println!("\n→ Session ended.");
                                    break;
                                }
                            } else {
                                session.complete();
                                service.save_session(&session).await?;
                                break;
                            }
                        }
                    }
                }
                AllowDecision::Skip => {
                    suggested.reject();
                    service.update_command(&suggested).await.ok();
                    save_step(
                        &service,
                        &mut session,
                        ReactStepType::Observation,
                        "Skipped current suggestion.".to_string(),
                    )
                    .await?;
                }
                AllowDecision::Direction(text) => {
                    suggested.reject();
                    service.update_command(&suggested).await.ok();
                    // User provided direction - go back to ANALYZE with their input
                    // Do NOT generate new command suggestion - re-analyze with user input
                    service.ingest_user_input(&mut session, &text);
                    save_step(&service, &mut session, ReactStepType::Observation, text).await?;
                    continue; // Skip command generation, go back to ANALYZE
                }
                AllowDecision::SessionCommand(command) => {
                    match command {
                        SessionCommand::Compact => {
                            let summary = service.compact_history(&session).await?;
                            session.set_compacted_summary(summary.clone());
                            println!("Compacting steps into summary:");
                            println!("{summary}");
                        }
                        SessionCommand::Reset => {
                            service.reset_memory(&mut session);
                            println!("Cleared facts and hypotheses.");
                        }
                        SessionCommand::Plan => {
                            let plan = service.generate_plan(&session.query);
                            let formatted = planning::format_plan(&plan);
                            println!("{formatted}");
                        }
                        SessionCommand::Memory(query) => {
                            if query.trim().is_empty() {
                                println!("Usage: /memory <query>");
                            } else {
                                let store = LifelongMemoryStore::new(default_memory_path())
                                    .map_err(|e| anyhow!(e.to_string()))?;
                                let results = store
                                    .search(&query, 5)
                                    .map_err(|e| anyhow!(e.to_string()))?;
                                if results.is_empty() {
                                    println!("No memory matches.");
                                } else {
                                    for entry in results {
                                        println!("{}: {}", entry.id, entry.content);
                                    }
                                }
                            }
                        }
                        SessionCommand::Remember(text) => {
                            if text.trim().is_empty() {
                                println!("Usage: /remember <fact>");
                            } else {
                                let store = LifelongMemoryStore::new(default_memory_path())
                                    .map_err(|e| anyhow!(e.to_string()))?;
                                let id =
                                    store.remember(&text).map_err(|e| anyhow!(e.to_string()))?;
                                println!("Stored memory id {id}");
                            }
                        }
                        SessionCommand::Forget(text) => {
                            if text.trim().is_empty() {
                                println!("Usage: /forget <text or id>");
                            } else {
                                let store = LifelongMemoryStore::new(default_memory_path())
                                    .map_err(|e| anyhow!(e.to_string()))?;
                                let count =
                                    store.forget(&text).map_err(|e| anyhow!(e.to_string()))?;
                                println!("Removed {count} entries");
                            }
                        }
                        SessionCommand::Autonomy(mode) => {
                            if mode.trim().is_empty() {
                                println!("Usage: /autonomy <manual|guided|auto>");
                            } else {
                                session.context.insert("autonomy".to_string(), mode.clone());
                                println!("Autonomy set to {mode}");
                            }
                        }
                        SessionCommand::Save => {
                            service.save_session(&session).await?;
                            println!("Session saved.");
                        }
                        SessionCommand::Sessions => {
                            let sessions = react_repo_for_cli
                                .get_recent_sessions(10)
                                .await
                                .map_err(|e| anyhow!(e.to_string()))?;
                            if sessions.is_empty() {
                                println!("No saved sessions.");
                            } else {
                                for s in sessions {
                                    println!("{} | {:?} | {}", s.id, s.status, s.query);
                                }
                            }
                        }
                        SessionCommand::Resume(id) => {
                            let target = if let Some(id) = id {
                                id
                            } else {
                                let sessions = react_repo_for_cli
                                    .get_recent_sessions(1)
                                    .await
                                    .map_err(|e| anyhow!(e.to_string()))?;
                                sessions.first().map(|s| s.id.clone()).unwrap_or_default()
                            };
                            if target.is_empty() {
                                println!("No session to resume.");
                            } else if let Some(mut loaded) = react_repo_for_cli
                                .get_session(&target)
                                .await
                                .map_err(|e| anyhow!(e.to_string()))?
                            {
                                let steps = react_repo_for_cli
                                    .get_steps(&loaded.id)
                                    .await
                                    .map_err(|e| anyhow!(e.to_string()))?;
                                loaded.steps = steps;
                                loaded.status = ReactStatus::Running;
                                session = loaded;
                                pending_command_override = None;
                                println!("Resumed session {}", session.id);
                            } else {
                                println!("Session not found: {target}");
                            }
                        }
                        SessionCommand::Stats => {
                            print_session_stats(&session);
                        }
                        _ => {
                            if handle_session_command(command, &mut session)? {
                                service.save_session(&session).await?;
                                return Ok(carry);
                            }
                        }
                    }
                    service.save_session(&session).await?;
                    continue;
                }
                AllowDecision::PromptForDirection => continue,
            }

            // Loop continues until user exits with /abort or Ctrl+C
        }

        // User exited the loop
        if matches!(session.status, ReactStatus::Running) {
            session.fail();
            service.save_session(&session).await?;
        }

        if options.allow_next_prompt {
            println!("\n→ Session ended.");
        }
        Ok(carry)
    }
}

#[derive(Debug)]
enum SessionCommand {
    Help,
    Context,
    Facts,
    Hypotheses,
    Plan,
    Memory(String),
    Remember(String),
    Forget(String),
    Autonomy(String),
    Save,
    Resume(Option<String>),
    Sessions,
    Stats,
    Compact,
    Reset,
    Skip,
    Abort,
}

#[derive(Debug)]
enum AllowDecision {
    Execute,
    Skip,
    Direction(String),
    PromptForDirection,
    SessionCommand(SessionCommand),
}

async fn save_step(
    service: &ReactAgentService,
    session: &mut ReactSession,
    step_type: ReactStepType,
    content: String,
) -> Result<()> {
    let mut step = ReactStep::new(session.id.clone(), step_type, content);
    step.start();
    step.complete();
    service.save_step(&step).await?;
    session.add_step(step);
    service.save_session(session).await
}

fn print_section(_title: &str, content: &str) {
    if !content.trim().is_empty() {
        ensure_fresh_line();
        print_markdown(content);
        let _ = io::stdout().flush();
    }
}

fn print_react_context(session: &ReactSession) {
    println!("Goal: {}", session.query);
    println!("Steps: {}", session.steps.len());
    if let Some(summary) = &session.compacted_summary {
        if !summary.trim().is_empty() {
            println!("Summary: {}", summary.trim());
        }
    }
    if !session.memory.constraints.is_empty() {
        let constraints = session
            .memory
            .constraints
            .iter()
            .map(|c| format!("{}={}", c.key, c.value))
            .collect::<Vec<_>>()
            .join(", ");
        println!("Constraints: {}", constraints);
    }
    for step in session.steps.iter().rev().take(6).rev() {
        let label = match step.step_type {
            ReactStepType::Thought => "ANALYZE",
            ReactStepType::Action => "SUGGESTED",
            ReactStepType::Observation => "OUTPUT",
            ReactStepType::Verify => "VERIFY",
            ReactStepType::Complete => "COMPLETE",
        };
        println!("- {label}: {}", step.content);
    }
}

fn print_react_facts(session: &ReactSession) {
    if session.memory.facts.is_empty() {
        println!("No facts extracted yet.");
        return;
    }
    println!("Extracted facts:");
    for fact in &session.memory.facts {
        println!(
            "- {}={} (step {}, source: {})",
            fact.key, fact.value, fact.source_step, fact.source_command
        );
    }
}

fn print_react_hypotheses(session: &ReactSession) {
    if session.memory.hypotheses.is_empty() {
        println!("No hypotheses recorded yet.");
        return;
    }
    println!("Current hypotheses:");
    for hypothesis in &session.memory.hypotheses {
        println!(
            "- {} (confidence {:.0}%)",
            hypothesis.description,
            hypothesis.confidence * 100.0
        );
    }
}

fn print_session_stats(session: &ReactSession) {
    println!("Session ID: {}", session.id);
    println!("Status: {:?}", session.status);
    println!("Steps: {}", session.steps.len());
    println!("Facts: {}", session.memory.facts.len());
    println!("Hypotheses: {}", session.memory.hypotheses.len());
    println!("Constraints: {}", session.memory.constraints.len());
    println!("Created: {}", session.created_at);
    println!("Updated: {}", session.updated_at);
}

fn parse_allow_input(input: &str, safety: CommandSafety) -> Result<AllowDecision> {
    let trimmed = input.trim();
    if trimmed.starts_with('/') {
        return Ok(AllowDecision::SessionCommand(parse_session_command(
            trimmed,
        )?));
    }

    let allow_empty_execute = matches!(safety, CommandSafety::ReadOnly);
    match trimmed.to_lowercase().as_str() {
        "" if allow_empty_execute => Ok(AllowDecision::Execute),
        "" => Ok(AllowDecision::PromptForDirection),
        "y" | "yes" => Ok(AllowDecision::Execute),
        "skip" => Ok(AllowDecision::Skip),
        "n" | "no" => Ok(AllowDecision::PromptForDirection),
        _ => Ok(AllowDecision::Direction(trimmed.to_string())),
    }
}

fn safety_prompt(safety: &CommandSafety) -> &'static str {
    match safety {
        CommandSafety::ReadOnly => "Run? [y/n] ",
        CommandSafety::Write => "This modifies files. Run? [y/n] ",
        CommandSafety::Destructive => "⚠️ System change. Sure? [y/n] ",
    }
}

struct CommandValidationReport {
    valid: Vec<ProposedCommand>,
    invalid: Vec<InvalidCommand>,
}

struct InvalidCommand {
    command: String,
    reason: String,
}

fn validate_command_candidates(
    commands: Vec<ProposedCommand>,
    tools: &application::services::tool_executor::ToolExecutor,
) -> CommandValidationReport {
    let mut valid = Vec::new();
    let mut invalid = Vec::new();

    for command in commands {
        match validate_command_line(&command.command, tools) {
            Ok(()) => valid.push(command),
            Err(reason) => invalid.push(InvalidCommand {
                command: command.command.clone(),
                reason,
            }),
        }
    }

    CommandValidationReport { valid, invalid }
}

fn print_validation_report(report: &CommandValidationReport, show: bool) {
    if !show {
        if report.valid.is_empty() && !report.invalid.is_empty() {
            println!("No valid commands found: {}", report.invalid[0].reason);
        }
        return;
    }
    let total = report.valid.len() + report.invalid.len();
    if total == 0 {
        return;
    }
    if report.invalid.is_empty() {
        println!("Command Validation: {}/{} valid", report.valid.len(), total);
        return;
    }

    println!("Command Validation: {}/{} valid", report.valid.len(), total);
    println!("Invalid commands:");
    for entry in &report.invalid {
        println!("  - {}: {}", entry.command, entry.reason);
    }
}

fn validate_command_line(
    command_line: &str,
    tools: &application::services::tool_executor::ToolExecutor,
) -> std::result::Result<(), String> {
    let tokens = parse_command_tokens(command_line);
    if tokens.is_empty() {
        return Err("Empty command".to_string());
    }

    let tool_name = tokens[0].clone();
    if tools.has_tool(&tool_name) {
        if tool_name == "shell" {
            let inner = tokens[1..].join(" ");
            return validate_shell_command(&inner);
        }
        return Ok(());
    }

    validate_shell_command(command_line)
}

fn validate_shell_command(command_line: &str) -> std::result::Result<(), String> {
    let syntax = Command::new("bash")
        .args(["-n", "-c", command_line])
        .output()
        .map_err(|e| format!("Syntax check failed: {}", e))?;
    if !syntax.status.success() {
        let reason = String::from_utf8_lossy(&syntax.stderr).trim().to_string();
        let message = if reason.is_empty() {
            "Syntax error".to_string()
        } else {
            format!("Syntax error: {}", reason)
        };
        return Err(message);
    }

    let binaries = extract_binaries(command_line);
    for bin in binaries {
        if !command_exists(&bin) {
            return Err(format!(
                "Command not found: '{}' (try: install {})",
                bin, bin
            ));
        }
    }

    Ok(())
}

fn extract_binaries(command_line: &str) -> Vec<String> {
    let tokens = parse_command_tokens(command_line);
    let mut binaries = Vec::new();
    let mut expect_command = true;
    let mut skip_next = false;

    for token in tokens {
        if skip_next {
            skip_next = false;
            continue;
        }

        if is_separator_token(&token) {
            expect_command = true;
            continue;
        }

        if is_redirection_token(&token) {
            if token == ">" || token == ">>" || token == "<" || token == "2>" || token == "2>>" {
                skip_next = true;
            }
            continue;
        }

        if expect_command {
            if token == "sudo" {
                binaries.push(token);
                expect_command = true;
                continue;
            }
            if is_env_assignment(&token) {
                continue;
            }
            binaries.push(token);
            expect_command = false;
            continue;
        }
    }

    binaries.sort();
    binaries.dedup();
    binaries
}

fn is_separator_token(token: &str) -> bool {
    matches!(token, "|" | "||" | "&&" | ";")
}

fn is_redirection_token(token: &str) -> bool {
    token.starts_with('>')
        || token.starts_with('<')
        || token == "1>"
        || token == "1>>"
        || token == "2>"
        || token == "2>>"
        || token == "&>"
        || token == "2>&1"
}

fn is_env_assignment(token: &str) -> bool {
    let mut split = token.splitn(2, '=');
    let Some(key) = split.next() else {
        return false;
    };
    let Some(_) = split.next() else { return false };
    if key.is_empty() {
        return false;
    }
    let mut chars = key.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn command_exists(binary: &str) -> bool {
    Command::new("bash")
        .args(["-lc", &format!("command -v {} >/dev/null 2>&1", binary)])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn should_start_with_structure_discovery(query: &str, session: &ReactSession) -> bool {
    let query_lower = query.to_lowercase();
    let is_explain_task = query_lower.contains("explain")
        || query_lower.contains("understand")
        || query_lower.contains("overview")
        || query_lower.contains("this project");
    if !is_explain_task {
        return false;
    }
    !session
        .steps
        .iter()
        .any(|step| matches!(step.step_type, ReactStepType::Action))
}

fn extract_user_command_override(input: &str) -> Option<String> {
    let mut text = input.trim();
    if text.is_empty() {
        return None;
    }

    for prefix in ["try ", "run ", "use ", "execute "] {
        if let Some(rest) = text.strip_prefix(prefix) {
            text = rest.trim();
            break;
        }
    }

    let first = text.split_whitespace().next()?.to_lowercase();
    let known_tools = [
        "read",
        "grep",
        "fd",
        "rag",
        "web_search",
        "web_fetch",
        "web_summarize",
        "web_extract",
        "read_pdf",
        "read_docx",
        "read_xlsx",
        "extract_tables",
        "doc_qa",
        "semantic_search",
        "grep_context",
        "search_memory",
        "find_patterns",
        "remember",
        "recall",
        "consolidate",
        "learn_patterns",
        "sed",
        "perl",
        "awk",
        "apply_patch",
        "write",
        "remove",
        "update",
        "replace_block",
        "shell",
        "pkg",
        "svc",
        "git",
        "build",
        "test",
    ];
    if known_tools.contains(&first.as_str()) {
        return Some(text.to_string());
    }

    if is_likely_shell_command(text) {
        return Some(format!("shell {text}"));
    }
    None
}

fn is_likely_shell_command(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.contains('?') {
        return false;
    }

    let first = trimmed.split_whitespace().next().unwrap_or_default();
    if first.is_empty() {
        return false;
    }

    let common_shell = [
        "ls",
        "pwd",
        "cat",
        "head",
        "tail",
        "grep",
        "find",
        "tree",
        "rg",
        "fd",
        "sed",
        "awk",
        "perl",
        "git",
        "cargo",
        "systemctl",
        "service",
        "ps",
        "top",
        "free",
        "df",
        "du",
        "echo",
        "curl",
        "wget",
    ];
    if common_shell.contains(&first) {
        return true;
    }

    Command::new("bash")
        .args(["-lc", &format!("command -v {} >/dev/null 2>&1", first)])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn adjust_command_for_query(command: &str, query: &str) -> String {
    let lower_query = query.to_lowercase();
    if !(lower_query.contains("no pager") || lower_query.contains("no-pager")) {
        return command.to_string();
    }
    let mut tokens = parse_command_tokens(command);
    if tokens.is_empty() {
        return command.to_string();
    }
    let offset = if tokens.get(0).map(|t| t.as_str()) == Some("shell") {
        1
    } else {
        0
    };
    if tokens.len() <= offset {
        return command.to_string();
    }
    if tokens[offset] != "journalctl" {
        return command.to_string();
    }
    if tokens.iter().any(|t| t == "--no-pager") {
        return command.to_string();
    }
    tokens.push("--no-pager".to_string());
    tokens.join(" ")
}

fn is_follow_up_request(input: &str) -> bool {
    let lower = input.trim().to_lowercase();
    if lower.is_empty() {
        return false;
    }
    let follow_starts = [
        "how",
        "why",
        "what about",
        "explain",
        "details",
        "fix",
        "resolve",
        "next",
    ];
    if follow_starts.iter().any(|p| lower.starts_with(p)) {
        return true;
    }
    let pronouns = ["it", "that", "this", "they", "those", "these", "there"];
    if lower
        .split_whitespace()
        .any(|word| pronouns.contains(&word))
    {
        return true;
    }
    lower.split_whitespace().count() <= 3
}

fn build_default_tool_executor() -> ToolExecutor {
    let mut executor = ToolExecutor::new();
    executor.register(Arc::new(tools::exploration::read_tool::ReadTool));
    executor.register(Arc::new(tools::exploration::grep_tool::GrepTool));
    executor.register(Arc::new(tools::exploration::fd_tool::FdTool));
    executor.register(Arc::new(tools::exploration::rag_tool::RagTool));
    executor.register(Arc::new(tools::web::search::WebSearchTool));
    executor.register(Arc::new(tools::web::fetch::WebFetchTool));
    executor.register(Arc::new(tools::web::summarize::WebSummarizeTool));
    executor.register(Arc::new(tools::web::extract::WebExtractTool));
    executor.register(Arc::new(tools::documents::pdf::ReadPdfTool));
    executor.register(Arc::new(tools::documents::docx::ReadDocxTool));
    executor.register(Arc::new(tools::documents::xlsx::ReadXlsxTool));
    executor.register(Arc::new(tools::documents::tables::ExtractTablesTool));
    executor.register(Arc::new(tools::documents::qa::DocQaTool));
    executor.register(Arc::new(tools::search::semantic_search::SemanticSearchTool));
    executor.register(Arc::new(tools::search::grep_context::GrepContextTool));
    executor.register(Arc::new(tools::search::find_patterns::FindPatternsTool));
    executor.register(Arc::new(tools::memory::remember::RememberTool));
    executor.register(Arc::new(tools::memory::recall::RecallTool));
    executor.register(Arc::new(tools::memory::consolidate::ConsolidateTool));
    executor.register(Arc::new(tools::memory::search_memory::SearchMemoryTool));
    executor.register(Arc::new(tools::memory::learn_patterns::LearnPatternsTool));
    executor.register(Arc::new(tools::editing::sed_tool::SedTool));
    executor.register(Arc::new(tools::editing::perl_tool::PerlTool));
    executor.register(Arc::new(tools::editing::awk_tool::AwkTool));
    executor.register(Arc::new(tools::editing::apply_patch_tool::ApplyPatchTool));
    executor.register(Arc::new(tools::file_ops::write_tool::WriteTool));
    executor.register(Arc::new(tools::file_ops::remove_tool::RemoveTool));
    executor.register(Arc::new(tools::file_ops::update_tool::UpdateTool));
    executor.register(Arc::new(
        tools::file_ops::replace_block_tool::ReplaceBlockTool,
    ));
    executor.register(Arc::new(tools::system::shell_tool::ShellTool));
    executor.register(Arc::new(tools::system::pkg_tool::PkgTool));
    executor.register(Arc::new(tools::system::svc_tool::SvcTool));
    executor.register(Arc::new(tools::system::git_tool::GitTool));
    executor.register(Arc::new(tools::system::build_tool::BuildTool));
    executor.register(Arc::new(tools::system::test_tool::TestTool));
    executor
}

fn init_react_storage() -> (Arc<dyn ReactRepository>, Arc<dyn ReactCommandRepository>) {
    let db_path = default_memory_path();
    if let Some(parent) = db_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let db_path_str = db_path.to_string_lossy().to_string();
    match SqliteReactStorage::new(&db_path_str) {
        Ok(storage) => {
            let storage = Arc::new(storage);
            (
                storage.clone() as Arc<dyn ReactRepository>,
                storage as Arc<dyn ReactCommandRepository>,
            )
        }
        Err(err) => {
            eprintln!(
                "[warn] Failed to open persistent storage: {err}. Falling back to in-memory."
            );
            let storage = Arc::new(InMemoryReactStorage::new());
            (
                storage.clone() as Arc<dyn ReactRepository>,
                storage as Arc<dyn ReactCommandRepository>,
            )
        }
    }
}

fn install_ctrlc_handler(flag: Arc<AtomicBool>) -> Result<()> {
    if let Err(err) = ctrlc::set_handler(move || {
        flag.store(true, Ordering::SeqCst);
    }) {
        eprintln!("[warn] Ctrl+C handler not installed: {}", err);
    }
    Ok(())
}

fn parse_session_command(input: &str) -> Result<SessionCommand> {
    let trimmed = input.trim();
    let mut parts = trimmed.splitn(2, ' ');
    let command = parts.next().unwrap_or("");
    let rest = parts.next().unwrap_or("").trim();

    match command {
        "/help" => Ok(SessionCommand::Help),
        "/context" => Ok(SessionCommand::Context),
        "/facts" => Ok(SessionCommand::Facts),
        "/hypotheses" => Ok(SessionCommand::Hypotheses),
        "/plan" => Ok(SessionCommand::Plan),
        "/memory" => Ok(SessionCommand::Memory(rest.to_string())),
        "/remember" => Ok(SessionCommand::Remember(rest.to_string())),
        "/forget" => Ok(SessionCommand::Forget(rest.to_string())),
        "/autonomy" => Ok(SessionCommand::Autonomy(rest.to_string())),
        "/save" => Ok(SessionCommand::Save),
        "/resume" => {
            if rest.is_empty() {
                Ok(SessionCommand::Resume(None))
            } else {
                Ok(SessionCommand::Resume(Some(rest.to_string())))
            }
        }
        "/sessions" => Ok(SessionCommand::Sessions),
        "/stats" => Ok(SessionCommand::Stats),
        "/compact" => Ok(SessionCommand::Compact),
        "/reset" => Ok(SessionCommand::Reset),
        "/skip" => Ok(SessionCommand::Skip),
        "/abort" => Ok(SessionCommand::Abort),
        _ => Err(anyhow!("Unknown command. Use /help")),
    }
}

fn handle_session_command(command: SessionCommand, session: &mut ReactSession) -> Result<bool> {
    match command {
        SessionCommand::Help => {
            println!("Commands: /help, /context, /facts, /hypotheses, /plan, /memory <query>, /remember <fact>, /forget <text>, /autonomy <mode>, /save, /resume [id], /sessions, /stats, /compact, /reset, /skip, /abort");
            Ok(false)
        }
        SessionCommand::Context => {
            print_react_context(session);
            Ok(false)
        }
        SessionCommand::Facts => {
            print_react_facts(session);
            Ok(false)
        }
        SessionCommand::Hypotheses => {
            print_react_hypotheses(session);
            Ok(false)
        }
        SessionCommand::Plan
        | SessionCommand::Memory(_)
        | SessionCommand::Remember(_)
        | SessionCommand::Forget(_)
        | SessionCommand::Autonomy(_)
        | SessionCommand::Save
        | SessionCommand::Resume(_)
        | SessionCommand::Sessions
        | SessionCommand::Stats
        | SessionCommand::Compact
        | SessionCommand::Reset
        | SessionCommand::Skip => Ok(false),
        SessionCommand::Abort => {
            session.abort();
            println!("Session ended.");
            Ok(true)
        }
    }
}

async fn execute_suggestion(
    handler: &mut CliHandlers,
    tools: &application::services::tool_executor::ToolExecutor,
    validator: &mut SyntaxGrammarValidator,
    command: &mut ProposedCommand,
    query: &str,
    emit_output: bool,
    auto_summary: bool,
) -> Result<ExecutionResult> {
    let command_line = command.command.clone();
    let tokens = parse_command_tokens(&command_line);
    if tokens.is_empty() {
        return Ok(ExecutionResult {
            output: "No command provided".to_string(),
            summary: None,
            output_printed: false,
        });
    }

    let tool_name = tokens[0].clone();
    let raw_args = tokens[1..].to_vec();
    let (normalized_args, rewrite_note) = normalize_tool_args(&tool_name, &raw_args, query);
    let args = normalized_args
        .iter()
        .map(|s| s.as_str())
        .collect::<Vec<_>>();

    if tool_name == "rag" {
        let output = execute_rag_via_service(handler, &normalized_args, query).await?;
        command.approve();
        command.execute(0, output.clone(), String::new());
        if let Some(note) = rewrite_note {
            return Ok(ExecutionResult {
                output: format!("{note}\n{output}"),
                summary: None,
                output_printed: false,
            });
        }
        return Ok(ExecutionResult {
            output,
            summary: None,
            output_printed: false,
        });
    }

    if tools.has_tool(&tool_name) {
        command.approve();
        let result = tools.execute(&tool_name, &args);
        return match result {
            Ok(output) => {
                command.execute(
                    output.exit_code,
                    output.stdout.clone(),
                    output.stderr.clone(),
                );
                let formatted = format_tool_output(&tool_name, &output);
                if let Some(note) = rewrite_note {
                    Ok(ExecutionResult {
                        output: format!("{note}\n{formatted}"),
                        summary: None,
                        output_printed: false,
                    })
                } else {
                    Ok(ExecutionResult {
                        output: formatted,
                        summary: None,
                        output_printed: false,
                    })
                }
            }
            Err(err) => {
                command.execute(1, String::new(), err.to_string());
                Ok(ExecutionResult {
                    output: format!("Tool '{}' failed: {}", tool_name, err),
                    summary: None,
                    output_printed: false,
                })
            }
        };
    }

    let reviewed = review_candidates(&[CommandCandidate::new(command.command.clone())], validator);
    if let Some(rejected) = reviewed.rejected.first() {
        command.reject();
        return Ok(ExecutionResult {
            output: format!(
                "Rejected command: {} ({})",
                rejected.command,
                rejected.reasons.join(", ")
            ),
            summary: None,
            output_printed: false,
        });
    }

    command.approve();
    let (output, summary, output_printed) = if auto_summary {
        let (tx, rx) = std::sync::mpsc::channel();
        let (ack_tx, ack_rx) = std::sync::mpsc::channel();
        let handle = handler.spawn_incremental_interpreter_with_policy(
            query,
            rx,
            ack_tx,
            super::domain_output::ChunkSummaryPolicy::long_output_default(),
        );
        let sink = super::OutputSink { tx, ack: ack_rx };
        let output = handler.run_shell_command_streaming_with_sink(&command.command, Some(sink))?;
        let summary = handle.join().unwrap_or_default();
        (output, Some(summary), true)
    } else if emit_output {
        let output = handler.run_shell_command_streaming(&command.command)?;
        (output, None, true)
    } else {
        let output = handler.run_shell_command_capture(&command.command)?;
        (output, None, false)
    };
    let exit_code = output.status.code().unwrap_or(-1);
    command.execute(exit_code, output.full_output.clone(), output.stderr.clone());
    Ok(ExecutionResult {
        output: output.full_output,
        summary,
        output_printed,
    })
}

async fn execute_rag_via_service(
    handler: &mut CliHandlers,
    args: &[String],
    fallback_query: &str,
) -> Result<String> {
    let (rag_query, limit) = if args.is_empty() {
        (fallback_query.to_string(), None)
    } else if args.len() > 1 {
        match args.last().and_then(|v| v.parse::<usize>().ok()) {
            Some(limit) => (args[..args.len() - 1].join(" "), Some(limit)),
            None => (args.join(" "), None),
        }
    } else {
        (args[0].clone(), None)
    };

    if handler.rag_service.is_none() {
        let client = OllamaClient::new()?;
        handler.rag_service = Some(
            RagService::new(".", &handler.config.db_path, client, handler.config.clone()).await?,
        );
    }

    let keywords = crate::cli::command_extraction::query_keywords(&rag_query);
    if let Some(rag) = handler.rag_service.as_ref() {
        rag.build_index_for_keywords(&keywords).await?;
        if let Some(limit) = limit {
            let chunks = rag.relevant_chunks(&rag_query, limit).await?;
            if chunks.is_empty() {
                return Ok("No relevant context found.".to_string());
            }
            return Ok(chunks.join("\n\n---\n\n"));
        }
        return rag.query(&rag_query).await;
    }

    Ok("RAG service unavailable.".to_string())
}

fn normalize_tool_args(
    tool_name: &str,
    args: &[String],
    query: &str,
) -> (Vec<String>, Option<String>) {
    let Some(path_arg_index) = tool_path_arg_index(tool_name) else {
        return (args.to_vec(), None);
    };
    if args.len() <= path_arg_index {
        return (args.to_vec(), None);
    }
    if !is_placeholder_path(&args[path_arg_index]) {
        return (args.to_vec(), None);
    }

    let Some(real_path) = select_workspace_path(query) else {
        return (args.to_vec(), None);
    };

    let mut normalized = args.to_vec();
    let old = normalized[path_arg_index].clone();
    normalized[path_arg_index] = real_path.clone();
    (
        normalized,
        Some(format!(
            "[Resolved placeholder path: '{old}' -> '{real_path}']"
        )),
    )
}

fn tool_path_arg_index(tool_name: &str) -> Option<usize> {
    match tool_name {
        "read" | "read_pdf" | "read_docx" | "read_xlsx" | "extract_tables" | "doc_qa" | "write"
        | "remove" | "update" | "replace_block" | "ast" => Some(0),
        "sed" | "perl" => Some(2),
        "awk" => Some(1),
        _ => None,
    }
}

fn build_exploration_seed_command(query: &str) -> String {
    let safe_query = query.replace('"', "\\\"");
    format!("rag \"{safe_query}\" 12")
}

fn parse_command_tokens(command_line: &str) -> Vec<String> {
    match shell_words::split(command_line) {
        Ok(tokens) if !tokens.is_empty() => tokens,
        _ => command_line
            .split_whitespace()
            .map(str::to_string)
            .collect::<Vec<_>>(),
    }
}

fn primary_command_binary(command_line: &str) -> Option<String> {
    let tokens = parse_command_tokens(command_line);
    if tokens.is_empty() {
        return None;
    }
    let mut idx = 0usize;
    if tokens.get(0).map(|t| t.as_str()) == Some("shell") {
        idx = 1;
    }
    while let Some(token) = tokens.get(idx) {
        if is_env_assignment(token) || is_redirection_token(token) {
            idx += 1;
            continue;
        }
        return Some(token.trim_matches(&['\'', '"'][..]).to_string());
    }
    None
}

fn is_placeholder_path(arg: &str) -> bool {
    let value = arg.trim().to_lowercase();
    if (value.starts_with('<') && value.ends_with('>'))
        || (value.starts_with('[') && value.ends_with(']'))
    {
        return true;
    }
    matches!(
        value.as_str(),
        "path"
            | "file"
            | "filepath"
            | "filename"
            | "your_file"
            | "your/path"
            | "path/to/file"
            | "./path/to/file"
            | "/path/to/file"
            | "file_path"
    ) || value.contains("path/to/")
        || value.contains("your_file")
}

fn select_workspace_path(query: &str) -> Option<String> {
    let output = Command::new("rg").arg("--files").output().ok()?;
    if !output.status.success() {
        return None;
    }

    let files = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if files.is_empty() {
        return None;
    }

    let keywords = query
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| w.len() >= 3)
        .map(|w| w.to_lowercase())
        .collect::<Vec<_>>();

    let mut best: Option<(i32, String)> = None;
    for file in &files {
        let mut score = 0_i32;
        let lower = file.to_lowercase();

        for keyword in &keywords {
            if lower.contains(keyword) {
                score += 2;
            }
        }
        if lower.ends_with(".rs") || lower.ends_with(".md") || lower.ends_with(".toml") {
            score += 1;
        }
        if lower.contains("target/") || lower.contains(".git/") {
            score -= 10;
        }

        if let Some((best_score, _)) = &best {
            if score > *best_score {
                best = Some((score, file.clone()));
            }
        } else {
            best = Some((score, file.clone()));
        }
    }

    if let Some((score, path)) = best {
        if score > 0 {
            return Some(path);
        }
    }

    for preferred in [
        "README.md",
        "Cargo.toml",
        "src/main.rs",
        "presentation/src/cli/handlers/react.rs",
    ] {
        if files.iter().any(|f| f == preferred) {
            return Some(preferred.to_string());
        }
    }

    files.first().cloned()
}

fn react_history_path() -> PathBuf {
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".config")
        .join("vibe_cli")
        .join("react_history.txt")
}

fn format_tool_output(name: &str, output: &domain::tools::ToolOutput) -> String {
    if name == "shell" {
        let mut rendered = output.stdout.clone();
        if !output.stderr.trim().is_empty() {
            if !rendered.trim().is_empty() {
                rendered.push('\n');
            }
            rendered.push_str("Errors:\n");
            rendered.push_str(output.stderr.trim_end());
        }
        if rendered.trim().is_empty() {
            return "(no output)".to_string();
        }
        return rendered;
    }

    let mut lines = Vec::new();
    lines.push(format!("Tool: {name}"));
    for (key, value) in &output.metadata {
        lines.push(format!("[{key}: {value}]"));
    }
    if !output.stdout.trim().is_empty() {
        lines.push(output.stdout.clone());
    }
    if !output.stderr.trim().is_empty() {
        lines.push(format!("stderr:\n{}", output.stderr));
    }
    if lines.len() == 1 {
        lines.push("(no output)".to_string());
    }
    lines.join("\n")
}

fn prompt_line(prompt: &str, interrupted: &AtomicBool) -> Result<Option<String>> {
    if interrupted.load(Ordering::SeqCst) {
        return Ok(None);
    }

    // Ensure we're in a good terminal state and then switch to raw mode for key handling.
    let _ = disable_raw_mode();
    enable_raw_mode()?;
    struct RawModeGuard;
    impl Drop for RawModeGuard {
        fn drop(&mut self) {
            let _ = disable_raw_mode();
        }
    }
    let _guard = RawModeGuard;

    print!("{prompt}");
    io::stdout().flush()?;

    let mut input = String::new();
    loop {
        match read()? {
            Event::Key(key) => match key.code {
                KeyCode::Enter => {
                    println!();
                    return Ok(Some(input));
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    interrupted.store(true, Ordering::SeqCst);
                    println!();
                    return Ok(None);
                }
                KeyCode::Backspace => {
                    if !input.is_empty() {
                        input.pop();
                        print!("\u{8} \u{8}");
                        io::stdout().flush()?;
                    }
                }
                KeyCode::Char(ch) => {
                    input.push(ch);
                    print!("{ch}");
                    io::stdout().flush()?;
                }
                KeyCode::Tab => {
                    input.push('\t');
                    print!("\t");
                    io::stdout().flush()?;
                }
                _ => {}
            },
            _ => {}
        }
    }
}

fn prompt_next_goal(interrupted: &AtomicBool) -> Result<Option<String>> {
    let input = match prompt_line("Next request (empty to end): ", interrupted)? {
        Some(value) => value,
        None => return Ok(None),
    };
    let trimmed = input.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed.to_string()))
    }
}

fn read_shell_line(
    repl: &mut Option<LineEditor>,
    interrupted: &AtomicBool,
) -> Result<Option<String>> {
    if let Some(repl) = repl.as_mut() {
        return repl.read_line();
    }
    prompt_line("> ", interrupted)
}

fn read_user_line(
    prompt: &str,
    interrupted: &AtomicBool,
    repl: &mut Option<LineEditor>,
) -> Result<Option<String>> {
    if let Some(repl) = repl.as_mut() {
        let message = if prompt.trim().is_empty() {
            None
        } else {
            Some(prompt)
        };
        return repl.read_line_with_prompt(message);
    }
    prompt_line(prompt, interrupted)
}

fn summarize_output_for_observation(output: &str) -> String {
    let mut lines = Vec::new();
    for line in output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(6)
    {
        lines.push(line.to_string());
    }

    if lines.is_empty() {
        "No significant output captured.".to_string()
    } else {
        lines.join("\n")
    }
}

fn print_markdown(text: &str) {
    let skin = MadSkin::default();
    let fmt = skin.term_text(text);
    print!("{}", fmt);
}

fn print_with_pager(output: &str) {
    if output.trim().is_empty() {
        return;
    }
    ensure_fresh_line();
    print!("{output}");
    if !output.ends_with('\n') {
        print!("\n");
    }
    let _ = io::stdout().flush();
}

fn ensure_fresh_line() {
    print!("\r\n");
    let _ = io::stdout().flush();
}
