use super::CliHandlers;
use crate::cli::cache::CommandCandidate;
use crate::cli::command_review::review_candidates;
use anyhow::anyhow;
use application::services::react_agent_service::ReactAgentService;
use application::services::tool_executor::ToolExecutor;
use application::services::test_time_scaling::{ScalingConfig, ScalingMethod};
use domain::entities::react::{
    ProposedCommand, ReactSession, ReactStatus, ReactStep, ReactStepType,
};
use infrastructure::react_storage::InMemoryReactStorage;
use infrastructure::syntax_grammar_validator::SyntaxGrammarValidator;
use infrastructure::tools;
use shared::types::Result;
use std::io::{self, Write};
use std::process::Command;
use std::sync::Arc;

impl CliHandlers {
    pub async fn handle_react(
        &mut self,
        query: &str,
        neurosymbolic: bool,
        scaling_config: &ScalingConfig,
    ) -> Result<()> {
        if query.trim().is_empty() {
            println!("Provide a task for --react");
            return Ok(());
        }

        let storage = Arc::new(InMemoryReactStorage::new());
        let react_repo = storage.clone();
        let cmd_repo = storage.clone();
        let neurosymbolic_service = if neurosymbolic {
            Some(Arc::new(
                application::services::neurosymbolic_service::NeurosymbolicService::new()?,
            ))
        } else {
            None
        };

        let service = ReactAgentService::new(neurosymbolic_service, react_repo, cmd_repo)?
            .with_max_iterations(30);
        let mut session = service
            .start_session(query.to_string(), neurosymbolic)
            .await?;

        let mut validator = SyntaxGrammarValidator::new();
        let tools = build_default_tool_executor();

        println!("ReAct session for: \"{}\"", session.query);
        print_react_help();

        let mut iteration = 0_u32;
        while iteration < 30 && matches!(session.status, ReactStatus::Running) {
            iteration += 1;

            let reasoning = service.generate_reasoning(&session).await?;
            print_section("REASONING", &reasoning);
            save_step(
                &service,
                &mut session,
                ReactStepType::Thought,
                reasoning.clone(),
            )
            .await?;

            let mut commands = service.propose_commands(&reasoning, &session).await?;
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

            let mut suggested = commands.remove(0);
            print_section("SUGGESTED COMMAND", &suggested.command);

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

            let decision = loop {
                let input = prompt_line("Allow? y/n> ")?;
                match parse_allow_input(&input)? {
                    AllowDecision::PromptForDirection => {
                        let extra = prompt_line("> ")?;
                        if extra.trim().is_empty() {
                            continue;
                        }
                        break AllowDecision::Direction(extra);
                    }
                    other => break other,
                }
            };

            match decision {
                AllowDecision::Execute => {
                    let output = execute_suggestion(
                        self,
                        &tools,
                        &mut validator,
                        &mut suggested,
                        &session.query,
                    )?;
                    print_section("OUTPUT", &output);
                    save_step(
                        &service,
                        &mut session,
                        ReactStepType::Observation,
                        summarize_output_for_observation(&output),
                    )
                    .await?;
                    service.update_command(&suggested).await.ok();
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
                    save_step(&service, &mut session, ReactStepType::Observation, text).await?;
                    continue;
                }
                AllowDecision::SessionCommand(command) => {
                    if handle_session_command(command, &mut session, iteration)? {
                        service.save_session(&session).await?;
                        return Ok(());
                    }
                    service.save_session(&session).await?;
                    continue;
                }
                AllowDecision::PromptForDirection => continue,
            }

            if let Some(inference) = service.generate_symbolic_inference(&session).await? {
                print_section("SYMBOLIC INFERENCE", &inference);
            }

            if service.is_goal_achieved(&session).await.unwrap_or(false) {
                session.complete();
                service.save_session(&session).await?;
                let summary = service
                    .generate_goal_summary(&session)
                    .await
                    .unwrap_or_else(|_| "Root cause: Unknown\nFix applied: Unknown".to_string());
                print_goal_achieved(&summary, iteration, session.neurosymbolic_enabled);
                return Ok(());
            }
        }

        if matches!(session.status, ReactStatus::Running) {
            session.fail();
            service.save_session(&session).await?;
            println!("\nSession ended without a confirmed resolution.");
        }

        Ok(())
    }
}

#[derive(Debug)]
enum SessionCommand {
    Help,
    Context,
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

fn print_section(title: &str, content: &str) {
    println!("\n--- {title} ---");
    println!("{content}");
}

fn print_goal_achieved(summary: &str, iterations: u32, neurosymbolic_enabled: bool) {
    println!("\n--- REASONING ---");
    println!("Goal achieved.");
    println!("{summary}");
    println!("Iterations: {iterations}");
    if neurosymbolic_enabled {
        println!("Mode: ReAct + Neurosymbolic");
    } else {
        println!("Mode: ReAct");
    }
}

fn print_react_help() {
    println!("\nBuilt-in session commands:");
    println!("  /help    - Show commands");
    println!("  /context - Show recent reasoning history");
    println!("  /skip    - Skip current suggestion");
    println!("  /abort   - End session");
}

fn print_react_context(session: &ReactSession, iteration: u32) {
    println!("Goal: {}", session.query);
    println!("Iterations: {iteration}");
    println!("Steps: {}", session.steps.len());
    for step in session.steps.iter().rev().take(6).rev() {
        let label = match step.step_type {
            ReactStepType::Thought => "REASONING",
            ReactStepType::Action => "SUGGESTED COMMAND",
            ReactStepType::Observation => "OUTPUT",
        };
        println!("- {label}: {}", step.content);
    }
}

fn parse_allow_input(input: &str) -> Result<AllowDecision> {
    let trimmed = input.trim();
    if trimmed.starts_with('/') {
        return Ok(AllowDecision::SessionCommand(parse_session_command(
            trimmed,
        )?));
    }

    match trimmed.to_lowercase().as_str() {
        "" | "y" | "yes" => Ok(AllowDecision::Execute),
        "skip" => Ok(AllowDecision::Skip),
        "n" | "no" => Ok(AllowDecision::PromptForDirection),
        _ => Ok(AllowDecision::Direction(trimmed.to_string())),
    }
}

fn build_default_tool_executor() -> ToolExecutor {
    let mut executor = ToolExecutor::new();
    executor.register(Arc::new(tools::exploration::read_tool::ReadTool));
    executor.register(Arc::new(tools::exploration::grep_tool::GrepTool));
    executor.register(Arc::new(tools::exploration::fd_tool::FdTool));
    executor.register(Arc::new(tools::exploration::rag_tool::RagTool));
    executor.register(Arc::new(tools::editing::sed_tool::SedTool));
    executor.register(Arc::new(tools::editing::perl_tool::PerlTool));
    executor.register(Arc::new(tools::editing::awk_tool::AwkTool));
    executor.register(Arc::new(tools::editing::apply_patch_tool::ApplyPatchTool));
    executor.register(Arc::new(tools::file_ops::write_tool::WriteTool));
    executor.register(Arc::new(tools::file_ops::remove_tool::RemoveTool));
    executor.register(Arc::new(tools::file_ops::update_tool::UpdateTool));
    executor.register(Arc::new(tools::system::shell_tool::ShellTool));
    executor.register(Arc::new(tools::system::pkg_tool::PkgTool));
    executor.register(Arc::new(tools::system::svc_tool::SvcTool));
    executor
}

fn parse_session_command(input: &str) -> Result<SessionCommand> {
    match input.trim() {
        "/help" => Ok(SessionCommand::Help),
        "/context" => Ok(SessionCommand::Context),
        "/skip" => Ok(SessionCommand::Skip),
        "/abort" => Ok(SessionCommand::Abort),
        _ => Err(anyhow!("Unknown command. Use /help")),
    }
}

fn handle_session_command(
    command: SessionCommand,
    session: &mut ReactSession,
    iteration: u32,
) -> Result<bool> {
    match command {
        SessionCommand::Help => {
            print_react_help();
            Ok(false)
        }
        SessionCommand::Context => {
            print_react_context(session, iteration);
            Ok(false)
        }
        SessionCommand::Skip => Ok(false),
        SessionCommand::Abort => {
            session.abort();
            println!("Session ended.");
            Ok(true)
        }
    }
}

fn execute_suggestion(
    handler: &CliHandlers,
    tools: &application::services::tool_executor::ToolExecutor,
    validator: &mut SyntaxGrammarValidator,
    command: &mut ProposedCommand,
    query: &str,
) -> Result<String> {
    let command_line = command.command.clone();
    let tokens = command_line
        .split_whitespace()
        .map(str::to_string)
        .collect::<Vec<_>>();
    if tokens.is_empty() {
        return Ok("No command provided".to_string());
    }

    let tool_name = tokens[0].clone();
    let raw_args = tokens[1..].to_vec();
    let (normalized_args, rewrite_note) = normalize_tool_args(&tool_name, &raw_args, query);
    let args = normalized_args
        .iter()
        .map(|s| s.as_str())
        .collect::<Vec<_>>();

    if tools.has_tool(&tool_name) {
        if requires_second_confirmation(tools, &tool_name)
            && !prompt_line("This tool can modify system state. Continue? [y/N]>")?
                .trim()
                .eq_ignore_ascii_case("y")
        {
            command.reject();
            return Ok("Execution cancelled by user.".to_string());
        }

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
                    Ok(format!("{note}\n{formatted}"))
                } else {
                    Ok(formatted)
                }
            }
            Err(err) => {
                command.execute(1, String::new(), err.to_string());
                Ok(format!("Tool '{}' failed: {}", tool_name, err))
            }
        };
    }

    let reviewed = review_candidates(&[CommandCandidate::new(command.command.clone())], validator);
    if let Some(rejected) = reviewed.rejected.first() {
        command.reject();
        return Ok(format!(
            "Rejected command: {} ({})",
            rejected.command,
            rejected.reasons.join(", ")
        ));
    }

    command.approve();
    let output = handler.run_shell_command_streaming(&command.command)?;
    let exit_code = output.status.code().unwrap_or(-1);
    command.execute(exit_code, output.full_output.clone(), output.stderr.clone());
    Ok(output.full_output)
}

fn normalize_tool_args(tool_name: &str, args: &[String], query: &str) -> (Vec<String>, Option<String>) {
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
        "read" | "write" | "remove" | "update" => Some(0),
        "sed" | "perl" => Some(2),
        "awk" => Some(1),
        _ => None,
    }
}

fn is_placeholder_path(arg: &str) -> bool {
    let value = arg.trim().to_lowercase();
    if (value.starts_with('<') && value.ends_with('>')) || (value.starts_with('[') && value.ends_with(']')) {
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

fn requires_second_confirmation(
    tools: &application::services::tool_executor::ToolExecutor,
    name: &str,
) -> bool {
    tools
        .list_tools()
        .into_iter()
        .any(|t| t.name == name && t.requires_confirmation)
}

fn format_tool_output(name: &str, output: &domain::tools::ToolOutput) -> String {
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

fn prompt_line(prompt: &str) -> Result<String> {
    print!("{prompt}");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim_end().to_string())
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
