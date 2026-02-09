use crate::cli::streaming::request_command_stream_then_confirm;

use super::cache::{CacheManager, ExplainCacheManager, RagCacheManager};
use super::command_extraction::{extract_command_from_response, parse_agent_plan};
use super::utils::{detect_system_info, project_cache_suffix};
use application::services::integrated_neurosymbolic_service::{
    IntegratedNeurosymbolicService, IntentSignal,
};
use application::services::rag_service::RagService;
use colored::Colorize;
use infrastructure::storage::experience_buffer::FailureType;
use infrastructure::{config::Config, ollama_client::OllamaClient};
use serde::Deserialize;
use shared::confirmation::{ask_confirmation, ask_feedback};
use shared::types::Message;
use shared::types::Result;

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

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
    params: Option<HashMap<String, serde_json::Value>>,
}

pub struct CliHandlers {
    cache_manager: CacheManager,
    explain_cache_manager: ExplainCacheManager,
    rag_cache_manager: RagCacheManager,
    system_info: String,
    config: Config,
    rag_service: Option<RagService>,
    integrated_service: Option<IntegratedNeurosymbolicService>,
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
            integrated_service,
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
            r#"You must respond with ONLY valid JSON. No markdown, no explanation, no text before or after.

Analyze this query: "{}"

Return this exact JSON structure (all fields required):
{{
  "intent_category": "system_info",
  "action": "check",
  "target": "disk",
  "objects": [],
  "constraints": [],
  "params": {{}},
  "neurosymbolic_suitable": true,
  "reasoning": "Query asks to check disk usage, which is a system_info intent with check action on disk target"
}}

Rules:
- intent_category: system_info|process|memory|disk|network|service|user|file|log|hardware|general
- action: list|show|check|monitor|start|stop|restart|enable|disable|find|read|delete
- target: process|memory|disk|network|service|user|file|log|hardware|gpu|package|system
- neurosymbolic_suitable: true for most queries, false only for complex reasoning tasks
- reasoning: 1-2 sentence explanation of your interpretation

Query to analyze: "{}""#,
            query, query
        );

        for _ in 0..3 {
            let response = client.generate_response(&prompt).await?;
            let parsed = self.parse_intent_analysis(&response)?;
            let has_signal = parsed.intent != "unknown"
                || parsed.action.as_deref().unwrap_or("unknown") != "unknown"
                || parsed.target.as_deref().unwrap_or("unknown") != "unknown";
            if has_signal {
                return Ok(parsed);
            }
        }

        let fallback = self.extract_intent_from_keywords(query);
        Ok(fallback)
    }

    fn parse_intent_analysis(&self, response: &str) -> Result<IntentAnalysis> {
        let response = response.trim();
        let cleaned = if response.starts_with("```") && response.ends_with("```") {
            let start = response.find('\n').unwrap_or(0) + 1;
            let end = response.len().saturating_sub(3);
            response[start..end].trim().to_string()
        } else if response.starts_with("```json") {
            let start = response.find('\n').unwrap_or(0) + 1;
            let end = response.len().saturating_sub(3);
            response[start..end].trim().to_string()
        } else {
            response.to_string()
        };

        if let Ok(parsed) = serde_json::from_str::<IntentAnalysisResponse>(&cleaned) {
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
                .unwrap_or_else(|| "Parsed from JSON response".to_string());
            let params = parsed
                .params
                .unwrap_or_default()
                .into_iter()
                .filter_map(|(k, v)| match v {
                    serde_json::Value::String(s) => Some((k, s)),
                    serde_json::Value::Number(n) => Some((k, n.to_string())),
                    serde_json::Value::Bool(b) => Some((k, b.to_string())),
                    _ => None,
                })
                .collect::<HashMap<String, String>>();
            return Ok(IntentAnalysis {
                intent,
                neurosymbolic_suitable,
                reasoning,
                action: parsed.action.map(|a| Self::normalize_single_label(&a)),
                target: parsed.target.map(|t| Self::normalize_single_label(&t)),
                objects: parsed.objects.unwrap_or_default(),
                constraints: parsed.constraints.unwrap_or_default(),
                params,
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
        let mut reasoning = String::new();
        let mut action = None;
        let mut target = None;

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
            } else if line.starts_with("ACTION:") {
                action = Some(line.replace("ACTION:", "").trim().to_string());
            } else if line.starts_with("TARGET:") {
                target = Some(line.replace("TARGET:", "").trim().to_string());
            }
        }

        if intent != "unknown" || action.is_some() || target.is_some() {
            if reasoning.is_empty() {
                reasoning = format!("Extracted from response: intent={}, action={:?}, target={:?}", intent, action, target);
            }
            return Ok(IntentAnalysis {
                intent,
                neurosymbolic_suitable,
                reasoning,
                action,
                target,
                objects: Vec::new(),
                constraints: Vec::new(),
                params: HashMap::new(),
            });
        }

        Ok(IntentAnalysis {
            intent: "unknown".to_string(),
            neurosymbolic_suitable: false,
            reasoning: "Could not parse LLM response - returning for keyword-based fallback".to_string(),
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

    fn extract_intent_from_keywords(&self, query: &str) -> IntentAnalysis {
        let query_lower = query.to_lowercase();
        let mut intent = "general".to_string();
        let mut action: Option<String> = None;
        let mut target: Option<String> = None;
        let mut reasoning = String::new();

        if query_lower.contains("disk") || query_lower.contains("storage") || query_lower.contains("space") {
            intent = "disk".to_string();
            target = Some("disk".to_string());
            action = Some("check".to_string());
            reasoning = "Query contains disk/storage/space keywords, interpreted as disk check intent".to_string();
        } else if query_lower.contains("memory") || query_lower.contains("ram") || query_lower.contains("usage") {
            intent = "memory".to_string();
            target = Some("memory".to_string());
            action = Some("check".to_string());
            reasoning = "Query contains memory/ram/usage keywords, interpreted as memory check intent".to_string();
        } else if query_lower.contains("cpu") || query_lower.contains("processor") {
            intent = "system_info".to_string();
            target = Some("cpu".to_string());
            action = Some("check".to_string());
            reasoning = "Query contains cpu/processor keywords, interpreted as CPU check".to_string();
        } else if query_lower.contains("process") || query_lower.contains("running") || query_lower.contains("ps ") {
            intent = "process".to_string();
            target = Some("process".to_string());
            if query_lower.contains("list") || query_lower.contains("show") {
                action = Some("list".to_string());
            } else {
                action = Some("check".to_string());
            }
            reasoning = "Query contains process/running keywords, interpreted as process intent".to_string();
        } else if query_lower.contains("service") || query_lower.contains("nginx") || query_lower.contains("docker") || query_lower.contains("systemctl") {
            intent = "service".to_string();
            target = Some("service".to_string());
            action = Some("check".to_string());
            reasoning = "Query contains service/nginx/docker keywords, interpreted as service intent".to_string();
        } else if query_lower.contains("network") || query_lower.contains("connection") || query_lower.contains("port") {
            intent = "network".to_string();
            target = Some("network".to_string());
            action = Some("check".to_string());
            reasoning = "Query contains network/connection/port keywords, interpreted as network intent".to_string();
        } else if query_lower.contains("log") || query_lower.contains("journal") || query_lower.contains("error") {
            intent = "log".to_string();
            target = Some("log".to_string());
            action = Some("read".to_string());
            reasoning = "Query contains log/journal/error keywords, interpreted as log intent".to_string();
        } else if query_lower.contains("gpu") || query_lower.contains("graphics") || query_lower.contains("display") {
            intent = "hardware".to_string();
            target = Some("gpu".to_string());
            action = Some("show".to_string());
            reasoning = "Query contains gpu/graphics/display keywords, interpreted as hardware/gpu intent".to_string();
        } else if query_lower.contains("file") || query_lower.contains("directory") || query_lower.contains("find") {
            intent = "file".to_string();
            target = Some("file".to_string());
            action = Some("find".to_string());
            reasoning = "Query contains file/directory/find keywords, interpreted as file intent".to_string();
        } else if query_lower.contains("user") || query_lower.contains("who") || query_lower.contains("login") {
            intent = "user".to_string();
            target = Some("user".to_string());
            action = Some("list".to_string());
            reasoning = "Query contains user/who/login keywords, interpreted as user intent".to_string();
        } else if query_lower.contains("package") || query_lower.contains("install") || query_lower.contains("apt") {
            intent = "system_info".to_string();
            target = Some("package".to_string());
            action = Some("list".to_string());
            reasoning = "Query contains package/apt keywords, interpreted as package intent".to_string();
        } else {
            intent = "system_info".to_string();
            target = Some("system".to_string());
            action = Some("show".to_string());
            reasoning = format!("Default fallback for query: '{}' - interpreted as general system info request", query);
        }

        IntentAnalysis {
            intent,
            neurosymbolic_suitable: true,
            reasoning,
            action,
            target,
            objects: Vec::new(),
            constraints: Vec::new(),
            params: HashMap::new(),
        }
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
        let intent_signal = IntentSignal {
            category: Some(intent_analysis.intent.clone()),
            action: intent_analysis.action.clone(),
            target: intent_analysis.target.clone(),
            objects: intent_analysis.objects.clone(),
            constraints: intent_analysis.constraints.clone(),
            params: intent_analysis.params.clone(),
        };
        let result = {
            let service = self
                .integrated_service
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("Integrated neurosymbolic service unavailable"))?;
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
                            println!("{}", format!("Command failed: {}", stderr).red());
                            let _ = service.record_failure(
                                query,
                                &result.command,
                                FailureType::ExecutionFailed,
                                Some(stderr.trim()),
                            );
                        }
                    }
                } else if let Some(service) = self.integrated_service.as_ref() {
                    let _ = service.record_failure(
                        query,
                        &result.command,
                        FailureType::UserCancelled,
                        Some("User cancelled execution"),
                    );
                }
            }
            Err(e) => {
                eprintln!("Integrated neurosymbolic processing failed: {:?}", e);
                eprintln!("Falling back to standard query...");
                self.handle_query(query, ai_interpret, true).await?;
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
    async fn learn_command(&mut self, query: &str, command: &str) -> Result<()> {
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
            if let Some(service) = self.integrated_service.as_mut() {
                let _ = service.reload_domain_registry();
            }
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

        let domain_json = include_str!("domain_templates/linux/domain.json");
        std::fs::write(config_dir.join("linux/domain.json"), domain_json)?;
        println!("  {}", "OK domain.json");

        let ops_json = include_str!("domain_templates/linux/operations.json");

        let ops_path = config_dir.join("linux/operations.json");
        let base_ops: Vec<serde_json::Value> = serde_json::from_str(ops_json)?;
        let mut merged_ops: Vec<serde_json::Value> = Vec::new();

        if ops_path.exists() {
            if let Ok(existing_data) = std::fs::read_to_string(&ops_path) {
                if let Ok(existing_ops) =
                    serde_json::from_str::<Vec<serde_json::Value>>(&existing_data)
                {
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
        let entity_files = [
            ("process.json", include_str!("domain_templates/linux/entities/process.json")),
            ("file.json", include_str!("domain_templates/linux/entities/file.json")),
            ("service.json", include_str!("domain_templates/linux/entities/service.json")),
            ("network_connection.json", include_str!("domain_templates/linux/entities/network_connection.json")),
            ("user.json", include_str!("domain_templates/linux/entities/user.json")),
            ("filesystem.json", include_str!("domain_templates/linux/entities/filesystem.json")),
            ("memory.json", include_str!("domain_templates/linux/entities/memory.json")),
            ("cpu.json", include_str!("domain_templates/linux/entities/cpu.json")),
            ("network_interface.json", include_str!("domain_templates/linux/entities/network_interface.json")),
            ("docker_container.json", include_str!("domain_templates/linux/entities/docker_container.json")),
            ("systemd_unit.json", include_str!("domain_templates/linux/entities/systemd_unit.json")),
        ];
        for (name, content) in entity_files {
            std::fs::write(linux_dir.join(name), content)?;
        }

        println!("  {}", "OK entities/ (11 entities: Process, File, Service, NetworkConnection, User, Filesystem, MemoryInfo, Cpu, NetworkInterface, DockerContainer, SystemdUnit)");

        // Relationships
        let relationships_json = include_str!("domain_templates/linux/relationships.json");
        std::fs::write(config_dir.join("linux/relationships.json"), relationships_json)?;
        println!("  {}", "OK relationships.json (8 relationships)");

        let inference_rules_json = include_str!("domain_templates/linux/inference_rules.json");
        std::fs::write(
            config_dir.join("linux/inference_rules.json"),
            inference_rules_json,
        )?;
        println!("  {}", "OK inference_rules.json (30 inference rules)");

        let troubleshooting_json = include_str!("domain_templates/linux/troubleshooting.json");
        std::fs::write(
            config_dir.join("linux/troubleshooting.json"),
            troubleshooting_json,
        )?;
        println!(
            "  {}",
            "OK troubleshooting.json (15 troubleshooting patterns)"
        );

        let reasoning_templates_json =
            include_str!("domain_templates/linux/reasoning_templates.json");

        let templates_path = config_dir.join("linux/reasoning_templates.json");
        let base_templates: Vec<serde_json::Value> =
            serde_json::from_str(reasoning_templates_json)?;
        let mut merged_templates: Vec<serde_json::Value> = Vec::new();

        if templates_path.exists() {
            if let Ok(existing_data) = std::fs::read_to_string(&templates_path) {
                if let Ok(existing_templates) =
                    serde_json::from_str::<Vec<serde_json::Value>>(&existing_data)
                {
                    merged_templates.extend(existing_templates);
                }
            }
        }

        for base in base_templates {
            let base_id = base.get("template_id").and_then(|v| v.as_str()).unwrap_or("");
            if base_id.is_empty() {
                continue;
            }
            let exists = merged_templates.iter().any(|tpl| {
                tpl.get("template_id")
                    .and_then(|v| v.as_str())
                    .map(|id| id == base_id)
                    .unwrap_or(false)
            });
            if !exists {
                merged_templates.push(base);
            }
        }

        let templates_output = serde_json::to_string_pretty(&merged_templates)?;
        std::fs::write(&templates_path, templates_output)?;
        println!("  {}", "OK reasoning_templates.json (6 templates)");

        let shared_port = include_str!("domain_templates/shared/port.json");
        std::fs::write(shared_dir.join("port.json"), shared_port)?;

        println!(
            "\n{}",
            "OK Linux symbolic reasoning domain initialized!"
                .green()
                .bold()
        );
        println!("\n{}", "Summary:".green());
        println!("  - 32 operations (process, memory, disk, network, services, files, containers, hardware, security, etc.)");
        println!("  - 11 entities (Process, File, Service, NetworkConnection, User, Filesystem, MemoryInfo, Cpu, NetworkInterface, DockerContainer, SystemdUnit)");
        println!("  - 8 relationships (hierarchical, ownership, containment, etc.)");
        println!("  - 30 inference rules for symbolic reasoning");
        println!("  - 15 troubleshooting patterns for common issues");
        println!("  - 21 reasoning templates for step-by-step diagnostics");

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
                            let enabled = domain
                                .get("enabled")
                                .and_then(|e| e.as_bool())
                                .unwrap_or(true);

                            let status = if enabled { "enabled" } else { "disabled" };
                            println!(
                                "  {} - {} [{}]",
                                domain_name.green().bold(),
                                desc,
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
