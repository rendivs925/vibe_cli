use crate::cli::streaming::request_command_stream_then_confirm;

use super::cache::{CacheManager, ExplainCacheManager, RagCacheManager};
use super::command_extraction::{extract_command_from_response, parse_agent_plan};
use super::utils::{detect_system_info, project_cache_suffix};
use application::services::integrated_neurosymbolic_service::{
    IntentSignal, IntegratedNeurosymbolicService,
};
use application::services::neurosymbolic_service::{NeurosymbolicConfig, NeurosymbolicService};
use application::services::rag_service::RagService;
use colored::Colorize;
use infrastructure::{config::Config, ollama_client::OllamaClient};
use infrastructure::storage::experience_buffer::FailureType;
use serde::Deserialize;
use shared::confirmation::{ask_confirmation, ask_feedback};
use shared::types::Message;
use shared::types::Result;

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct CommandValidationResult {
    pub command: String,
    pub is_valid: bool,
    pub syntax_valid: bool,
    pub command_available: bool,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CommandValidator;

impl CommandValidator {
    pub fn new() -> Self {
        Self
    }

    pub fn validate(&self, cmd: &str) -> CommandValidationResult {
        let trimmed = cmd.trim();
        if trimmed.is_empty() {
            return CommandValidationResult {
                command: cmd.to_string(),
                is_valid: false,
                syntax_valid: false,
                command_available: false,
                error_message: Some("Empty command".to_string()),
            };
        }

        let syntax_valid = self.check_syntax(trimmed);
        let command_available = self.check_command_availability(trimmed);

        let is_valid = syntax_valid && command_available;

        let error_message = if !syntax_valid {
            Some("Syntax error".to_string())
        } else if !command_available {
            let first_cmd = self.extract_first_command(trimmed);
            Some(format!(
                "Command not found: '{}' (try: apt install {})",
                first_cmd, first_cmd
            ))
        } else {
            None
        };

        CommandValidationResult {
            command: cmd.to_string(),
            is_valid,
            syntax_valid,
            command_available,
            error_message,
        }
    }

    fn check_syntax(&self, cmd: &str) -> bool {
        let output = Command::new("bash").args(&["-n", "-c", cmd]).output().ok();

        match output {
            Some(o) => o.status.success(),
            None => false,
        }
    }

    fn check_command_availability(&self, cmd: &str) -> bool {
        let first_cmd = self.extract_first_command(cmd);
        if first_cmd.is_empty() {
            return false;
        }

        let output = Command::new("command")
            .args(&["-v", &first_cmd])
            .output()
            .ok();

        match output {
            Some(o) => o.status.success(),
            None => {
                let which_output = Command::new("which").arg(&first_cmd).output().ok();
                which_output.map(|o| o.status.success()).unwrap_or(false)
            }
        }
    }

    fn extract_first_command(&self, cmd: &str) -> String {
        let tokens: Vec<&str> = cmd.split_whitespace().collect();
        if tokens.is_empty() {
            return String::new();
        }

        let first = tokens[0];

        if first.starts_with("sudo") && tokens.len() > 1 {
            tokens[1].to_string()
        } else if first.starts_with('-') {
            String::new()
        } else {
            first.to_string()
        }
    }

    pub fn validate_multiple(&self, commands: &[String]) -> Vec<CommandValidationResult> {
        commands.iter().map(|cmd| self.validate(cmd)).collect()
    }

    pub fn filter_valid(&self, commands: &[String]) -> Vec<String> {
        self.validate_multiple(commands)
            .into_iter()
            .filter(|r| r.is_valid)
            .map(|r| r.command)
            .collect()
    }

    pub fn summarize_validation(&self, results: &[CommandValidationResult]) -> String {
        let total = results.len();
        let valid = results.iter().filter(|r| r.is_valid).count();
        let invalid = total - valid;

        if total == 0 {
            return "No commands to validate.".to_string();
        }

        let mut summary = format!("Command Validation: {}/{} valid", valid, total);

        if invalid > 0 {
            summary.push_str(&format!("\n{}", "Invalid commands:".red().bold()));
            for result in results.iter().filter(|r| !r.is_valid) {
                if let Some(ref err) = result.error_message {
                    summary.push_str(&format!(
                        "\n  X {}: {}",
                        result.command.yellow(),
                        err.white()
                    ));
                }
            }
        }

        summary
    }
}

#[derive(Debug, Clone)]
pub struct IntentAnalysis {
    pub intent: String,
    pub neurosymbolic_suitable: bool,
    pub reasoning: String,
    pub action: Option<String>,
    pub target: Option<String>,
    pub objects: Vec<String>,
    pub constraints: Vec<String>,
    pub params: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct IntentAnalysisResponse {
    intent_category: Option<String>,
    neurosymbolic_suitable: Option<bool>,
    reasoning: Option<String>,
    action: Option<String>,
    target: Option<String>,
    objects: Option<Vec<String>>,
    constraints: Option<Vec<String>>,
    params: Option<HashMap<String, String>>,
}

pub struct CliHandlers {
    cache_manager: CacheManager,
    explain_cache_manager: ExplainCacheManager,
    rag_cache_manager: RagCacheManager,
    system_info: String,
    config: Config,
    rag_service: Option<RagService>,
    neurosymbolic_service: Option<NeurosymbolicService>,
    integrated_service: Option<IntegratedNeurosymbolicService>,
    command_validator: CommandValidator,
}

impl CliHandlers {
    pub fn new(config: Config) -> Self {
        let cache_path = Self::default_cache_path();
        let explain_cache_path = Self::explain_cache_path();
        let rag_cache_path = Self::rag_cache_path();
        let system_info_path = Self::default_system_info_path();
        let system_info = Self::load_or_collect_system_info(&system_info_path);

        let integrated_service = IntegratedNeurosymbolicService::new().ok();

        Self {
            cache_manager: CacheManager::new(cache_path),
            explain_cache_manager: ExplainCacheManager::new(explain_cache_path),
            rag_cache_manager: RagCacheManager::new(rag_cache_path),
            system_info,
            config,
            rag_service: None,
            neurosymbolic_service: None,
            integrated_service,
            command_validator: CommandValidator::new(),
        }
    }

    fn default_cache_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let mut path = PathBuf::from(home);
        path.push(".local");
        path.push("share");
        path.push("vibe_cli");
        let suffix = project_cache_suffix();
        path.push(format!("{}_cli_cache.bin", suffix));
        path
    }

    fn explain_cache_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let mut path = PathBuf::from(home);
        path.push(".local");
        path.push("share");
        path.push("vibe_cli");
        let suffix = project_cache_suffix();
        path.push(format!("{}_explain_cache.bin", suffix));
        path
    }

    fn rag_cache_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let mut path = PathBuf::from(home);
        path.push(".local");
        path.push("share");
        path.push("vibe_cli");
        let suffix = project_cache_suffix();
        path.push(format!("{}_rag_cache.bin", suffix));
        path
    }

    fn default_system_info_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let mut path = PathBuf::from(home);
        path.push(".config");
        path.push("vibe_cli");
        path.push("system_info.txt");
        path
    }

    fn load_or_collect_system_info(path: &PathBuf) -> String {
        if let Ok(existing) = std::fs::read_to_string(path) {
            if !existing.trim().is_empty() {
                return existing.trim().to_string();
            }
        }

        let detected = detect_system_info();

        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, &detected);

        detected
    }

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
            let prompt = format!("You are on a system with: {}. Generate a bash command to: {}. Respond with only the exact command to run, without any formatting, backticks, quotes, or explanation. Ensure the command is complete, syntactically correct, and uses standard Unix tools. For size comparisons, use appropriate units like -BG for gigabytes in df.", self.system_info, input);
            let response = client.generate_response(&prompt).await?;
            let command = extract_command_from_response(&response);
            println!("{}", format!("Command: {}", command).green());
            if ask_confirmation("Run this command?", false)? {
                let output = Command::new("bash").arg("-c").arg(&command).output()?;
                println!("{}", String::from_utf8_lossy(&output.stdout));
                if !output.status.success() {
                    println!(
                        "{}",
                        format!(
                            "Command failed: {}",
                            String::from_utf8_lossy(&output.stderr)
                        )
                        .red()
                    );
                }
            } else {
                println!("{}", "Command execution cancelled.".yellow());
            }
        }
        Ok(())
    }

    pub async fn handle_agent(&self, task: &str) -> Result<()> {
        let client = infrastructure::ollama_client::OllamaClient::new()?;
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
        let commands = parse_agent_plan(&response);

        if commands.is_empty() {
            println!(
                "{}",
                "Model did not return a runnable command list (expected JSON array).".red()
            );
            return Ok(());
        }

        println!("\n{}", "Proposed plan:".green());
        for (i, cmd) in commands.iter().enumerate() {
            println!("  {} {}", format!("[{}]", i + 1).blue(), cmd);
        }

        for (i, cmd) in commands.iter().enumerate() {
            println!(
                "\n{} {}",
                "Step".green().bold(),
                format!("{}:", i + 1).green().bold()
            );
            println!("{} {}", "Suggested command:".green(), cmd.yellow());
            let accept = ask_confirmation("Run this command?", false)?;
            if !accept {
                println!("{}", "Skipping this step.".yellow());
                continue;
            }
            let status = Command::new("bash").arg("-c").arg(cmd).status()?;
            if status.success() {
                println!("{}", "Command completed successfully.".green());
            } else {
                println!(
                    "{} (exit status: {:?})",
                    "Command failed.".red(),
                    status.code()
                );
            }
        }
        Ok(())
    }

    pub async fn handle_explain(&self, file: &str) -> Result<()> {
        let path = std::path::Path::new(file);
        let content = if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            match ext.to_lowercase().as_str() {
                "pdf" => match pdf_extract::extract_text(file) {
                    Ok(text) => text,
                    Err(e) => {
                        println!("Error extracting text from PDF '{}': {}", file, e);
                        return Ok(());
                    }
                },
                "docx" => match std::fs::read(file) {
                    Ok(bytes) => match docx_rs::read_docx(&bytes) {
                        Ok(docx) => {
                            let mut text = String::new();
                            for child in &docx.document.children {
                                match child {
                                    docx_rs::DocumentChild::Paragraph(p) => {
                                        text.push_str(&p.raw_text());
                                        text.push('\n');
                                    }
                                    docx_rs::DocumentChild::Table(_t) => {
                                        text.push_str("[Table content not extracted]\n");
                                    }
                                    _ => {}
                                }
                            }
                            text
                        }
                        Err(e) => {
                            println!("Error parsing DOCX '{}': {}", file, e);
                            return Ok(());
                        }
                    },
                    Err(e) => {
                        println!("Error reading DOCX file '{}': {}", file, e);
                        return Ok(());
                    }
                },
                _ => match std::fs::read_to_string(file) {
                    Ok(text) => text,
                    Err(_) => {
                        println!("Error: Cannot read file '{}' as text. Supported formats: text files, PDF, DOCX.", file);
                        return Ok(());
                    }
                },
            }
        } else {
            match std::fs::read_to_string(file) {
                Ok(text) => text,
                Err(_) => {
                    println!("Error: Cannot read file '{}' as text. Supported formats: text files, PDF, DOCX.", file);
                    return Ok(());
                }
            }
        };

        if content.trim().is_empty() {
            println!("Error: No text content found in file '{}'.", file);
            return Ok(());
        }

        let prompt = format!("Explain this content in detail:\n\n{}", content);

        if let Some(cached_response) = self.explain_cache_manager.load_cached(&prompt)? {
            println!("{}", cached_response);
            return Ok(());
        }

        eprintln!("Analyzing file content...");
        let client = infrastructure::ollama_client::OllamaClient::new()?;
        let response = client.generate_response(&prompt).await?;

        self.explain_cache_manager.save_cached(&prompt, &response)?;

        println!("{}", response);
        Ok(())
    }

    pub async fn handle_rag(&mut self, question: &str) -> Result<()> {
        if let Some(cached_response) = self.rag_cache_manager.load_cached(question)? {
            if ask_confirmation("Cached answer found. Use it?", true)? {
                println!("{}", cached_response);
                return Ok(());
            }
        }

        if self.rag_service.is_none() {
            eprintln!("Analyzing query and scanning codebase...");
            let client = OllamaClient::new()?;
            self.rag_service = Some(
                RagService::new(".", &self.config.db_path, client, self.config.clone()).await?,
            );
            let keywords = Self::keywords_from_text(question);
            self.rag_service
                .as_ref()
                .unwrap()
                .build_index_for_keywords(&keywords)
                .await?;
        }

        let mut feedback = String::new();
        loop {
            eprintln!("Thinking...");
            let response = self
                .rag_service
                .as_ref()
                .unwrap()
                .query_with_feedback(question, &feedback)
                .await?;

            println!("{}", response);

            if ask_confirmation("Satisfied with this response?", true)? {
                self.rag_cache_manager.save_cached(question, &response)?;
                break;
            } else {
                feedback.clear();
                feedback = ask_feedback("Provide feedback for improvement: ")?;
                eprintln!("Regenerating with feedback...");
            }
        }

        Ok(())
    }

    pub async fn handle_context(&mut self, path: &str) -> Result<()> {
        eprintln!("Loading context from {}...", path);
        let client = OllamaClient::new()?;
        self.rag_service =
            Some(RagService::new(path, &self.config.db_path, client, self.config.clone()).await?);
        self.rag_service.as_ref().unwrap().build_index().await?;
        eprintln!("Context loaded from {}", path);
        self.handle_chat().await
    }

    async fn understand_intent(&self, query: &str) -> Result<IntentAnalysis> {
        let client = infrastructure::ollama_client::OllamaClient::new()?;
        let prompt = format!(
            r#"Analyze this query and return STRICT JSON only with these keys.
Pick EXACTLY ONE value for intent_category, action, and target (no pipes or multiple values):

{{
  "intent_category": "system_info|process|memory|disk|network|service|user|file|log|hardware|general|unknown",
  "action": "list|show|check|monitor|start|stop|restart|enable|disable|find|read|delete|unknown",
  "target": "process|memory|disk|network|service|user|file|log|hardware|gpu|package|system|unknown",
  "objects": ["..."],
  "constraints": ["..."],
  "params": {{"lines": "20", "pattern": "error", "service": "nginx", "path": "/var/log"}},
  "neurosymbolic_suitable": true,
  "reasoning": "brief explanation",
}}

Query: "{}""#,
            query
        );

        let mut last = None;
        for _ in 0..3 {
            let response = client.generate_response(&prompt).await?;
            let parsed = self.parse_intent_analysis(&response)?;
            let has_signal = parsed.intent != "unknown"
                || parsed.action.as_deref().unwrap_or("unknown") != "unknown"
                || parsed.target.as_deref().unwrap_or("unknown") != "unknown";
            if has_signal {
                return Ok(parsed);
            }
            last = Some(parsed);
        }

        let mut fallback = last.unwrap_or(IntentAnalysis {
            intent: "unknown".to_string(),
            neurosymbolic_suitable: false,
            reasoning: "Could not parse".to_string(),
            action: None,
            target: None,
            objects: Vec::new(),
            constraints: Vec::new(),
            params: HashMap::new(),
        });
        fallback.neurosymbolic_suitable = false;
        Ok(fallback)
    }

    fn parse_intent_analysis(&self, response: &str) -> Result<IntentAnalysis> {
        if let Ok(parsed) = serde_json::from_str::<IntentAnalysisResponse>(response) {
            let intent = parsed
                .intent_category
                .unwrap_or_else(|| "unknown".to_string());
            let intent = Self::normalize_single_label(&intent);
            let mut neurosymbolic_suitable = parsed.neurosymbolic_suitable.unwrap_or(true);
            if intent == "log" {
                neurosymbolic_suitable = true;
            }
            let reasoning = parsed
                .reasoning
                .unwrap_or_else(|| "Could not parse".to_string());
            return Ok(IntentAnalysis {
                intent,
                neurosymbolic_suitable,
                reasoning,
                action: parsed.action.map(|a| Self::normalize_single_label(&a)),
                target: parsed.target.map(|t| Self::normalize_single_label(&t)),
                objects: parsed.objects.unwrap_or_default(),
                constraints: parsed.constraints.unwrap_or_default(),
                params: parsed.params.unwrap_or_default(),
            });
        }

        if std::env::var("VIBE_CLI_INTENT_DEBUG")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
        {
            eprintln!("Intent parse failed. Raw response:\n{}", response);
        }

        let mut intent = "unknown".to_string();
        let mut neurosymbolic_suitable = true;
        let mut reasoning = "Could not parse".to_string();
        for line in response.lines() {
            let line = line.trim();
            if line.starts_with("INTENT:") {
                intent = line.replace("INTENT:", "").trim().to_string();
            } else if line.starts_with("NEUROSYMBOLIC_SUITABLE:") {
                let val = line
                    .replace("NEUROSYMBOLIC_SUITABLE:", "")
                    .trim()
                    .to_string();
                neurosymbolic_suitable = val.to_lowercase() == "yes";
            } else if line.starts_with("REASONING:") {
                reasoning = line.replace("REASONING:", "").trim().to_string();
            }
        }

        Ok(IntentAnalysis {
            intent,
            neurosymbolic_suitable,
            reasoning,
            action: None,
            target: None,
            objects: Vec::new(),
            constraints: Vec::new(),
            params: HashMap::new(),
        })
    }

    fn normalize_single_label(input: &str) -> String {
        input
            .split('|')
            .map(|s| s.trim())
            .find(|s| !s.is_empty())
            .unwrap_or("unknown")
            .to_string()
    }

    pub async fn handle_neurosymbolic(&mut self, query: &str, ai_interpret: bool) -> Result<()> {
        if self.integrated_service.is_none() {
            eprintln!("Initializing integrated neurosymbolic service...");
            self.integrated_service = IntegratedNeurosymbolicService::new().ok();
        }

        eprintln!("Analyzing query intent...");
        let intent_analysis = self.understand_intent(query).await?;

        println!("\n{}", "=== Intent Analysis ===".green().bold());
        println!("Intent: {}", intent_analysis.intent.cyan());
        println!("Reasoning: {}", intent_analysis.reasoning.white());

        if !intent_analysis.neurosymbolic_suitable {
            println!(
                "\n{}",
                format!(
                    "Neurosymbolic not suitable ({}), using LLM...",
                    intent_analysis.intent
                )
                .yellow()
            );
            eprintln!("Falling back to LLM...");
            return self.handle_query(query, ai_interpret, true).await;
        }

        eprintln!("Processing query with neurosymbolic reasoning...");
        if self.integrated_service.is_some() {
            let intent_signal = IntentSignal {
                category: Some(intent_analysis.intent.clone()),
                action: intent_analysis.action.clone(),
                target: intent_analysis.target.clone(),
                objects: intent_analysis.objects.clone(),
                constraints: intent_analysis.constraints.clone(),
                params: intent_analysis.params.clone(),
            };
            let result = {
                let service = self.integrated_service.as_mut().unwrap();
                service.process_with_intent(query, Some(&intent_signal))
            };
            match result {
                Ok(result) => {
                    println!(
                        "\n{}",
                        "=== Integrated Neurosymbolic Response ===".green().bold()
                    );
                    println!("{}", result.format_display());

                    if !result.can_execute {
                        if let Some(reason) = result.block_reason.as_deref() {
                            println!("{}", reason.red());
                        }

                        let failure_type = if result.safety_report.is_blocked() {
                            FailureType::SafetyViolation
                        } else if !result.syntax_valid {
                            FailureType::InvalidFlag
                        } else {
                            FailureType::Other
                        };

                        if let Some(service) = self.integrated_service.as_ref() {
                            let _ = service.record_failure(
                                query,
                                &result.command,
                                failure_type,
                                result.block_reason.as_deref(),
                            );
                        }
                        return Ok(());
                    }

                    if let Some(reason) = result.block_reason.as_deref() {
                        println!("{}", reason.yellow());
                    }

                    if ask_confirmation("Execute this command?", false)? {
                        println!("\n{}", format!("Executing: {}", result.command).yellow());
                        let output = Command::new("bash")
                            .arg("-c")
                            .arg(&result.command)
                            .output()?;
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        let full_output = format!(
                            "{}{}",
                            stdout,
                            if !stderr.is_empty() {
                                format!("\nErrors:\n{}", stderr)
                            } else {
                                String::new()
                            }
                        );

                        if ai_interpret {
                            self.interpret_output(query, &full_output).await?;
                        } else {
                            println!("{}", stdout);
                        }

                        if let Some(service) = self.integrated_service.as_ref() {
                            if output.status.success() {
                                let _ = service.record_success(query, &result.command);
                            } else {
                                println!(
                                    "{}",
                                    format!("Command failed: {}", stderr).red()
                                );
                                let _ = service.record_failure(
                                    query,
                                    &result.command,
                                    FailureType::ExecutionFailed,
                                    Some(stderr.trim()),
                                );
                            }
                        }
                    } else {
                        if let Some(service) = self.integrated_service.as_ref() {
                            let _ = service.record_failure(
                                query,
                                &result.command,
                                FailureType::UserCancelled,
                                Some("User cancelled execution"),
                            );
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Integrated neurosymbolic processing failed: {:?}", e);
                    eprintln!("Falling back to standard query...");
                    self.handle_query(query, ai_interpret, true).await?;
                }
            }
        } else {
            if self.neurosymbolic_service.is_none() {
                eprintln!("Initializing neurosymbolic service with domain configs...");
                let config = NeurosymbolicConfig::default();
                match NeurosymbolicService::new(config).await {
                    Ok(service) => {
                        self.neurosymbolic_service = Some(service);
                        eprintln!("Neurosymbolic service initialized.");
                    }
                    Err(e) => {
                        eprintln!("Failed to initialize neurosymbolic service: {:?}", e);
                        return Ok(());
                    }
                }
            }

            match self
                .neurosymbolic_service
                .as_mut()
                .unwrap()
                .process_query_with_domains(query)
                .await
            {
                Ok(response) => {
                    println!("\n{}", "=== Neurosymbolic Response ===".green().bold());
                    println!("Confidence: {:.1}%", response.confidence * 100.0);
                    println!("\n{}", response.explanation);
                    println!("\n{}", "=== Best Solution ===".green().bold());
                    if let Some(solution) = response.ranked_solutions.first() {
                        let commands = &solution.solution.command_sequence;

                        let validation_results = self.command_validator.validate_multiple(commands);
                        let validation_summary = self
                            .command_validator
                            .summarize_validation(&validation_results);
                        println!("{}", validation_summary);

                        let valid_commands: Vec<String> = validation_results
                            .iter()
                            .filter(|r| r.is_valid)
                            .map(|r| r.command.clone())
                            .collect();

                        if valid_commands.is_empty() {
                            println!("\n{}", "No valid commands to execute.".red());
                            return Ok(());
                        }

                        if valid_commands.len() != commands.len() {
                            println!(
                                "\n{}",
                                format!(
                                    "Executing {} valid command(s) out of {}...",
                                    valid_commands.len(),
                                    commands.len()
                                )
                                .yellow()
                            );
                        }

                        println!("Commands to execute: {}", valid_commands.join("; ").white());
                    }
                    println!("\n{}", "=== Reasoning Summary ===".green().bold());
                    println!("{}", response.reasoning_trace.summary);

                    if ask_confirmation("Execute these commands?", false)? {
                        if let Some(solution) = response.ranked_solutions.first() {
                            let commands = &solution.solution.command_sequence;
                            let validation_results =
                                self.command_validator.validate_multiple(commands);
                            let valid_commands: Vec<String> = validation_results
                                .iter()
                                .filter(|r| r.is_valid)
                                .map(|r| r.command.clone())
                                .collect();

                            let mut all_outputs = Vec::new();
                            let mut has_any_output = false;

                            for cmd in &valid_commands {
                                println!("\n{}", format!("Executing: {}", cmd).yellow());
                                let output = Command::new("bash").arg("-c").arg(cmd).output()?;
                                let stdout = String::from_utf8_lossy(&output.stdout);
                                let stderr = String::from_utf8_lossy(&output.stderr);

                                if ai_interpret {
                                    let cmd_output = format!(
                                        "=== Command: {} ===\n{}\n{}",
                                        cmd, stdout, stderr
                                    );
                                    all_outputs.push(cmd_output);
                                } else {
                                    println!("{}", stdout);
                                }

                                if !stdout.is_empty() || !stderr.is_empty() {
                                    has_any_output = true;
                                }

                                if !output.status.success() {
                                    println!("{}", format!("Command failed: {}", stderr).red());
                                }
                            }

                            if ai_interpret && has_any_output {
                                let combined_output = all_outputs.join("\n\n");
                                match self.interpret_output(query, &combined_output).await {
                                    Ok(_) => {}
                                    Err(e) => {
                                        eprintln!("\nAI interpretation failed: {:?}", e);
                                        println!("\n=== Raw Command Output ===");
                                        println!("{}", combined_output);
                                    }
                                }
                            } else if ai_interpret {
                                println!("\n{}", "No output to interpret.".yellow());
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Neurosymbolic processing failed: {:?}", e);
                    eprintln!("Falling back to standard query...");
                    self.handle_query(query, ai_interpret, true).await?;
                }
            }
        }
        Ok(())
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

        let command = request_command_stream_then_confirm(&self.config, &messages).await?;
        if let Some(cmd) = command {
            let output = Command::new("bash").arg("-c").arg(&cmd).output()?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let full_output = format!(
                "{}{}",
                stdout,
                if !stderr.is_empty() {
                    format!("\nErrors:\n{}", stderr)
                } else {
                    String::new()
                }
            );

            if ai_interpret {
                self.interpret_output(query, &full_output).await?;
            } else {
                println!("{}", stdout);
            }

            if !output.status.success() {
                println!(
                    "{}",
                    format!("Command failed with exit code: {:?}", output.status.code()).red()
                );
                if !stderr.is_empty() {
                    println!("{}", stderr.red());
                }
            } else {
                last_successful_command = cmd;
                last_successful_query = query.to_string();
            }
        } else {
            // println!("{}", "No command generated or cancelled.".yellow());
        }

        // Learning system: offer to add successful commands to domain
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

    /// Learn a new command from successful fallback execution
    async fn learn_command(&self, query: &str, command: &str) -> Result<()> {
        if self.is_known_operation(query, command) {
            return Ok(());
        }

        println!("\n{}", "=== Learning New Command ===".green().bold());

        // Extract operation name from query
        let operation_name = Self::generate_operation_name(query);
        let operation_id = operation_name.to_lowercase().replace(" ", "_");

        // Generate description using AI
        let client = infrastructure::ollama_client::OllamaClient::new()?;
        let desc_prompt = format!(
            "Generate a short (max 80 chars) description for this command: {}\n\
             Query was: {}\n\
             Just return the description, no formatting.",
            command, query
        );
        let description = client.generate_response(&desc_prompt).await?;

        // Extract the tool from command
        let tool = command.split_whitespace().next().unwrap_or("bash");
        let template = command;

        println!("Operation Name: {}", operation_name);
        println!("Operation ID: {}", operation_id);
        println!("Description: {}", description.trim());
        println!("Tool: {}", tool);
        println!("Template: {}", template);

        if ask_confirmation("Save this operation to the Linux domain?", false)? {
            let domains_dir = self.config_dir();
            let linux_dir = domains_dir.join("linux");

            if !linux_dir.exists() {
                std::fs::create_dir_all(&linux_dir)?;
            }

            let ops_file = linux_dir.join("operations.json");

            let mut operations: Vec<serde_json::Value> = if ops_file.exists() {
                let data = std::fs::read_to_string(&ops_file)?;
                serde_json::from_str(&data)?
            } else {
                Vec::new()
            };

            let new_op = serde_json::json!({
                "op_id": operation_id,
                "name": operation_name,
                "description": description.trim(),
                "input_schema": {},
                "generators": [
                    {
                        "name": format!("{}_generator", operation_id),
                        "tool": tool,
                        "template": template,
                        "when": []
                    }
                ],
                "examples": [
                    {
                        "description": query,
                        "inputs": {}
                    }
                ]
            });

            operations.push(new_op);

            let output = serde_json::to_string_pretty(&operations)?;
            std::fs::write(&ops_file, output)?;

            println!(
                "\n{}",
                format!("Saved new operation to: {}", ops_file.display()).green()
            );
            println!("Restart vibe_cli or run --neurosymbolic-init to use the new operation.");
        }

        Ok(())
    }

    fn is_known_operation(&self, query: &str, command: &str) -> bool {
        let ops_file = self.config_dir().join("linux").join("operations.json");
        if !ops_file.exists() {
            return false;
        }

        let data = match std::fs::read_to_string(&ops_file) {
            Ok(data) => data,
            Err(_) => return false,
        };

        let operations: Vec<serde_json::Value> = match serde_json::from_str(&data) {
            Ok(ops) => ops,
            Err(_) => return false,
        };

        for op in operations {
            let examples = op
                .get("examples")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            for ex in examples {
                if let Some(desc) = ex.get("description").and_then(|v| v.as_str()) {
                    if desc.eq_ignore_ascii_case(query) {
                        return true;
                    }
                }
            }

            let generators = op
                .get("generators")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            for gen in generators {
                if let Some(template) = gen.get("template").and_then(|v| v.as_str()) {
                    if template.trim() == command.trim() {
                        return true;
                    }
                }
            }
        }

        false
    }

    fn generate_operation_name(query: &str) -> String {
        let words: Vec<&str> = query.split_whitespace().collect();

        let action_words: Vec<&str> = words
            .iter()
            .filter(|w| {
                let w = w.to_lowercase();
                ["check", "show", "list", "get", "find", "view", "display"].contains(&w.as_str())
            })
            .copied()
            .collect();

        if !action_words.is_empty() {
            let rest: Vec<&str> = words
                .iter()
                .filter(|w| !action_words.contains(w))
                .copied()
                .collect();

            let capitalized: Vec<String> = rest
                .iter()
                .map(|w| {
                    let mut chars = w.chars();
                    match chars.next() {
                        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                        None => String::new(),
                    }
                })
                .collect();

            format!(
                "{} {}",
                action_words[0].to_lowercase() + " " + &capitalized.join(" "),
                // Add "usage/status/info" based on context
                if query.to_lowercase().contains("log") || query.to_lowercase().contains("journal")
                {
                    "logs"
                } else if query.to_lowercase().contains("line") {
                    "output"
                } else {
                    "info"
                }
            )
            .trim()
            .to_string()
        } else {
            format!(
                "Check {} info",
                words
                    .first()
                    .map(|w| {
                        let mut chars = w.chars();
                        match chars.next() {
                            Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                            None => "System".to_string(),
                        }
                    })
                    .unwrap_or_else(|| "System".to_string())
            )
        }
    }

    /// Interpret command output using AI to make it readable
    async fn interpret_output(&self, query: &str, output: &str) -> Result<()> {
        println!("\n{}", "=== AI Interpretation ===".green().bold());

        let client = infrastructure::ollama_client::OllamaClient::new()?;
        let prompt = format!(
            "The user asked: '{}'\n\n\
            Command output:\n{}\n\n\
            Please provide a clear, concise summary of what this output means. \
            Focus on the key information and present it in a well-organized format. \
            Use sections and bullet points where appropriate.",
            query, output
        );

        let response = client.generate_response(&prompt).await?;
        println!("{}", response);
        Ok(())
    }

    fn keywords_from_text(text: &str) -> Vec<String> {
        text.split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
            .filter(|w| w.len() > 2)
            .map(|w| w.to_lowercase())
            .collect()
    }

    fn config_dir(&self) -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".config/vibe_cli/domains")
    }

    pub async fn handle_neurosymbolic_init(&self) -> Result<()> {
        let config_dir = self.config_dir();

        println!(
            "{}",
            "Initializing complete Linux symbolic reasoning domain..."
                .green()
                .bold()
        );

        if config_dir.exists() {
            println!(
                "{}",
                "Domain config directory already exists. Updating...".yellow()
            );
        } else {
            std::fs::create_dir_all(&config_dir)?;
        }

        let linux_dir = config_dir.join("linux/entities");
        let shared_dir = config_dir.join("../shared_entities");

        std::fs::create_dir_all(&linux_dir)?;
        std::fs::create_dir_all(&shared_dir)?;

        println!("{}", "Creating Linux symbolic reasoning domain...".green());

        // Complete Linux domain manifest
        let domain_json = r#"{
    "domain": "linux",
    "version": "2.0.0",
    "description": "Complete Linux system administration and symbolic reasoning domain",
    "depends_on": [],
    "priority": 10,
    "enabled": true,
    "author": "vibe_cli",
    "tags": ["linux", "process", "filesystem", "network", "services", "users"]
}"#;

        std::fs::write(config_dir.join("linux/domain.json"), domain_json)?;
        println!("  {}", "OK domain.json");

        // Comprehensive Linux operations
        let ops_json = r#"[
    {
        "op_id": "list_processes",
        "name": "List processes",
        "description": "List running processes with detailed information",
        "input_schema": {
            "filter": {"type": "string", "optional": true},
            "sort": {"type": "string", "optional": true}
        },
        "generators": [
            {"name": "ps_standard", "tool": "ps", "template": "ps aux", "when": []},
            {"name": "ps_tree", "tool": "ps", "template": "ps auxf", "when": []},
            {"name": "ps_sort_cpu", "tool": "ps", "template": "ps aux --sort=-%cpu", "when": []},
            {"name": "ps_sort_mem", "tool": "ps", "template": "ps aux --sort=-%mem", "when": []}
        ],
        "examples": [
            {"description": "List all processes", "inputs": {}},
            {"description": "Show process tree", "inputs": {"sort": "tree"}},
            {"description": "Find nginx processes", "inputs": {"filter": "nginx"}}
        ]
    },
    {
        "op_id": "check_memory",
        "name": "Check memory usage",
        "description": "Display memory and swap usage",
        "input_schema": {},
        "generators": [
            {"name": "free_standard", "tool": "free", "template": "free -h", "when": []},
            {"name": "free_detailed", "tool": "free", "template": "free -m -t", "when": []}
        ],
        "examples": [
            {"description": "Memory summary", "inputs": {}}
        ]
    },
    {
        "op_id": "check_disk_usage",
        "name": "Check disk usage",
        "description": "Show disk space usage by filesystem",
        "input_schema": {
            "path": {"type": "string", "optional": true},
            "type": {"type": "string", "optional": true}
        },
        "generators": [
            {"name": "df_standard", "tool": "df", "template": "df -h", "when": []},
            {"name": "df_inodes", "tool": "df", "template": "df -i", "when": []},
            {"name": "du_standard", "tool": "du", "template": "du -sh {{path}}", "when": [{"name": "path"}]},
            {"name": "ncdu_scan", "tool": "ncdu", "template": "ncdu -q -o /tmp/ncdu.json {{path}}", "when": [{"name": "path"}]}
        ],
        "examples": [
            {"description": "Disk space summary", "inputs": {}},
            {"description": "Check /var usage", "inputs": {"path": "/var"}}
        ]
    },
    {
        "op_id": "check_cpu_load",
        "name": "Check CPU load",
        "description": "Display system load averages and CPU info",
        "input_schema": {},
        "generators": [
            {"name": "uptime_standard", "tool": "uptime", "template": "uptime", "when": []},
            {"name": "top_header", "tool": "top", "template": "top -bn1 | head -20", "when": []},
            {"name": "lscpu_info", "tool": "lscpu", "template": "lscpu", "when": []}
        ],
        "examples": [
            {"description": "Current load", "inputs": {}}
        ]
    },
    {
        "op_id": "manage_service",
        "name": "Manage system services",
        "description": "Start, stop, restart, or check status of services",
        "input_schema": {
            "action": {"type": "string"},
            "service": {"type": "string"}
        },
        "generators": [
            {"name": "systemctl_status", "tool": "systemctl", "template": "systemctl status {{service}}", "when": [{"name": "action", "equals": "status"}]},
            {"name": "systemctl_start", "tool": "systemctl", "template": "sudo systemctl start {{service}}", "when": [{"name": "action", "equals": "start"}]},
            {"name": "systemctl_stop", "tool": "systemctl", "template": "sudo systemctl stop {{service}}", "when": [{"name": "action", "equals": "stop"}]},
            {"name": "systemctl_restart", "tool": "systemctl", "template": "sudo systemctl restart {{service}}", "when": [{"name": "action", "equals": "restart"}]},
            {"name": "systemctl_reload", "tool": "systemctl", "template": "sudo systemctl reload {{service}}", "when": [{"name": "action", "equals": "reload"}]},
            {"name": "systemctl_enable", "tool": "systemctl", "template": "sudo systemctl enable {{service}}", "when": [{"name": "action", "equals": "enable"}]},
            {"name": "systemctl_disable", "tool": "systemctl", "template": "sudo systemctl disable {{service}}", "when": [{"name": "action", "equals": "disable"}]},
            {"name": "systemctl_list", "tool": "systemctl", "template": "systemctl list-units --type=service --state=running", "when": [{"name": "action", "equals": "list"}]}
        ],
        "examples": [
            {"description": "Check nginx status", "inputs": {"action": "status", "service": "nginx"}},
            {"description": "Restart docker", "inputs": {"action": "restart", "service": "docker"}}
        ]
    },
    {
        "op_id": "check_network",
        "name": "Check network status",
        "description": "Display network connections, interfaces, and statistics",
        "input_schema": {
            "protocol": {"type": "string", "optional": true},
            "state": {"type": "string", "optional": true}
        },
        "generators": [
            {"name": "ss_listen", "tool": "ss", "template": "ss -tulpn", "when": []},
            {"name": "ss_all", "tool": "ss", "template": "ss -tan", "when": []},
            {"name": "ss_udp", "tool": "ss", "template": "ss -u", "when": [{"name": "protocol", "equals": "udp"}]},
            {"name": "ip_addr", "tool": "ip", "template": "ip addr show", "when": []},
            {"name": "ip_route", "tool": "ip", "template": "ip route show", "when": []},
            {"name": "netstat_listen", "tool": "netstat", "template": "netstat -tulpn", "when": []}
        ],
        "examples": [
            {"description": "Listening ports", "inputs": {}},
            {"description": "All connections", "inputs": {"protocol": "tcp"}}
        ]
    },
    {
        "op_id": "check_users",
        "name": "Check logged-in users",
        "description": "Show currently logged-in users",
        "input_schema": {},
        "generators": [
            {"name": "who_standard", "tool": "who", "template": "who", "when": []},
            {"name": "w_detailed", "tool": "w", "template": "w", "when": []},
            {"name": "last_users", "tool": "last", "template": "last -20", "when": []}
        ],
        "examples": [
            {"description": "Who is logged in", "inputs": {}}
        ]
    },
    {
        "op_id": "manage_file_permissions",
        "name": "Manage file permissions",
        "description": "Change file permissions and ownership",
        "input_schema": {
            "path": {"type": "string"},
            "mode": {"type": "string", "optional": true},
            "owner": {"type": "string", "optional": true},
            "group": {"type": "string", "optional": true},
            "recursive": {"type": "boolean", "optional": true}
        },
        "generators": [
            {"name": "chmod_mode", "tool": "chmod", "template": "chmod {{mode}} {{path}}", "when": [{"name": "mode"}]},
            {"name": "chmod_recursive", "tool": "chmod", "template": "chmod -R {{mode}} {{path}}", "when": [{"name": "mode"}, {"name": "recursive", "equals": true}]},
            {"name": "chown_user", "tool": "chown", "template": "chown {{owner}} {{path}}", "when": [{"name": "owner"}]},
            {"name": "chown_both", "tool": "chown", "template": "chown {{owner}}:{{group}} {{path}}", "when": [{"name": "owner"}, {"name": "group"}]},
            {"name": "chgrp_group", "tool": "chgrp", "template": "chgrp {{group}} {{path}}", "when": [{"name": "group"}]}
        ],
        "examples": [
            {"description": "Make executable", "inputs": {"path": "/usr/local/bin/script.sh", "mode": "755"}},
            {"description": "Change owner recursively", "inputs": {"path": "/var/www", "owner": "www-data", "recursive": true}}
        ]
    },
    {
        "op_id": "find_files",
        "name": "Find files",
        "description": "Search for files by name, type, or pattern",
        "input_schema": {
            "path": {"type": "string", "optional": true},
            "name": {"type": "string", "optional": true},
            "type": {"type": "string", "optional": true},
            "size": {"type": "string", "optional": true},
            "mtime": {"type": "string", "optional": true}
        },
        "generators": [
            {"name": "find_name", "tool": "find", "template": "find {{path}} -name \"{{name}}\"", "when": [{"name": "name"}]},
            {"name": "find_type", "tool": "find", "template": "find {{path}} -type {{type}}", "when": [{"name": "type"}]},
            {"name": "find_size", "tool": "find", "template": "find {{path}} -size {{size}}", "when": [{"name": "size"}]},
            {"name": "find_mtime", "tool": "find", "template": "find {{path}} -mtime {{mtime}}", "when": [{"name": "mtime"}]},
            {"name": "locate_fast", "tool": "locate", "template": "locate \"{{name}}\"", "when": [{"name": "name"}]},
            {"name": "which_cmd", "tool": "which", "template": "which {{name}}", "when": [{"name": "name"}]},
            {"name": "whereis_cmd", "tool": "whereis", "template": "whereis {{name}}", "when": [{"name": "name"}]}
        ],
        "examples": [
            {"description": "Find config files", "inputs": {"name": "*.conf", "path": "/etc"}},
            {"description": "Find large files", "inputs": {"size": "+100M", "path": "/var"}}
        ]
    },
    {
        "op_id": "check_logs",
        "name": "Check system logs",
        "description": "View and search system logs",
        "input_schema": {
            "log": {"type": "string", "optional": true},
            "pattern": {"type": "string", "optional": true},
            "lines": {"type": "number", "optional": true}
        },
        "generators": [
            {"name": "journalctl_recent", "tool": "journalctl", "template": "journalctl -n {{lines}}", "when": []},
            {"name": "journalctl_errors", "tool": "journalctl", "template": "journalctl -p err -n {{lines}}", "when": []},
            {"name": "journalctl_grep", "tool": "journalctl", "template": "journalctl | grep \"{{pattern}}\"", "when": [{"name": "pattern"}]},
            {"name": "tail_syslog", "tool": "tail", "template": "tail -n {{lines}} /var/log/syslog", "when": []},
            {"name": "tail_messages", "tool": "tail", "template": "tail -n {{lines}} /var/log/messages", "when": []},
            {"name": "grep_log", "tool": "grep", "template": "grep \"{{pattern}}\" /var/log/{{log}}", "when": [{"name": "pattern"}, {"name": "log"}]}
        ],
        "examples": [
            {"description": "Recent logs", "inputs": {"lines": 50}},
            {"description": "Find errors", "inputs": {"pattern": "error", "lines": 100}}
        ]
    },
    {
        "op_id": "kill_process",
        "name": "Kill process",
        "description": "Terminate processes by PID or name",
        "input_schema": {
            "target": {"type": "string"},
            "signal": {"type": "string", "optional": true},
            "force": {"type": "boolean", "optional": true}
        },
        "generators": [
            {"name": "kill_term", "tool": "kill", "template": "kill {{target}}", "when": []},
            {"name": "kill_sig", "tool": "kill", "template": "kill -{{signal}} {{target}}", "when": [{"name": "signal"}]},
            {"name": "killall_name", "tool": "killall", "template": "killall {{target}}", "when": []},
            {"name": "pkill_pattern", "tool": "pkill", "template": "pkill {{target}}", "when": []},
            {"name": "kill_force", "tool": "kill", "template": "kill -9 {{target}}", "when": [{"name": "force", "equals": true}]}
        ],
        "examples": [
            {"description": "Kill by PID", "inputs": {"target": "1234"}},
            {"description": "Force kill", "inputs": {"target": "nginx", "force": true}}
        ]
    },
    {
        "op_id": "check_package_manager",
        "name": "Check installed packages",
        "description": "List installed packages",
        "input_schema": {
            "manager": {"type": "string", "optional": true},
            "pattern": {"type": "string", "optional": true}
        },
        "generators": [
            {"name": "apt_list", "tool": "dpkg", "template": "dpkg -l", "when": []},
            {"name": "apt_search", "tool": "apt", "template": "apt list --installed", "when": []},
            {"name": "apt_search_pattern", "tool": "apt", "template": "dpkg -l | grep {{pattern}}", "when": [{"name": "pattern"}]},
            {"name": "rpm_list", "tool": "rpm", "template": "rpm -qa", "when": []},
            {"name": "yum_search", "tool": "yum", "template": "yum list installed", "when": []}
        ],
        "examples": [
            {"description": "List all packages", "inputs": {}},
            {"description": "Search for package", "inputs": {"pattern": "nginx"}}
        ]
    },
    {
        "op_id": "check_docker",
        "name": "Check Docker status",
        "description": "Display Docker containers, images, and stats",
        "input_schema": {
            "action": {"type": "string", "optional": true}
        },
        "generators": [
            {"name": "docker_ps", "tool": "docker", "template": "docker ps -a", "when": []},
            {"name": "docker_images", "tool": "docker", "template": "docker images", "when": []},
            {"name": "docker_stats", "tool": "docker", "template": "docker stats --no-stream", "when": []},
            {"name": "docker_logs", "tool": "docker", "template": "docker logs --tail 100 {{action}}", "when": [{"name": "action"}]},
            {"name": "docker_ps_filter", "tool": "docker", "template": "docker ps --filter \"name={{action}}\"", "when": [{"name": "action"}]}
        ],
        "examples": [
            {"description": "List containers", "inputs": {}},
            {"description": "Container stats", "inputs": {"action": "stats"}}
        ]
    }
]"#;

        let ops_path = config_dir.join("linux/operations.json");
        let base_ops: Vec<serde_json::Value> = serde_json::from_str(ops_json)?;
        let mut merged_ops: Vec<serde_json::Value> = Vec::new();

        if ops_path.exists() {
            if let Ok(existing_data) = std::fs::read_to_string(&ops_path) {
                if let Ok(existing_ops) = serde_json::from_str::<Vec<serde_json::Value>>(&existing_data) {
                    merged_ops.extend(existing_ops);
                }
            }
        }

        for base in base_ops {
            let base_id = base.get("op_id").and_then(|v| v.as_str()).unwrap_or("");
            if base_id.is_empty() {
                continue;
            }
            let exists = merged_ops.iter().any(|op| {
                op.get("op_id")
                    .and_then(|v| v.as_str())
                    .map(|id| id == base_id)
                    .unwrap_or(false)
            });
            if !exists {
                merged_ops.push(base);
            }
        }

        let output = serde_json::to_string_pretty(&merged_ops)?;
        std::fs::write(&ops_path, output)?;
        println!("  {}", "OK operations.json (merged)");

        // Entity definitions
        let process_entity = r#"{
    "name": "Process",
    "description": "A running process on the system",
    "core_properties": [
        {"name": "pid", "type": "integer", "meaning": "Process ID"},
        {"name": "ppid", "type": "integer", "meaning": "Parent Process ID"},
        {"name": "cmdline", "type": "string", "meaning": "Command line that started the process"},
        {"name": "user", "type": "string", "meaning": "User owning the process"},
        {"name": "cpu", "type": "number", "meaning": "CPU usage percentage"},
        {"name": "mem", "type": "number", "meaning": "Memory usage percentage"},
        {"name": "state", "type": "string", "meaning": "Process state (R/S/D/Z)"},
        {"name": "start_time", "type": "string", "meaning": "Process start time"},
        {"name": "elapsed_time", "type": "string", "meaning": "CPU time used"}
    ],
    "derived_properties": [
        {"name": "is_zombie", "expression": "state == 'Z'"},
        {"name": "is_running", "expression": "state == 'R'"},
        {"name": "is_sleeping", "expression": "state == 'S'"}
    ]
}"#;
        std::fs::write(linux_dir.join("process.json"), process_entity)?;

        let file_entity = r#"{
    "name": "File",
    "description": "A file or directory in the filesystem",
    "core_properties": [
        {"name": "path", "type": "string", "meaning": "Absolute file path"},
        {"name": "size", "type": "integer", "meaning": "File size in bytes"},
        {"name": "mode", "type": "string", "meaning": "File permissions (octal)"},
        {"name": "owner", "type": "string", "meaning": "File owner username"},
        {"name": "group", "type": "string", "meaning": "File group name"},
        {"name": "modified", "type": "string", "meaning": "Last modification time"},
        {"name": "type", "type": "string", "meaning": "File type (file/dir/symlink)"},
        {"name": "inode", "type": "integer", "meaning": "Inode number"}
    ],
    "derived_properties": [
        {"name": "is_readable", "expression": "mode contains 'r'"},
        {"name": "is_writable", "expression": "mode contains 'w'"},
        {"name": "is_executable", "expression": "mode contains 'x'"},
        {"name": "is_directory", "expression": "type == 'dir'"}
    ]
}"#;
        std::fs::write(linux_dir.join("file.json"), file_entity)?;

        let service_entity = r#"{
    "name": "Service",
    "description": "A system service managed by systemd",
    "core_properties": [
        {"name": "name", "type": "string", "meaning": "Service name"},
        {"name": "state", "type": "string", "meaning": "Service state (active/inactive/failed)"},
        {"name": "enabled", "type": "boolean", "meaning": "Whether service is enabled at boot"},
        {"name": "main_pid", "type": "integer", "meaning": "Main process PID"},
        {"name": "memory_usage", "type": "number", "meaning": "Memory usage in MB"},
        {"name": "cpu_usage", "type": "number", "meaning": "CPU usage percentage"},
        {"name": "active_since", "type": "string", "meaning": "Time service became active"},
        {"name": "description", "type": "string", "meaning": "Service description"},
        {"name": "unit_file", "type": "string", "meaning": "Unit file path"}
    ],
    "derived_properties": [
        {"name": "is_running", "expression": "state == 'active'"},
        {"name": "is_failed", "expression": "state == 'failed'"},
        {"name": "is_active", "expression": "state == 'active'"}
    ]
}"#;
        std::fs::write(linux_dir.join("service.json"), service_entity)?;

        let network_entity = r#"{
    "name": "NetworkConnection",
    "description": "A network connection or listening port",
    "core_properties": [
        {"name": "local_addr", "type": "string", "meaning": "Local IP:port"},
        {"name": "remote_addr", "type": "string", "meaning": "Remote IP:port"},
        {"name": "state", "type": "string", "meaning": "Connection state (LISTEN/ESTABLISHED/TIME_WAIT)"},
        {"name": "protocol", "type": "string", "meaning": "Protocol (tcp/udp)"},
        {"name": "process", "type": "string", "meaning": "Process owning the connection"},
        {"name": "pid", "type": "integer", "meaning": "Process ID"},
        {"name": "recv_q", "type": "integer", "meaning": "Receive queue size"},
        {"name": "send_q", "type": "integer", "meaning": "Send queue size"}
    ],
    "derived_properties": [
        {"name": "is_listening", "expression": "state == 'LISTEN'"},
        {"name": "is_established", "expression": "state == 'ESTABLISHED'"},
        {"name": "is_tcp", "expression": "protocol == 'tcp'"},
        {"name": "is_udp", "expression": "protocol == 'udp'"}
    ]
}"#;
        std::fs::write(linux_dir.join("network_connection.json"), network_entity)?;

        let user_entity = r#"{
    "name": "User",
    "description": "A system user account",
    "core_properties": [
        {"name": "username", "type": "string", "meaning": "Login username"},
        {"name": "uid", "type": "integer", "meaning": "User ID"},
        {"name": "gid", "type": "integer", "meaning": "Primary group ID"},
        {"name": "home", "type": "string", "meaning": "Home directory"},
        {"name": "shell", "type": "string", "meaning": "Login shell"},
        {"name": "logged_in", "type": "boolean", "meaning": "Currently logged in"},
        {"name": "last_login", "type": "string", "meaning": "Last login time"},
        {"name": "groups", "type": "array", "meaning": "Group memberships"}
    ]
}"#;
        std::fs::write(linux_dir.join("user.json"), user_entity)?;

        let disk_entity = r#"{
    "name": "Filesystem",
    "description": "A mounted filesystem",
    "core_properties": [
        {"name": "mount_point", "type": "string", "meaning": "Mount point path"},
        {"name": "device", "type": "string", "meaning": "Device name"},
        {"name": "total", "type": "string", "meaning": "Total space"},
        {"name": "used", "type": "string", "meaning": "Used space"},
        {"name": "available", "type": "string", "meaning": "Available space"},
        {"name": "use_percent", "type": "number", "meaning": "Usage percentage"},
        {"name": "type", "type": "string", "meaning": "Filesystem type (ext4/xfs/btrfs)"},
        {"name": "inode_total", "type": "integer", "meaning": "Total inodes"},
        {"name": "inode_used", "type": "integer", "meaning": "Used inodes"},
        {"name": "inode_free", "type": "integer", "meaning": "Free inodes"}
    ],
    "derived_properties": [
        {"name": "is_full", "expression": "use_percent > 90"},
        {"name": "is_critical", "expression": "use_percent > 95"},
        {"name": "has_inode_space", "expression": "inode_free > 1000"}
    ]
}"#;
        std::fs::write(linux_dir.join("filesystem.json"), disk_entity)?;

        let memory_entity = r#"{
    "name": "MemoryInfo",
    "description": "System memory information",
    "core_properties": [
        {"name": "total", "type": "string", "meaning": "Total RAM"},
        {"name": "used", "type": "string", "meaning": "Used RAM"},
        {"name": "free", "type": "string", "meaning": "Free RAM"},
        {"name": "shared", "type": "string", "meaning": "Shared memory"},
        {"name": "buffers", "type": "string", "meaning": "Buffers"},
        {"name": "available", "type": "string", "meaning": "Available memory"},
        {"name": "swap_total", "type": "string", "meaning": "Total swap"},
        {"name": "swap_used", "type": "string", "meaning": "Used swap"},
        {"name": "swap_free", "type": "string", "meaning": "Free swap"}
    ],
    "derived_properties": [
        {"name": "is_low_memory", "expression": "available < '1G'"},
        {"name": "is_using_swap", "expression": "swap_used > '0'"},
        {"name": "memory_pressure", "expression": "(used / total) * 100"}
    ]
}"#;
        std::fs::write(linux_dir.join("memory.json"), memory_entity)?;

        println!("  {}", "OK entities/ (6 entities: Process, File, Service, NetworkConnection, User, Filesystem, MemoryInfo)");

        // Relationships
        let relationships_json = r#"[
    {"name": "process_has_parent", "type": "hierarchical", "from": "Process", "to": "Process", "meaning": "Process has parent process"},
    {"name": "process_owns_connection", "type": "ownership", "from": "Process", "to": "NetworkConnection", "meaning": "Process owns network connection"},
    {"name": "file_belongs_to_filesystem", "type": "containment", "from": "File", "to": "Filesystem", "meaning": "File resides on filesystem"},
    {"name": "user_owns_process", "type": "ownership", "from": "User", "to": "Process", "meaning": "User started process"},
    {"name": "service_creates_process", "type": "creation", "from": "Service", "to": "Process", "meaning": "Service manages process"},
    {"name": "filesystem_mounted_at", "type": "location", "from": "Filesystem", "to": "File", "meaning": "Filesystem contains path"},
    {"name": "process_uses_file", "type": "usage", "from": "Process", "to": "File", "meaning": "Process has file open"},
    {"name": "connection_binds_to_port", "type": "binding", "from": "NetworkConnection", "to": "Port", "meaning": "Connection uses port"}
]"#;
        std::fs::write(
            config_dir.join("linux/relationships.json"),
            relationships_json,
        )?;
        println!("  {}", "OK relationships.json (8 relationships)");

        // Inference rules for symbolic reasoning
        let inference_rules_json = r#"[
    {
        "rule_id": "zombie_detect",
        "name": "Zombie Detection",
        "if": [{"entity": "Process", "prop": "state", "equals": "Z"}],
        "then": [{"conclude": "zombie_process", "confidence": 0.99, "recommendation": "Kill the parent process or restart the service"}]
    },
    {
        "rule_id": "high_cpu_process",
        "name": "High CPU Detection",
        "if": [{"entity": "Process", "prop": "cpu", "gt": 80}],
        "then": [{"conclude": "cpu_heavy_process", "confidence": 0.95, "recommendation": "Investigate process, consider killing if not essential"}]
    },
    {
        "rule_id": "high_memory_process",
        "name": "High Memory Detection",
        "if": [{"entity": "Process", "prop": "mem", "gt": 50}],
        "then": [{"conclude": "memory_heavy_process", "confidence": 0.95, "recommendation": "Check for memory leaks, consider restarting service"}]
    },
    {
        "rule_id": "disk_full",
        "name": "Full Disk Detection",
        "if": [{"entity": "Filesystem", "prop": "use_percent", "gt": 90}],
        "then": [{"conclude": "disk_space_critical", "confidence": 0.99, "recommendation": "Clean up logs, temporary files, or expand storage"}]
    },
    {
        "rule_id": "inode_full",
        "name": "Inode Exhaustion Detection",
        "if": [{"entity": "Filesystem", "prop": "inode_free", "lt": 1000}],
        "then": [{"conclude": "inode_exhaustion", "confidence": 0.99, "recommendation": "Find and remove small files, increase inodes"}]
    },
    {
        "rule_id": "low_memory",
        "name": "Low Memory Detection",
        "if": [{"entity": "MemoryInfo", "prop": "available", "matches": "[0-9]+M"}],
        "then": [{"conclude": "system_low_memory", "confidence": 0.90, "recommendation": "Free memory by stopping processes or adding RAM"}]
    },
    {
        "rule_id": "swap_usage",
        "name": "Swap Usage Detection",
        "if": [{"entity": "MemoryInfo", "prop": "swap_used", "matches": "[1-9]"}],
        "then": [{"conclude": "heavy_swap_usage", "confidence": 0.85, "recommendation": "Add RAM or reduce memory usage"}]
    },
    {
        "rule_id": "service_failed",
        "name": "Failed Service Detection",
        "if": [{"entity": "Service", "prop": "state", "equals": "failed"}],
        "then": [{"conclude": "service_failure", "confidence": 0.99, "recommendation": "Check logs and restart service"}]
    },
    {
        "rule_id": "port_listening",
        "name": "Listening Port Detection",
        "if": [{"entity": "NetworkConnection", "prop": "state", "equals": "LISTEN"}],
        "then": [{"conclude": "exposed_service", "confidence": 0.80, "recommendation": "Verify this service should be exposed"}]
    },
    {
        "rule_id": "orphaned_process",
        "name": "Orphaned Process Detection",
        "if": [{"entity": "Process", "prop": "ppid", "equals": "1"}],
        "then": [{"conclude": "orphaned_process", "confidence": 0.70, "recommendation": "Process parent died, may need cleanup"}]
    }
]"#;
        std::fs::write(
            config_dir.join("linux/inference_rules.json"),
            inference_rules_json,
        )?;
        println!("  {}", "OK inference_rules.json (10 inference rules)");

        // Troubleshooting patterns
        let troubleshooting_json = r#"[
    {
        "pattern_id": "high_cpu",
        "name": "High CPU Usage",
        "symptoms": [
            {"metric": "cpu", "observation": "high cpu"},
            {"metric": "load", "observation": "high load average"}
        ],
        "likely_causes": [
            {"cause": "runaway_process", "probability": 0.6},
            {"cause": "dos_attack", "probability": 0.2},
            {"cause": "hardware_issue", "probability": 0.1},
            {"cause": "configuration_error", "probability": 0.1}
        ],
        "checks": [
            {"step": "Find CPU hog", "command": "top -bn1 | head -20"},
            {"step": "Check per-core usage", "command": "mpstat -P ALL 1"},
            {"step": "Check system load", "command": "uptime && cat /proc/loadavg"}
        ],
        "actions": [
            {"action": "identify_hog", "methods": ["top", "htop", "ps"]},
            {"action": "kill_process", "methods": ["kill", "pkill"]},
            {"action": "restart_service", "methods": ["systemctl restart"]}
        ]
    },
    {
        "pattern_id": "high_memory",
        "name": "High Memory Usage",
        "symptoms": [
            {"metric": "memory", "observation": "high memory usage"},
            {"metric": "oom", "observation": "out of memory"}
        ],
        "likely_causes": [
            {"cause": "memory_leak", "probability": 0.5},
            {"cause": "too_many_processes", "probability": 0.3},
            {"cause": "insufficient_ram", "probability": 0.2}
        ],
        "checks": [
            {"step": "Memory usage", "command": "free -m"},
            {"step": "Top consumers", "command": "ps aux --sort=-%mem | head -10"},
            {"step": "Memory details", "command": "cat /proc/meminfo"}
        ],
        "actions": [
            {"action": "free_memory", "methods": ["kill", "service_restart"]},
            {"action": "add_swap", "methods": ["dd", "mkswap", "swapon"]},
            {"action": "optimize_config", "methods": ["edit_config"]}
        ]
    },
    {
        "pattern_id": "disk_full",
        "name": "Disk Space Issues",
        "symptoms": [
            {"metric": "disk", "observation": "disk full"},
            {"metric": "disk", "observation": "no space left"},
            {"metric": "inode", "observation": "inode exhaustion"}
        ],
        "likely_causes": [
            {"cause": "log_files", "probability": 0.4},
            {"cause": "temp_files", "probability": 0.3},
            {"cause": "large_data", "probability": 0.2},
            {"cause": "too_many_files", "probability": 0.1}
        ],
        "checks": [
            {"step": "Disk by size", "command": "du -sh /var/* | sort -h | tail -10"},
            {"step": "Large files", "command": "find /var -type f -size +100M"},
            {"step": "Inode usage", "command": "df -i"},
            {"step": "Old files", "command": "find /var -type f -mtime +30"}
        ],
        "actions": [
            {"action": "clean_logs", "methods": ["truncate", "rm"]},
            {"action": "rotate_logs", "methods": ["logrotate"]},
            {"action": "clear_cache", "methods": ["sync", "echo 3 > /proc/sys/vm/drop_caches"]},
            {"action": "extend_storage", "methods": ["lvm", "resize2fs"]}
        ]
    },
    {
        "pattern_id": "service_down",
        "name": "Service Not Running",
        "symptoms": [
            {"metric": "service", "observation": "service not running"},
            {"metric": "service", "observation": "connection refused"},
            {"metric": "port", "observation": "port not listening"}
        ],
        "likely_causes": [
            {"cause": "service_crashed", "probability": 0.4},
            {"cause": "configuration_error", "probability": 0.3},
            {"cause": "dependency_down", "probability": 0.2},
            {"cause": "port_in_use", "probability": 0.1}
        ],
        "checks": [
            {"step": "Service status", "command": "systemctl status {{service}}"},
            {"step": "Recent logs", "command": "journalctl -u {{service}} --since '1 hour ago'"},
            {"step": "Port status", "command": "ss -tulpn | grep {{port}}"},
            {"step": "Dependency status", "command": "systemctl list-dependencies {{service}}"}
        ],
        "actions": [
            {"action": "restart_service", "methods": ["systemctl restart"]},
            {"action": "fix_config", "methods": ["edit_config", "systemctl daemon-reload"]},
            {"action": "free_port", "methods": ["kill", "lsof"]},
            {"action": "start_dependency", "methods": ["systemctl start"]}
        ]
    },
    {
        "pattern_id": "network_issue",
        "name": "Network Connectivity Issues",
        "symptoms": [
            {"metric": "network", "observation": "connection timeout"},
            {"metric": "network", "observation": "network unreachable"},
            {"metric": "dns", "observation": "dns failure"}
        ],
        "likely_causes": [
            {"cause": "interface_down", "probability": 0.3},
            {"cause": "route_problem", "probability": 0.3},
            {"cause": "dns_issue", "probability": 0.2},
            {"cause": "firewall_block", "probability": 0.2}
        ],
        "checks": [
            {"step": "Interface status", "command": "ip addr show"},
            {"step": "Routing table", "command": "ip route show"},
            {"step": "DNS resolution", "command": "nslookup google.com"},
            {"step": "Firewall status", "command": "iptables -L -n"},
            {"step": "Ping test", "command": "ping -c 3 8.8.8.8"}
        ],
        "actions": [
            {"action": "restart_network", "methods": ["systemctl restart networking"]},
            {"action": "fix_route", "methods": ["ip route add"]},
            {"action": "fix_dns", "methods": ["edit_resolv.conf"]},
            {"action": "open_port", "methods": ["iptables -A"]}
        ]
    }
]"#;
        std::fs::write(
            config_dir.join("linux/troubleshooting.json"),
            troubleshooting_json,
        )?;
        println!(
            "  {}",
            "OK troubleshooting.json (5 troubleshooting patterns)"
        );

        // Shared entities
        let shared_port = r#"{
    "name": "Port",
    "description": "A network port number",
    "core_properties": [
        {"name": "number", "type": "integer", "meaning": "Port number (1-65535)"},
        {"name": "protocol", "type": "string", "meaning": "Protocol (tcp/udp)"},
        {"name": "service", "type": "string", "meaning": "Common service name"},
        {"name": "state", "type": "string", "meaning": "Port state"}
    ]
}"#;
        std::fs::write(shared_dir.join("port.json"), shared_port)?;

        println!(
            "\n{}",
            "OK Linux symbolic reasoning domain initialized!"
                .green()
                .bold()
        );
        println!("\n{}", "Summary:".green());
        println!("  - 14 operations (process, memory, disk, network, services, files, etc.)");
        println!("  - 7 entities (Process, File, Service, NetworkConnection, User, Filesystem, MemoryInfo)");
        println!("  - 8 relationships (hierarchical, ownership, containment, etc.)");
        println!("  - 10 inference rules for symbolic reasoning");
        println!("  - 5 troubleshooting patterns for common issues");

        println!("\n{}", "Usage:".green());
        println!("  vibe_cli --neurosymbolic \"list processes\"");
        println!("  vibe_cli --neurosymbolic \"check disk usage\"");
        println!("  vibe_cli --neurosymbolic \"nginx is not running\"");
        println!("  vibe_cli --neurosymbolic \"memory is full\"");

        Ok(())
    }

    pub async fn handle_neurosymbolic_install(&self, package: &str) -> Result<()> {
        let config_dir = self.config_dir();

        println!(
            "{}",
            format!("Installing domain package: {}", package)
                .green()
                .bold()
        );

        if package.starts_with("http://") || package.starts_with("https://") {
            println!("{}", "Downloading from URL...".yellow());
            let client = reqwest::Client::new();
            let response = client.get(package).send().await?;

            if response.status().is_success() {
                let content = response.text().await?;
                let domain_name = package
                    .split('/')
                    .last()
                    .unwrap_or(package)
                    .replace(".json", "");

                let target_dir = config_dir.join(&domain_name);
                std::fs::create_dir_all(&target_dir)?;
                std::fs::write(target_dir.join("domain.json"), content)?;

                println!("{}", format!("Installed domain: {}", domain_name).green());
            } else {
                eprintln!("{}", "Failed to download package".red());
            }
        } else {
            println!(
                "{}",
                format!("Looking for local package: {}", package).yellow()
            );
            let package_dir = std::path::Path::new(package);
            if package_dir.exists() && package_dir.is_dir() {
                let domain_name = package_dir
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                let target_dir = config_dir.join(&domain_name);
                std::fs::create_dir_all(&target_dir)?;

                for entry in std::fs::read_dir(package_dir)? {
                    let entry = entry?;
                    if entry.path().is_file() {
                        let file_name = entry.file_name();
                        std::fs::copy(entry.path(), target_dir.join(&file_name))?;
                    }
                }

                println!("{}", format!("Installed domain: {}", domain_name).green());
            } else {
                eprintln!("{}", format!("Package not found: {}", package).red());
            }
        }

        Ok(())
    }

    pub async fn handle_neurosymbolic_remove(&self, domain: &str) -> Result<()> {
        let config_dir = self.config_dir();
        let domain_dir = config_dir.join(domain);

        println!("{}", format!("Removing domain: {}", domain).green().bold());

        if domain_dir.exists() {
            std::fs::remove_dir_all(&domain_dir)?;
            println!("{}", format!("Removed: {}", domain_dir.display()).green());
        } else {
            println!("{}", format!("Domain not found: {}", domain).yellow());
        }

        Ok(())
    }

    pub async fn handle_neurosymbolic_edit(&self, domain: &str) -> Result<()> {
        let config_dir = self.config_dir();
        let domain_dir = config_dir.join(domain);

        println!("{}", format!("Editing domain: {}", domain).green().bold());

        if !domain_dir.exists() {
            println!(
                "{}",
                format!(
                    "Domain not found: {}. Use --neurosymbolic-add to create it.",
                    domain
                )
                .yellow()
            );
            return Ok(());
        }

        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());

        for entry in std::fs::read_dir(&domain_dir)? {
            let entry = entry?;
            if entry.path().is_file() {
                let file_name = entry.file_name();
                println!("{}", format!("Opening: {}", file_name.display()).yellow());

                let status = Command::new(&editor).arg(entry.path()).status()?;

                if status.success() {
                    println!("{}", format!("Saved: {}", file_name.display()).green());
                }
            }
        }

        Ok(())
    }

    pub async fn handle_neurosymbolic_add(&self, domain: &str) -> Result<()> {
        let config_dir = self.config_dir();
        let domain_dir = config_dir.join(domain);

        println!(
            "{}",
            format!("Adding new domain: {}", domain).green().bold()
        );

        std::fs::create_dir_all(&domain_dir.join("entities"))?;

        let domain_json = format!(
            r#"{{
    "domain": "{}",
    "version": "1.0.0",
    "description": "Custom domain: {}",
    "depends_on": [],
    "priority": 50,
    "enabled": true
}}"#,
            domain, domain
        );

        std::fs::write(domain_dir.join("domain.json"), &domain_json)?;
        println!("{}", format!("Created: {}/domain.json", domain).green());

        let ops_json = r#"[
    {
        "op_id": "custom_operation",
        "name": "Custom Operation",
        "description": "Description of your custom operation",
        "input_schema": {},
        "generators": [
            {
                "name": "custom_tool",
                "tool": "your-tool",
                "template": "your-tool --option value",
                "when": []
            }
        ],
        "examples": []
    }
]"#;

        std::fs::write(domain_dir.join("operations.json"), ops_json)?;
        println!("{}", format!("Created: {}/operations.json", domain).green());

        std::fs::write(domain_dir.join("relationships.json"), "[]")?;
        std::fs::write(domain_dir.join("inference_rules.json"), "[]")?;
        std::fs::write(domain_dir.join("troubleshooting.json"), "[]")?;

        println!("\n{}", "Domain template created!".green().bold());
        println!(
            "{}",
            format!("Edit with: vibe_cli --neurosymbolic-edit {}", domain).yellow()
        );

        Ok(())
    }

    pub async fn handle_neurosymbolic_list(&self) -> Result<()> {
        let config_dir = self.config_dir();

        println!("{}", "Installed Domains".green().bold());
        println!("{}", "==============".to_string());

        if !config_dir.exists() {
            println!(
                "{}",
                "No domains installed. Run --neurosymbolic-init first.".yellow()
            );
            return Ok(());
        }

        let mut domains: Vec<String> = Vec::new();

        for entry in std::fs::read_dir(&config_dir)? {
            let entry = entry?;
            if entry.path().is_dir() {
                let domain_name = entry.file_name().to_string_lossy().to_string();
                let domain_json = entry.path().join("domain.json");

                if domain_json.exists() {
                    if let Ok(content) = std::fs::read_to_string(&domain_json) {
                        if let Ok(domain) = serde_json::from_str::<serde_json::Value>(&content) {
                            let desc = domain
                                .get("description")
                                .and_then(|d| d.as_str())
                                .unwrap_or("");
                            let version = domain
                                .get("version")
                                .and_then(|v| v.as_str())
                                .unwrap_or("?");
                            let enabled = domain
                                .get("enabled")
                                .and_then(|e| e.as_bool())
                                .unwrap_or(true);

                            let status = if enabled { "enabled" } else { "disabled" };
                            println!(
                                "  {} - {} (v{}) [{}]",
                                domain_name.green().bold(),
                                desc,
                                version,
                                status
                            );
                            domains.push(domain_name);
                        }
                    }
                }
            }
        }

        if domains.is_empty() {
            println!("{}", "No domains found.".yellow());
        } else {
            println!("\n{}", format!("Total: {} domain(s)", domains.len()).cyan());
        }

        println!("\n{}", "Usage:".green());
        println!("  vibe_cli --neurosymbolic \"your query\"");
        println!("  vibe_cli --neurosymbolic-edit <domain>  # Edit a domain");
        println!("  vibe_cli --neurosymbolic-remove <domain>  # Remove a domain");

        Ok(())
    }

    pub fn handle_clear_cache(&self) -> Result<()> {
        let cache_paths = vec![
            self.cache_manager.cache_path().clone(),
            self.explain_cache_manager.cache_path().clone(),
            self.rag_cache_manager.cache_path().clone(),
        ];

        let mut cleared = 0;
        let mut failed = 0;

        for path in cache_paths {
            if path.exists() {
                match std::fs::remove_file(&path) {
                    Ok(_) => {
                        println!("Cleared: {}", path.display());
                        cleared += 1;
                    }
                    Err(e) => {
                        println!("Failed to clear {}: {:?}", path.display(), e);
                        failed += 1;
                    }
                }
            }
        }

        if cleared == 0 && failed == 0 {
            println!("No cache files found.");
        } else {
            println!("\nCleared {} cache file(s), {} failed", cleared, failed);
        }

        Ok(())
    }
}
