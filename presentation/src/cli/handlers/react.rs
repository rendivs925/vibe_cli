use super::CliHandlers;
use application::services::test_time_scaling::ScalingConfig;
use colored::Colorize;
use crate::cli::cache::CommandCandidate;
use crate::cli::command_review::review_candidates;
use anyhow::anyhow;
use application::services::react_agent_service::ReactAgentService;
use domain::entities::react::{
    ProposedCommand, ReactSession, ReactStatus, ReactStep, ReactStepType,
};
use infrastructure::react_storage::InMemoryReactStorage;
use infrastructure::syntax_grammar_validator::SyntaxGrammarValidator;
use shared::confirmation::ask_confirmation;
use shared::types::Result;
use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::Arc;

impl CliHandlers {
    pub async fn handle_react(&mut self, query: &str, neurosymbolic: bool, scaling_config: &ScalingConfig) -> Result<()> {
        use application::services::test_time_scaling::ScalingMethod;

        if query.trim().is_empty() {
            println!("Provide a task for --react");
            return Ok(());
        }

        if scaling_config.method != ScalingMethod::None {
            if let Some(best_cmd) = self.select_best_with_scaling(query, scaling_config).await {
                println!("Scaling selected command: {}", best_cmd);
                if ask_confirmation("Run this command directly?", false)? {
                    let status = std::process::Command::new("bash")
                        .arg("-c")
                        .arg(&best_cmd)
                        .status()?;
                    if status.success() {
                        println!("{}", "Command completed successfully.".green());
                    } else {
                        println!("{} (exit status: {:?})", "Command failed.".red(), status.code());
                    }
                    return Ok(());
                }
            }
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

        println!("ReAct session for: \"{}\"", session.query);
        print_react_help();

        let mut validator = SyntaxGrammarValidator::new();
        let mut iteration = 0_u32;
        let mut step_no = 1_u32;
        let mut auto_execute_all = false;
        let mut last_action_commands: Option<Vec<ProposedCommand>> = None;

        while iteration < 30 && matches!(session.status, ReactStatus::Running) {
            iteration += 1;

            let thought = service.generate_reasoning(&session).await?;
            print_step_header(step_no, "THOUGHT");
            println!("{thought}");
            step_no += 1;

            let mut thought_step =
                ReactStep::new(session.id.clone(), ReactStepType::Thought, thought.clone())
                    .with_reasoning(thought);
            thought_step.start();
            thought_step.complete();
            service.save_step(&thought_step).await?;
            session.add_step(thought_step);
            service.save_session(&session).await?;

            let mut commands = service.propose_commands("", &session).await?;
            if commands.is_empty() {
                println!("\nNo commands proposed for this step.");
                break;
            }

            let domain_operation = detect_domain_operation(&commands);
            let action_title = match domain_operation.as_deref() {
                Some(op) => format!("ACTION (Domain Operation: {op})"),
                None => "ACTION".to_string(),
            };
            print_step_header(step_no, &action_title);
            if let Some(op) = domain_operation.as_deref() {
                println!(
                    "Operation: {}{}",
                    op,
                    infer_operation_inputs(op, &session.query)
                );
            }
            println!("Generated commands:");
            for (idx, cmd) in commands.iter().enumerate() {
                println!("  {}. {}", idx + 1, cmd.command);
            }

            loop {
                let decision = if auto_execute_all {
                    ActionDecision::Execute
                } else {
                    prompt_action()?
                };

                match decision {
                    ActionDecision::Execute => break,
                    ActionDecision::Skip => {
                        println!("\nSkipping this action.");
                        commands.clear();
                        break;
                    }
                    ActionDecision::Revise => {
                        let revised = prompt_line("How should the command be revised? ")?;
                        if revised.trim().is_empty() {
                            println!("Revision cannot be empty.");
                            continue;
                        }
                        commands = vec![ProposedCommand::new(
                            revised.clone(),
                            "User revised command".to_string(),
                            "Manual revision".to_string(),
                        )];
                        println!("Revised commands:");
                        println!("  1. {revised}");
                    }
                    ActionDecision::ReviseGoal => {
                        let new_goal = prompt_line("New goal: ")?;
                        if !new_goal.trim().is_empty() {
                            session.query = new_goal;
                            service.save_session(&session).await?;
                            println!("Goal updated.");
                        }
                    }
                    ActionDecision::AutoAll => {
                        auto_execute_all = true;
                        break;
                    }
                    ActionDecision::Abort => {
                        session.abort();
                        service.save_session(&session).await?;
                        println!("Session aborted.");
                        return Ok(());
                    }
                    ActionDecision::SessionCommand(command) => match command {
                        SessionCommand::Help => {
                            print_react_help();
                        }
                        SessionCommand::Context => {
                            print_react_context(&session, iteration);
                        }
                        SessionCommand::Retry => {
                            if let Some(previous) = last_action_commands.as_ref() {
                                commands = previous.clone();
                                println!("Retrying last action commands.");
                            } else {
                                println!("No previous action to retry.");
                            }
                        }
                        SessionCommand::Skip => {
                            println!("Skipping this action.");
                            commands.clear();
                            break;
                        }
                        SessionCommand::Abort => {
                            session.abort();
                            service.save_session(&session).await?;
                            println!("Session aborted.");
                            return Ok(());
                        }
                        SessionCommand::Revise(text) => {
                            if !text.trim().is_empty() {
                                session.query = text;
                                service.save_session(&session).await?;
                                println!("Goal updated.");
                            }
                        }
                    },
                }
            }
            step_no += 1;

            if !commands.is_empty() {
                let mut rejected = HashMap::new();
                let candidates: Vec<CommandCandidate> = commands
                    .iter()
                    .map(|cmd| CommandCandidate::new(cmd.command.clone()))
                    .collect();
                let reviewed = review_candidates(&candidates, &mut validator);
                for rejected_candidate in reviewed.rejected {
                    rejected.insert(rejected_candidate.command, rejected_candidate.reasons);
                }

                let mut observation_lines = Vec::new();
                for cmd in &mut commands {
                    if let Some(reasons) = rejected.get(&cmd.command) {
                        println!("Rejected: {} ({})", cmd.command, reasons.join(", "));
                        cmd.reject();
                        service.update_command(cmd).await.ok();
                        continue;
                    }

                    cmd.approve();
                    let output = self.run_shell_command_streaming(&cmd.command)?;
                    let exit_code = output.status.code().unwrap_or(-1);
                    cmd.execute(exit_code, output.full_output.clone(), output.stderr.clone());
                    service.update_command(cmd).await.ok();

                    observation_lines.push(summarize_output_for_observation(&output.full_output));
                }

                if !observation_lines.is_empty() {
                    print_observation(&observation_lines.join("\n"));
                    let mut observation_step = ReactStep::new(
                        session.id.clone(),
                        ReactStepType::Observation,
                        observation_lines.join("\n"),
                    );
                    for line in observation_lines {
                        observation_step.add_observation(line);
                    }
                    observation_step.start();
                    observation_step.complete();
                    service.save_step(&observation_step).await?;
                    session.add_step(observation_step);
                    service.save_session(&session).await?;
                }
            }

            if !commands.is_empty() {
                last_action_commands = Some(commands.clone());
            }

            if let Some(inference) = service.generate_symbolic_inference(&session).await? {
                print_step_header(step_no, "SYMBOLIC INFERENCE");
                println!("{inference}");
                step_no += 1;
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
    Revise(String),
    Retry,
    Skip,
    Abort,
}

#[derive(Debug)]
enum ActionDecision {
    Execute,
    Skip,
    Revise,
    ReviseGoal,
    AutoAll,
    Abort,
    SessionCommand(SessionCommand),
}

fn print_step_header(step_no: u32, title: &str) {
    println!("\n--- STEP {step_no}: {title} ---");
}

fn print_observation(summary: &str) {
    println!("\n--- OBSERVATION ---");
    println!("{summary}");
}

fn print_goal_achieved(summary: &str, iterations: u32, neurosymbolic_enabled: bool) {
    println!("\n--- GOAL ACHIEVED ---");
    println!("{summary}");
    println!("Iterations: {iterations}");
    if neurosymbolic_enabled {
        println!("Mode: ReAct + Neurosymbolic");
    } else {
        println!("Mode: ReAct");
    }
}

fn print_react_help() {
    println!("\nAvailable commands during session:");
    println!("  /revise <new_goal>  - Update the goal mid-session");
    println!("  /context            - Show current reasoning context");
    println!("  /retry              - Retry last action");
    println!("  /skip               - Skip to next step");
    println!("  /abort              - End session");
    println!("  /help               - Show commands");
}

fn print_react_context(session: &ReactSession, iteration: u32) {
    println!("Goal: {}", session.query);
    println!("Iterations: {iteration}");
    println!("Steps: {}", session.steps.len());
}

fn prompt_action() -> Result<ActionDecision> {
    let input = prompt_line("Execute? [Y/n/r/g/a/x]: ")?;
    let trimmed = input.trim();
    if trimmed.starts_with('/') {
        return match parse_session_command(trimmed) {
            Ok(command) => Ok(ActionDecision::SessionCommand(command)),
            Err(err) => {
                println!("{err}");
                prompt_action()
            }
        };
    }

    match trimmed.to_lowercase().as_str() {
        "" | "y" => Ok(ActionDecision::Execute),
        "n" => Ok(ActionDecision::Skip),
        "r" => Ok(ActionDecision::Revise),
        "g" => Ok(ActionDecision::ReviseGoal),
        "a" => Ok(ActionDecision::AutoAll),
        "x" => Ok(ActionDecision::Abort),
        _ => {
            println!("Invalid option. Use Y/n/r/g/a/x.");
            prompt_action()
        }
    }
}

fn parse_session_command(input: &str) -> Result<SessionCommand> {
    let mut parts = input.trim().splitn(2, ' ');
    let cmd = parts.next().unwrap_or_default();
    let arg = parts.next().unwrap_or_default().trim();
    match cmd {
        "/help" => Ok(SessionCommand::Help),
        "/context" => Ok(SessionCommand::Context),
        "/retry" => Ok(SessionCommand::Retry),
        "/skip" => Ok(SessionCommand::Skip),
        "/abort" => Ok(SessionCommand::Abort),
        "/revise" => Ok(SessionCommand::Revise(arg.to_string())),
        _ => Err(anyhow!("Unknown command. Use /help")),
    }
}

fn prompt_line(prompt: &str) -> Result<String> {
    print!("{prompt}");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim_end().to_string())
}

fn detect_domain_operation(commands: &[ProposedCommand]) -> Option<String> {
    let first = commands.first()?;
    let prefix = "Domain op: ";
    if first.description.starts_with(prefix) {
        Some(first.description[prefix.len()..].trim().to_string())
    } else {
        None
    }
}

fn infer_operation_inputs(operation: &str, query: &str) -> String {
    if operation.contains("service") {
        if let Some(service_name) = extract_service_name(query) {
            return format!(" {{service: \"{service_name}\"}}");
        }
    }
    String::new()
}

fn extract_service_name(query: &str) -> Option<String> {
    let candidates = [
        "nginx",
        "apache2",
        "httpd",
        "mysql",
        "postgresql",
        "postgres",
        "redis",
        "docker",
        "sshd",
        "ssh",
    ];
    let lowered = query.to_lowercase();
    candidates
        .iter()
        .find(|service| lowered.contains(**service))
        .map(|s| s.to_string())
}

fn summarize_output_for_observation(output: &str) -> String {
    let mut lines = Vec::new();
    for line in output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(4)
    {
        lines.push(line.to_string());
    }

    if lines.is_empty() {
        "No significant output captured.".to_string()
    } else {
        lines.join("\n")
    }
}
