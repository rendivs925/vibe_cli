use super::CliHandlers;
use crate::cli::cache::CommandCandidate;
use crate::cli::command_review::review_candidates;
use application::services::react_agent_service::ReactAgentService;
use colored::Colorize;
use dialoguer::{theme::ColorfulTheme, Input};
use domain::entities::react::{
    ProposedCommand, ReactSession, ReactStatus, ReactStep, ReactStepStatus, ReactStepType,
};
use infrastructure::react_storage::InMemoryReactStorage;
use infrastructure::syntax_grammar_validator::SyntaxGrammarValidator;
use shared::confirmation::ask_confirmation;
use shared::types::Result;
use std::collections::HashMap;
use std::sync::Arc;

impl CliHandlers {
    pub async fn handle_react(&mut self, query: &str, neurosymbolic: bool) -> Result<()> {
        if query.trim().is_empty() {
            println!("{}", "Provide a task for --react".red());
            return Ok(());
        }

        let storage = Arc::new(InMemoryReactStorage::new());
        let react_repo = storage.clone();
        let cmd_repo = storage.clone();
        let neurosymbolic_service = if neurosymbolic {
            Some(Arc::new(application::services::neurosymbolic_service::NeurosymbolicService::new()?))
        } else {
            None
        };

        let service = ReactAgentService::new(neurosymbolic_service, react_repo, cmd_repo)?;
        let mut session = service
            .start_session(query.to_string(), neurosymbolic)
            .await?;

        println!(
            "{} {}",
            "ReAct session started:".green(),
            session.id.blue()
        );
        println!("{} {}", "Goal:".green(), session.query.yellow());
        print_react_help();

        let mut validator = SyntaxGrammarValidator::new();
        let mut iteration = 0_u32;
        let mut last_action_step_id: Option<String> = None;
        let mut last_action_commands: Vec<ProposedCommand> = Vec::new();

        while iteration < 10 && matches!(session.status, ReactStatus::Running) {
            iteration += 1;
            println!(
                "\n{} {}",
                "Iteration".green().bold(),
                format!("{}:", iteration).green().bold()
            );

            if let Some(cmd) = maybe_handle_session_command()? {
                match cmd {
                    SessionCommand::Help => {
                        print_react_help();
                        continue;
                    }
                    SessionCommand::Context => {
                        print_react_context(&session, iteration);
                        continue;
                    }
                    SessionCommand::Revise(text) => {
                        session.query = text;
                        service.save_session(&session).await?;
                        println!("{}", "Goal updated.".green());
                        continue;
                    }
                    SessionCommand::Observation(text) => {
                        let mut observation_step = ReactStep::new(
                            session.id.clone(),
                            ReactStepType::Observation,
                            text,
                        );
                        observation_step.start();
                        observation_step.complete();
                        service.save_step(&observation_step).await?;
                        session.add_step(observation_step);
                        service.save_session(&session).await?;
                        continue;
                    }
                    SessionCommand::Abort => {
                        session.abort();
                        service.save_session(&session).await?;
                        println!("{}", "Session aborted.".yellow());
                        break;
                    }
                    SessionCommand::Retry => {
                        if let Some(step_id) = last_action_step_id.clone() {
                            retry_last_commands(
                                self,
                                &service,
                                &mut session,
                                &mut last_action_commands,
                                &step_id,
                            )
                            .await?;
                        } else {
                            println!("{}", "No prior commands to retry.".yellow());
                        }
                        continue;
                    }
                    SessionCommand::Skip => {
                        println!("{}", "Skipping iteration.".yellow());
                        continue;
                    }
                }
            }

            let reasoning = service.generate_reasoning(&session).await?;
            println!("{} {}", "Thought:".green(), reasoning.white());
            let mut thought_step = ReactStep::new(
                session.id.clone(),
                ReactStepType::Thought,
                reasoning.clone(),
            )
            .with_reasoning(reasoning.clone());
            thought_step.start();
            thought_step.complete();
            service.save_step(&thought_step).await?;
            session.add_step(thought_step);
            service.save_session(&session).await?;

            let commands = service.propose_commands(&reasoning, &session).await?;
            if commands.is_empty() {
                println!("{}", "No commands proposed. Provide input or /revise.".yellow());
                let user_input: String = Input::with_theme(&ColorfulTheme::default())
                    .with_prompt("Observation")
                    .interact_text()?;
                let mut observation_step = ReactStep::new(
                    session.id.clone(),
                    ReactStepType::Observation,
                    user_input.clone(),
                );
                observation_step.start();
                observation_step.complete();
                service.save_step(&observation_step).await?;
                session.add_step(observation_step);
                service.save_session(&session).await?;
                continue;
            }

            println!("{}", "Proposed commands:".green());
            for (i, cmd) in commands.iter().enumerate() {
                println!("  {} {}", format!("[{}]", i + 1).blue(), cmd.command);
            }

            let mut action_step = ReactStep::new(
                session.id.clone(),
                ReactStepType::Action,
                "Proposed commands".to_string(),
            );
            for cmd in &commands {
                action_step.add_command(cmd.clone());
            }
            action_step.start();
            action_step.complete();
            service.save_step(&action_step).await?;
            session.add_step(action_step.clone());
            service.save_session(&session).await?;

            last_action_step_id = Some(action_step.id.clone());
            last_action_commands = commands.clone();

            let mut rejected = HashMap::new();
            let candidates: Vec<CommandCandidate> = commands
                .iter()
                .map(|cmd| CommandCandidate::new(cmd.command.clone()))
                .collect();
            let reviewed = review_candidates(&candidates, &mut validator);
            for rejected_candidate in reviewed.rejected {
                rejected.insert(rejected_candidate.command, rejected_candidate.reasons);
            }

            for cmd in &mut last_action_commands {
                if let Some(reasons) = rejected.get(&cmd.command) {
                    println!(
                        "{} {}",
                        "Rejected: ".red(),
                        format!("{} ({})", cmd.command, reasons.join(", ")).red()
                    );
                    cmd.reject();
                    service.update_command(cmd).await.ok();
                    continue;
                }

                println!("{} {}", "Command:".green(), cmd.command.yellow());
                if ask_confirmation("Run this command?", false)? {
                    cmd.approve();
                    let output = self.run_shell_command_streaming(&cmd.command)?;
                    let exit_code = output.status.code().unwrap_or(-1);
                    cmd.execute(exit_code, output.full_output.clone(), output.stderr.clone());
                    service.update_command(cmd).await.ok();

                    let observation = format_observation(&cmd.command, &output);
                    let mut observation_step = ReactStep::new(
                        session.id.clone(),
                        ReactStepType::Observation,
                        observation.clone(),
                    );
                    observation_step.add_observation(observation);
                    observation_step.start();
                    observation_step.complete();
                    service.save_step(&observation_step).await?;
                    session.add_step(observation_step);
                    service.save_session(&session).await?;
                } else {
                    cmd.reject();
                    service.update_command(cmd).await.ok();
                    println!("{}", "Skipping command.".yellow());
                }
            }

            if !prompt_continue()? {
                session.abort();
                service.save_session(&session).await?;
                println!("{}", "Session aborted.".yellow());
                break;
            }
        }

        if iteration >= 10 && matches!(session.status, ReactStatus::Running) {
            session.fail();
            service.save_session(&session).await?;
            println!("{}", "Max iterations reached.".yellow());
        }

        Ok(())
    }
}

#[derive(Debug)]
enum SessionCommand {
    Help,
    Context,
    Observation(String),
    Revise(String),
    Retry,
    Skip,
    Abort,
}

fn maybe_handle_session_command() -> Result<Option<SessionCommand>> {
    let input: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("ReAct")
        .allow_empty(true)
        .interact_text()?;
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if !trimmed.starts_with('/') {
        return Ok(Some(SessionCommand::Observation(trimmed.to_string())));
    }

    let mut parts = trimmed.splitn(2, ' ');
    let cmd = parts.next().unwrap_or("");
    let arg = parts.next().unwrap_or("").trim();
    let result = match cmd {
        "/help" => SessionCommand::Help,
        "/context" => SessionCommand::Context,
        "/retry" => SessionCommand::Retry,
        "/skip" => SessionCommand::Skip,
        "/abort" => SessionCommand::Abort,
        "/revise" => {
            let text = if arg.is_empty() {
                Input::with_theme(&ColorfulTheme::default())
                    .with_prompt("New goal")
                    .interact_text()?
            } else {
                arg.to_string()
            };
            SessionCommand::Revise(text)
        }
        _ => {
            println!("{}", "Unknown command. Use /help".yellow());
            return Ok(None);
        }
    };
    Ok(Some(result))
}

fn print_react_help() {
    println!("{}", "Session commands:".green());
    println!("  /revise <text>  Update the goal");
    println!("  /context        Show current session state");
    println!("  /retry          Retry last proposed commands");
    println!("  /skip           Skip current iteration");
    println!("  /abort          Abort the session");
    println!("  /help           Show this help");
}

fn print_react_context(session: &ReactSession, iteration: u32) {
    println!("{} {}", "Goal:".green(), session.query.yellow());
    println!("{} {}", "Iterations:".green(), iteration);
    println!("{} {}", "Steps:".green(), session.steps.len());
    if let Some(step) = session.current_step() {
        println!(
            "{} {:?}",
            "Last step:".green(),
            step.step_type
        );
    }
}

fn prompt_continue() -> Result<bool> {
    ask_confirmation("Continue ReAct loop?", true)
}

fn format_observation(command: &str, output: &super::CommandOutput) -> String {
    let mut summary = String::new();
    summary.push_str(&format!("Command: {}\n", command));
    summary.push_str(&format!(
        "Exit: {}\n",
        output.status.code().unwrap_or(-1)
    ));
    if !output.full_output.trim().is_empty() {
        summary.push_str("Stdout:\n");
        summary.push_str(trim_output(&output.full_output));
        summary.push('\n');
    }
    if !output.stderr.trim().is_empty() {
        summary.push_str("Stderr:\n");
        summary.push_str(trim_output(&output.stderr));
        summary.push('\n');
    }
    summary
}

fn trim_output(output: &str) -> &str {
    const LIMIT: usize = 2000;
    let trimmed = output.trim();
    if trimmed.len() <= LIMIT {
        trimmed
    } else {
        &trimmed[..LIMIT]
    }
}

async fn retry_last_commands(
    handlers: &CliHandlers,
    service: &ReactAgentService,
    session: &mut ReactSession,
    commands: &mut Vec<ProposedCommand>,
    step_id: &str,
) -> Result<()> {
    if commands.is_empty() {
        println!("{}", "No commands available to retry.".yellow());
        return Ok(());
    }

    println!("{}", "Retrying last commands".green());
    for cmd in commands.iter_mut() {
        println!("{} {}", "Command:".green(), cmd.command.yellow());
        if ask_confirmation("Run this command?", false)? {
            cmd.approve();
            let output = handlers.run_shell_command_streaming(&cmd.command)?;
            let exit_code = output.status.code().unwrap_or(-1);
            cmd.execute(exit_code, output.full_output.clone(), output.stderr.clone());
            service.update_command(cmd).await.ok();

            let observation = format_observation(&cmd.command, &output);
            let mut observation_step = ReactStep::new(
                session.id.clone(),
                ReactStepType::Observation,
                observation.clone(),
            );
            observation_step.add_observation(observation);
            observation_step.start();
            observation_step.complete();
            service.save_step(&observation_step).await?;
            session.add_step(observation_step);
            service.save_session(session).await?;
        } else {
            cmd.reject();
            service.update_command(cmd).await.ok();
        }
    }

    if let Some(step) = session.steps.iter_mut().find(|s| s.id == step_id) {
        step.status = ReactStepStatus::Completed;
    }

    Ok(())
}
