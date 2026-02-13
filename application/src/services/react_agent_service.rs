use anyhow::anyhow;
use domain::entities::react::{
    ProposedCommand, ReactContext, ReactSession, ReactStatus, ReactStep, ReactStepType,
};
use domain::repositories::react_repository::{ReactCommandRepository, ReactRepository};
use infrastructure::ollama_client::OllamaClient;
use shared::types::Result;
use std::sync::Arc;

use crate::services::neurosymbolic_service::NeurosymbolicService;

pub struct ReactAgentService {
    neurosymbolic_service: Option<Arc<NeurosymbolicService>>,
    react_repository: Arc<dyn ReactRepository>,
    command_repository: Arc<dyn ReactCommandRepository>,
    client: OllamaClient,
    max_iterations: u32,
}

impl ReactAgentService {
    pub fn new(
        neurosymbolic_service: Option<Arc<NeurosymbolicService>>,
        react_repository: Arc<dyn ReactRepository>,
        command_repository: Arc<dyn ReactCommandRepository>,
    ) -> Result<Self> {
        Ok(Self {
            neurosymbolic_service,
            react_repository,
            command_repository,
            client: OllamaClient::new()?,
            max_iterations: 10,
        })
    }

    pub fn with_max_iterations(mut self, max_iterations: u32) -> Self {
        self.max_iterations = max_iterations;
        self
    }

    pub async fn start_session(&self, query: String, neurosymbolic: bool) -> Result<ReactSession> {
        let session = ReactSession::new(query, neurosymbolic);
        self.react_repository
            .save_session(&session)
            .await
            .map_err(|e| anyhow!(e.to_string()))?;
        Ok(session)
    }

    pub async fn execute_react_loop(&self, session: &mut ReactSession) -> Result<()> {
        let mut context = ReactContext::new(self.max_iterations);

        while context.should_continue() && matches!(session.status, ReactStatus::Running) {
            let reasoning = self.generate_reasoning(session).await?;
            let thought_step = ReactStep::new(
                session.id.clone(),
                ReactStepType::Thought,
                reasoning.clone(),
            )
            .with_reasoning(reasoning.clone());
            self.add_step(session, thought_step).await?;

            let commands = self.propose_commands(&reasoning, session).await?;
            let mut action_step = ReactStep::new(
                session.id.clone(),
                ReactStepType::Action,
                "Proposed commands".to_string(),
            );
            for command in commands {
                self.command_repository
                    .save_command(&command)
                    .await
                    .map_err(|e| anyhow!(e.to_string()))?;
                action_step.add_command(command);
            }
            self.add_step(session, action_step).await?;

            context.increment_iteration();
            break;
        }

        Ok(())
    }

    pub async fn process_user_input(
        &self,
        session: &mut ReactSession,
        input: String,
    ) -> Result<ReactStep> {
        let mut step = ReactStep::new(session.id.clone(), ReactStepType::Observation, input);
        step.start();
        step.complete();
        self.add_step(session, step.clone()).await?;
        Ok(step)
    }

    pub async fn generate_reasoning(&self, session: &ReactSession) -> Result<String> {
        let history = Self::format_history(session);
        let prompt = format!(
            "You are a careful systems assistant using a conversational ReAct loop. Produce the next REASONING only.\n\
Goal: {goal}\n\
History:\n{history}\n\
Rules:\n\
- Output concise reasoning (1-4 sentences).\n\
- Do not include commands or code blocks.\n\
- Focus on the next diagnostic step.\n\
- If user has redirected strategy, acknowledge and adapt.\n",
            goal = session.query,
            history = if history.is_empty() { "(none)" } else { &history }
        );

        let response = self.client.generate_response(&prompt).await?;
        let thought = response.trim().to_string();
        if thought.is_empty() {
            return Err(anyhow!("empty reasoning response"));
        }
        Ok(thought)
    }

    pub async fn propose_commands(
        &self,
        reasoning: &str,
        session: &ReactSession,
    ) -> Result<Vec<ProposedCommand>> {
        if session.neurosymbolic_enabled {
            if let Some(service) = self.neurosymbolic_service.as_ref() {
                if let Some(suggestion) = service.suggest_commands_from_domains(&session.query) {
                    let mut commands = Vec::new();
                    let reasoning = format!(
                        "Matched domain op '{}' (id: {}, confidence {:.0}%)",
                        suggestion.op_name,
                        suggestion.op_id,
                        suggestion.confidence * 100.0
                    );
                    for command in suggestion.commands {
                        commands.push(ProposedCommand::new(
                            command,
                            format!("Domain op: {}", suggestion.op_name),
                            reasoning.clone(),
                        ));
                    }
                    if !commands.is_empty() {
                        return Ok(commands);
                    }
                }
            }
        }

        let history = Self::format_history(session);
        let prompt = format!(
            "You are a cautious systems assistant. Based on the goal and reasoning, propose 1-3 executable suggestions.\n\
Respond ONLY with a JSON array of strings. No prose.\n\
Goal: {goal}\n\
Reasoning: {reasoning}\n\
History:\n{history}\n\
Available tools:\n\
- read <path> [lines] [offset]\n\
- grep <pattern> [path]\n\
- fd <pattern> [directory]\n\
- rag <query> [num_results]\n\
- sed <pattern> <replacement> <path>\n\
- perl <regex> <replacement> <path>\n\
- awk <script> <path>\n\
- apply_patch <patch_file>\n\
- write <path> <content>\n\
- remove <path>\n\
- update <path> <old> <new>\n\
- replace_block <path> <old_block> <new_block>\n\
- shell <command>\n\
- pkg <install|remove|search|update|upgrade> [package]\n\
- svc <start|stop|restart|status|enable|disable> <service>\n\
- git <status|diff|add|commit|log> [args]\n\
- build <check|build|fmt|clippy> [package]\n\
- test [pattern]\n\
Constraints:\n\
- Prefer read-only diagnostics first.\n\
- Avoid destructive commands.\n\
- Use standard Linux tools.\n\
- If a built-in tool is better, output it directly as the command string.\n\
- NEVER use placeholder paths such as <path>, /path/to/..., your_file, or /tmp/example.\n\
- Only use real paths likely to exist under current working directory.\n\
- If path is unknown, first suggest discovery commands using shell/fd/find to locate files.\n\
- For codebase exploration/explanation tasks, prefer rag first; rag is AST-aware during indexing.\n\
- For project explanation tasks, start by discovering structure, then read real files (README.md, Cargo.toml, src/*).\n",
            goal = session.query,
            reasoning = reasoning,
            history = if history.is_empty() { "(none)" } else { &history }
        );

        let response = self.client.generate_response(&prompt).await?;
        let parsed = parse_command_list(&response);
        let mut commands = Vec::new();
        for command in parsed {
            if command.trim().is_empty() {
                continue;
            }
            commands.push(ProposedCommand::new(
                command,
                "LLM proposed".to_string(),
                reasoning.to_string(),
            ));
        }
        Ok(commands)
    }

    pub async fn execute_approved_command(&self, command: &mut ProposedCommand) -> Result<()> {
        if command.approved != Some(true) {
            return Err(anyhow!("command not approved"));
        }

        let output = std::process::Command::new("bash")
            .arg("-c")
            .arg(&command.command)
            .output()?;

        let exit_code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        command.execute(exit_code, stdout, stderr);
        Ok(())
    }

    pub async fn generate_symbolic_inference(
        &self,
        session: &ReactSession,
    ) -> Result<Option<String>> {
        if !session.neurosymbolic_enabled {
            return Ok(None);
        }

        let history = Self::format_history(session);
        let prompt = format!(
            "You are a symbolic diagnostics engine for Linux troubleshooting.\n\
Based on the ReAct history, produce a concise symbolic inference in this exact shape:\n\
Rule: <rule_name>\n\
Conditions:\n\
  - <condition 1>\n\
Conclusion: <single sentence>\n\
No markdown fences. No extra sections.\n\
Goal: {goal}\n\
History:\n{history}\n",
            goal = session.query,
            history = if history.is_empty() {
                "(none)"
            } else {
                &history
            }
        );

        let response = self.client.generate_response(&prompt).await?;
        let cleaned = response.trim().to_string();
        if cleaned.is_empty() {
            return Ok(None);
        }
        Ok(Some(cleaned))
    }

    pub async fn is_goal_achieved(&self, session: &ReactSession) -> Result<bool> {
        let history = Self::format_history(session);
        let prompt = format!(
            "Decide if the troubleshooting goal is achieved based on history.\n\
Reply with ONLY YES or NO.\n\
Goal: {goal}\n\
History:\n{history}\n",
            goal = session.query,
            history = if history.is_empty() {
                "(none)"
            } else {
                &history
            }
        );

        let response = self.client.generate_response(&prompt).await?;
        Ok(response.trim().eq_ignore_ascii_case("yes"))
    }

    pub async fn generate_goal_summary(&self, session: &ReactSession) -> Result<String> {
        let history = Self::format_history(session);
        let prompt = format!(
            "Summarize troubleshooting result in this exact format:\n\
Root cause: <text>\n\
Fix applied: <text>\n\
Use \"Unknown\" when not confirmed. No extra lines.\n\
Goal: {goal}\n\
History:\n{history}\n",
            goal = session.query,
            history = if history.is_empty() {
                "(none)"
            } else {
                &history
            }
        );

        let response = self.client.generate_response(&prompt).await?;
        let summary = response.trim().to_string();
        if summary.is_empty() {
            return Ok("Root cause: Unknown\nFix applied: Unknown".to_string());
        }
        Ok(summary)
    }

    pub async fn save_session(&self, session: &ReactSession) -> Result<()> {
        self.react_repository
            .update_session(session)
            .await
            .map_err(|e| anyhow!(e.to_string()))
    }

    pub async fn save_step(&self, step: &ReactStep) -> Result<()> {
        self.react_repository
            .save_step(step)
            .await
            .map_err(|e| anyhow!(e.to_string()))
    }

    pub async fn update_step(&self, step: &ReactStep) -> Result<()> {
        self.react_repository
            .update_step(step)
            .await
            .map_err(|e| anyhow!(e.to_string()))
    }

    pub async fn save_command(&self, command: &ProposedCommand) -> Result<()> {
        self.command_repository
            .save_command(command)
            .await
            .map_err(|e| anyhow!(e.to_string()))
    }

    pub async fn update_command(&self, command: &ProposedCommand) -> Result<()> {
        self.command_repository
            .update_command(command)
            .await
            .map_err(|e| anyhow!(e.to_string()))
    }

    async fn add_step(&self, session: &mut ReactSession, mut step: ReactStep) -> Result<()> {
        step.start();
        step.complete();
        self.react_repository
            .save_step(&step)
            .await
            .map_err(|e| anyhow!(e.to_string()))?;
        session.add_step(step);
        self.react_repository
            .update_session(session)
            .await
            .map_err(|e| anyhow!(e.to_string()))
    }

    fn format_history(session: &ReactSession) -> String {
        let mut lines = Vec::new();
        for step in session.steps.iter().rev().take(6).rev() {
            let label = match step.step_type {
                ReactStepType::Thought => "REASONING",
                ReactStepType::Action => "SUGGESTED COMMAND",
                ReactStepType::Observation => "OUTPUT",
            };
            let content = step.content.trim();
            if !content.is_empty() {
                lines.push(format!("{}: {}", label, content));
            }
            if !step.observations.is_empty() {
                lines.push(format!("Observations: {}", step.observations.join(" | ")));
            }
        }
        lines.join("\n")
    }
}

fn parse_command_list(response: &str) -> Vec<String> {
    if let Ok(list) = serde_json::from_str::<Vec<String>>(response.trim()) {
        return list;
    }

    if let Some(json) = extract_json_array(response) {
        if let Ok(list) = serde_json::from_str::<Vec<String>>(json) {
            return list;
        }
    }

    domain::services::command_extraction::extract_candidate_commands(response, "")
}

fn extract_json_array(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    let mut depth = 0_i32;
    let mut start = None;
    let mut in_string = false;
    let mut escape_next = false;

    for (i, &b) in bytes.iter().enumerate() {
        if escape_next {
            escape_next = false;
            continue;
        }

        match b {
            b'"' => in_string = !in_string,
            b'\\' => {
                if in_string {
                    escape_next = true;
                }
            }
            b'[' => {
                if !in_string && depth == 0 {
                    start = Some(i);
                }
                if !in_string {
                    depth += 1;
                }
            }
            b']' => {
                if !in_string {
                    depth -= 1;
                    if depth == 0 {
                        if let Some(s) = start {
                            return Some(&text[s..=i]);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    None
}
